use crate::app::state::AppState;
use crate::infrastructure::db::MediaItemRecord;
use crate::modules::metadata::{MetadataModule, VideoMetadata};
use crate::modules::settings::SettingsModule;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use walkdir::WalkDir;

pub struct ScannerModule;

#[derive(Default)]
struct LibraryScanResult {
	items: Vec<MediaItemRecord>,
	top_level_count: usize
}

#[derive(Default)]
struct ShowScanResult {
	series_items: Vec<MediaItemRecord>,
	episode_items: Vec<MediaItemRecord>
}

#[derive(Default)]
struct MusicScanResult {
	artist_items: Vec<MediaItemRecord>,
	album_items: Vec<MediaItemRecord>,
	track_items: Vec<MediaItemRecord>
}

struct ShowAggregate {
	show_id: String,
	title: String,
	year: Option<i32>,
	poster_path: Option<String>,
	cover_path: Option<String>,
	media_path: String,
	source_name: String,
	episodes: Vec<MediaItemRecord>
}

struct MusicArtistAggregate {
	artist_id: String,
	title: String,
	poster_path: Option<String>,
	media_path: String,
	source_name: String,
	albums: BTreeMap<String, MusicAlbumAggregate>
}

struct MusicAlbumAggregate {
	album_id: String,
	title: String,
	year: Option<i32>,
	poster_path: Option<String>,
	media_path: String,
	source_name: String,
	tracks: Vec<MediaItemRecord>
}

impl ScannerModule {
	pub fn scan_library(state: &AppState) -> Result<usize, String> {
		let root = SettingsModule::library_root_path(state)?
			.ok_or_else(|| "Library root path is not configured".to_string())?;

		scan_and_replace_library(state, &root)
	}

	pub fn reset_and_scan_library(state: &AppState) -> Result<usize, String> {
		let root = SettingsModule::library_root_path(state)?
			.ok_or_else(|| "Library root path is not configured".to_string())?;

		state.database.clear_all_data()?;
		SettingsModule::save_library_root_path(state, &root)?;

		scan_and_replace_library(state, &root)
	}
}

fn scan_and_replace_library(state: &AppState, root: &str) -> Result<usize, String> {
	let app_data_dir = state
		.database
		.db_path()
		.parent()
		.ok_or_else(|| "App data directory is not available".to_string())?;
	let scan_result = scan_root(Path::new(root), app_data_dir)?;

	state.database.replace_media_items(&scan_result.items)?;
	SettingsModule::set_initial_scan_completed(state)?;

	Ok(scan_result.top_level_count)
}

fn scan_root(root: &Path, app_data_dir: &Path) -> Result<LibraryScanResult, String> {
	if !root.exists() {
		return Err("Selected library path does not exist".to_string());
	}

	let mut result = LibraryScanResult::default();

	for (path, media_type) in [
		(root.join("movies"), "movie"),
		(root.join("documentary"), "documentary"),
		(root.join("documentaries"), "documentary"),
		(root.join("standup"), "standup"),
		(root.join("concerts"), "concert")
	] {
		let items = scan_flat_category(&path, media_type, app_data_dir)?;
		result.top_level_count += items.len();
		result.items.extend(items);
	}

	let show_result = scan_show_category(&root.join("shows"), app_data_dir)?;
	result.top_level_count += show_result.series_items.len();
	result.items.extend(show_result.series_items);
	result.items.extend(show_result.episode_items);

	let music_result = scan_music_category(&root.join("music"), &app_data_dir.join("music-covers"))?;
	result.top_level_count += music_result.artist_items.len();
	result.items.extend(music_result.artist_items);
	result.items.extend(music_result.album_items);
	result.items.extend(music_result.track_items);

	Ok(result)
}

fn scan_flat_category(category_root: &Path, media_type: &str, app_data_dir: &Path) -> Result<Vec<MediaItemRecord>, String> {
	if !category_root.exists() {
		return Ok(Vec::new());
	}

	let mut items = Vec::new();

	for entry in WalkDir::new(category_root).follow_links(true).into_iter().filter_map(Result::ok) {
		let path = entry.path();

		if !entry.file_type().is_file() || !is_video_file(path) {
			continue;
		}

		let media_directory = path.parent().unwrap_or(category_root);
		let source_name = file_stem(path);
		let display_name = preferred_flat_display_name(category_root, media_directory, &source_name);
		let (title, year) = parse_media_name(&display_name);

		let mut item = MediaItemRecord {
			id: path.to_string_lossy().to_string(),
			title,
			media_type: media_type.to_string(),
			year,
			poster_path: find_companion_file(media_directory, &["poster", "cover"], IMAGE_EXTENSIONS),
			cover_path: find_companion_file(media_directory, &["cover", "poster"], IMAGE_EXTENSIONS),
			genre: None,
			director: None,
			actors: None,
			imdb_rating: None,
			imdb_id: None,
			tmdb_id: None,
			media_path: path.to_string_lossy().to_string(),
			subtitle_path: find_companion_file(media_directory, &["subtitle", "subtitles"], SUBTITLE_EXTENSIONS)
				.or_else(|| find_any_with_extension(media_directory, SUBTITLE_EXTENSIONS)),
			source_name,
			show_id: None,
			season_number: None,
			episode_number: None
		};

		if media_type == "movie" {
			let metadata = MetadataModule::resolve_video_metadata(
				media_directory,
				&item.title,
				item.year,
				media_type,
				item.poster_path.clone(),
				item.cover_path.clone(),
				app_data_dir,
				false
			);
			apply_video_metadata(&mut item, metadata);
		}

		items.push(item);
	}

	Ok(items)
}

fn scan_show_category(shows_root: &Path, app_data_dir: &Path) -> Result<ShowScanResult, String> {
	if !shows_root.exists() {
		return Ok(ShowScanResult::default());
	}

	let mut shows = BTreeMap::<String, ShowAggregate>::new();

	for entry in WalkDir::new(shows_root).follow_links(true).into_iter().filter_map(Result::ok) {
		let path = entry.path();

		if !entry.file_type().is_file() || !is_video_file(path) {
			continue;
		}

		let media_directory = path.parent().unwrap_or(shows_root);
		let source_name = file_stem(path);
		let relative = path.strip_prefix(shows_root).unwrap_or(path);
		let parts = relative
			.components()
			.filter_map(|component| component.as_os_str().to_str())
			.collect::<Vec<_>>();

		if parts.is_empty() {
			continue;
		}

		let series_folder_name = parts.first().copied().unwrap_or(&source_name);
		let season_folder_name = parts.get(1).copied();
		let series_root = shows_root.join(series_folder_name);
		let show_id = series_root.to_string_lossy().to_string();
		let (series_title, year) = parse_media_name(series_folder_name);
		let season_number = season_folder_name.and_then(parse_season_number).unwrap_or(1) as i32;
		let episode_number = parse_episode_number(&source_name).map(|value| value as i32);
		let episode_title = parse_episode_title(&source_name, episode_number);
		let poster_path = find_nearest_poster(media_directory, shows_root, series_folder_name);
		let cover_path = find_nearest_cover(media_directory, shows_root, series_folder_name);

		let episode_record = MediaItemRecord {
			id: path.to_string_lossy().to_string(),
			title: episode_title,
			media_type: "show_episode".to_string(),
			year,
			poster_path: None,
			cover_path: None,
			genre: None,
			director: None,
			actors: None,
			imdb_rating: None,
			imdb_id: None,
			tmdb_id: None,
			media_path: path.to_string_lossy().to_string(),
			subtitle_path: find_companion_file(media_directory, &["subtitle", "subtitles"], SUBTITLE_EXTENSIONS)
				.or_else(|| find_any_with_extension(media_directory, SUBTITLE_EXTENSIONS)),
			source_name: source_name.clone(),
			show_id: Some(show_id.clone()),
			season_number: Some(season_number),
			episode_number
		};

		let entry = shows.entry(show_id.clone()).or_insert_with(|| ShowAggregate {
			show_id: show_id.clone(),
			title: series_title.clone(),
			year,
			poster_path: poster_path.clone(),
			cover_path: cover_path.clone(),
			media_path: series_root.to_string_lossy().to_string(),
			source_name: series_folder_name.to_string(),
			episodes: Vec::new()
		});

		if entry.poster_path.is_none() {
			entry.poster_path = poster_path.clone();
		}

		if entry.cover_path.is_none() {
			entry.cover_path = cover_path;
		}

		entry.episodes.push(episode_record);
	}

	let mut result = ShowScanResult::default();

	for (_, mut show) in shows {
		show.episodes.sort_by(|left, right| {
			left
				.season_number
				.unwrap_or(1)
				.cmp(&right.season_number.unwrap_or(1))
				.then(left.episode_number.unwrap_or(0).cmp(&right.episode_number.unwrap_or(0)))
				.then_with(|| left.title.cmp(&right.title))
		});

		let mut series_item = MediaItemRecord {
			id: show.show_id,
			title: show.title,
			media_type: "show".to_string(),
			year: show.year,
			poster_path: show.poster_path,
			cover_path: show.cover_path,
			genre: None,
			director: None,
			actors: None,
			imdb_rating: None,
			imdb_id: None,
			tmdb_id: None,
			media_path: show.media_path,
			subtitle_path: None,
			source_name: show.source_name,
			show_id: None,
			season_number: None,
			episode_number: None
		};
		let metadata = MetadataModule::resolve_video_metadata(
			Path::new(&series_item.media_path),
			&series_item.title,
			series_item.year,
			"show",
			series_item.poster_path.clone(),
			series_item.cover_path.clone(),
			app_data_dir,
			false
		);
		apply_video_metadata(&mut series_item, metadata);
		result.series_items.push(series_item);

		result.episode_items.extend(show.episodes);
	}

	Ok(result)
}

fn apply_video_metadata(item: &mut MediaItemRecord, metadata: VideoMetadata) {
	if metadata.year.is_some() {
		item.year = metadata.year;
	}

	if metadata.poster_path.is_some() {
		item.poster_path = metadata.poster_path;
	}

	if metadata.cover_path.is_some() {
		item.cover_path = metadata.cover_path;
	}

	item.genre = metadata.genre;
	item.director = metadata.director;
	item.actors = metadata.actors;
	item.imdb_rating = metadata.imdb_rating;
	item.imdb_id = metadata.imdb_id;
	item.tmdb_id = metadata.tmdb_id;
}

fn scan_music_category(music_root: &Path, cover_cache_root: &Path) -> Result<MusicScanResult, String> {
	if !music_root.exists() {
		return Ok(MusicScanResult::default());
	}

	let mut artists = BTreeMap::<String, MusicArtistAggregate>::new();

	for entry in WalkDir::new(music_root).follow_links(true).into_iter().filter_map(Result::ok) {
		let path = entry.path();

		if !entry.file_type().is_file() || !is_audio_file(path) {
			continue;
		}

		let relative = path.strip_prefix(music_root).unwrap_or(path);
		let parts = relative
			.components()
			.filter_map(|component| component.as_os_str().to_str())
			.collect::<Vec<_>>();

		if parts.len() < 3 {
			continue;
		}

		let artist_folder_name = parts[0];
		let album_folder_name = parts[1];
		let artist_root = music_root.join(artist_folder_name);
		let album_root = artist_root.join(album_folder_name);
		let artist_id = artist_root.to_string_lossy().to_string();
		let album_id = album_root.to_string_lossy().to_string();
		let source_name = file_stem(path);
		let (album_title, title_year) = parse_music_album_name(album_folder_name);
		let existing_album_year = artists
			.get(&artist_id)
			.and_then(|artist| artist.albums.get(&album_id))
			.and_then(|album| album.year);
		let metadata_year = if existing_album_year.is_none() || existing_album_year == title_year {
			read_audio_year(path)
		} else {
			None
		};
		let year = metadata_year.or(existing_album_year).or(title_year);
		let artist_cover_path = find_companion_file(
			&artist_root,
			&["artist", "band", "cover", "folder", "front", "poster"],
			IMAGE_EXTENSIONS
		);
		let cover_path = find_companion_file(&album_root, &["cover", "folder", "front", "poster"], IMAGE_EXTENSIONS)
			.or_else(|| extract_embedded_cover(path, cover_cache_root, &album_id));
		let track_number = parse_track_number(&source_name).map(|value| value as i32);
		let track_title = parse_track_title(&source_name, track_number);

		let track_record = MediaItemRecord {
			id: path.to_string_lossy().to_string(),
			title: track_title,
			media_type: "music_track".to_string(),
			year,
			poster_path: cover_path.clone(),
			cover_path: cover_path.clone(),
			genre: None,
			director: None,
			actors: None,
			imdb_rating: None,
			imdb_id: None,
			tmdb_id: None,
			media_path: path.to_string_lossy().to_string(),
			subtitle_path: None,
			source_name: source_name.clone(),
			show_id: Some(album_id.clone()),
			season_number: track_number,
			episode_number: None
		};

		let artist_entry = artists.entry(artist_id.clone()).or_insert_with(|| MusicArtistAggregate {
			artist_id: artist_id.clone(),
			title: cleanup_title(&normalize_name(artist_folder_name)),
			poster_path: artist_cover_path.clone(),
			media_path: artist_root.to_string_lossy().to_string(),
			source_name: artist_folder_name.to_string(),
			albums: BTreeMap::new()
		});

		let album_entry = artist_entry.albums.entry(album_id.clone()).or_insert_with(|| MusicAlbumAggregate {
			album_id: album_id.clone(),
			title: album_title.clone(),
			year,
			poster_path: cover_path.clone(),
			media_path: album_root.to_string_lossy().to_string(),
			source_name: album_folder_name.to_string(),
			tracks: Vec::new()
		});

		if album_entry.poster_path.is_none() {
			album_entry.poster_path = cover_path.clone();
		}

		if album_entry.year.is_none() || album_entry.year == title_year {
			album_entry.year = year;
		}

		if artist_entry.poster_path.is_none() {
			artist_entry.poster_path = cover_path.clone();
		}

		album_entry.tracks.push(track_record);
	}

	let mut result = MusicScanResult::default();

	for (_, artist) in artists {
		let mut albums = artist.albums.into_values().collect::<Vec<_>>();
		albums.sort_by(|left, right| {
			left
				.year
				.unwrap_or(i32::MAX)
				.cmp(&right.year.unwrap_or(i32::MAX))
				.then_with(|| left.title.cmp(&right.title))
		});

		result.artist_items.push(MediaItemRecord {
			id: artist.artist_id.clone(),
			title: artist.title,
			media_type: "music_artist".to_string(),
			year: None,
			poster_path: artist.poster_path,
			cover_path: None,
			genre: None,
			director: None,
			actors: None,
			imdb_rating: None,
			imdb_id: None,
			tmdb_id: None,
			media_path: artist.media_path,
			subtitle_path: None,
			source_name: artist.source_name,
			show_id: None,
			season_number: None,
			episode_number: None
		});

		for mut album in albums {
			album.tracks.sort_by(|left, right| {
				left
					.season_number
					.unwrap_or(i32::MAX)
					.cmp(&right.season_number.unwrap_or(i32::MAX))
					.then_with(|| left.title.cmp(&right.title))
			});

			result.album_items.push(MediaItemRecord {
				id: album.album_id.clone(),
				title: album.title,
				media_type: "music_album".to_string(),
				year: album.year,
				poster_path: album.poster_path.clone(),
				cover_path: album.poster_path.clone(),
				genre: None,
				director: None,
				actors: None,
				imdb_rating: None,
				imdb_id: None,
				tmdb_id: None,
				media_path: album.media_path,
				subtitle_path: None,
				source_name: album.source_name,
				show_id: Some(artist.artist_id.clone()),
				season_number: None,
				episode_number: None
			});

			result.track_items.extend(album.tracks);
		}
	}

	Ok(result)
}

fn extract_embedded_cover(audio_path: &Path, cover_cache_root: &Path, album_id: &str) -> Option<String> {
	if let Some(existing_cover_path) = find_cached_cover_path(cover_cache_root, album_id) {
		return Some(existing_cover_path);
	}

	fs::create_dir_all(cover_cache_root).ok()?;

	let tagged_file = Probe::open(audio_path).ok()?.read().ok()?;
	let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag())?;
	let picture = tag.pictures().first()?;
	let extension = detect_picture_extension(picture.data());
	let cover_path = cover_cache_root.join(format!("{}.{}", hash_cache_key(album_id), extension));

	fs::write(&cover_path, picture.data()).ok()?;

	Some(cover_path.to_string_lossy().to_string())
}

fn find_cached_cover_path(cover_cache_root: &Path, album_id: &str) -> Option<String> {
	let cache_key = hash_cache_key(album_id);

	for extension in CACHED_IMAGE_EXTENSIONS {
		let cover_path = cover_cache_root.join(format!("{}.{}", cache_key, extension));

		if cover_path.exists() {
			return Some(cover_path.to_string_lossy().to_string());
		}
	}

	None
}

fn hash_cache_key(value: &str) -> String {
	let mut hasher = DefaultHasher::new();
	value.hash(&mut hasher);
	format!("{:016x}", hasher.finish())
}

fn detect_picture_extension(data: &[u8]) -> &'static str {
	if data.starts_with(&[0x89, b'P', b'N', b'G']) {
		return "png";
	}

	if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
		return "jpg";
	}

	if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
		return "gif";
	}

	if data.starts_with(b"RIFF") && data.len() > 12 && &data[8..12] == b"WEBP" {
		return "webp";
	}

	if data.starts_with(b"BM") {
		return "bmp";
	}

	"jpg"
}

fn is_video_file(path: &Path) -> bool {
	has_extension(path, VIDEO_EXTENSIONS)
}

fn is_audio_file(path: &Path) -> bool {
	has_extension(path, AUDIO_EXTENSIONS)
}

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
	path
		.extension()
		.and_then(OsStr::to_str)
		.map(|extension| extensions.iter().any(|candidate| extension.eq_ignore_ascii_case(candidate)))
		.unwrap_or(false)
}

fn find_companion_file(directory: &Path, names: &[&str], extensions: &[&str]) -> Option<String> {
	let entries = std::fs::read_dir(directory).ok()?;

	for entry in entries.filter_map(Result::ok) {
		let path = entry.path();
		let stem = path.file_stem().and_then(OsStr::to_str)?;

		if names.iter().any(|name| stem.eq_ignore_ascii_case(name)) && has_extension(&path, extensions) {
			return Some(path.to_string_lossy().to_string());
		}
	}

	None
}

fn find_any_with_extension(directory: &Path, extensions: &[&str]) -> Option<String> {
	let entries = std::fs::read_dir(directory).ok()?;

	for entry in entries.filter_map(Result::ok) {
		let path = entry.path();

		if has_extension(&path, extensions) {
			return Some(path.to_string_lossy().to_string());
		}
	}

	None
}

fn parse_media_name(raw: &str) -> (String, Option<i32>) {
	let normalized = normalize_name(raw);
	let tokens = normalized.split_whitespace().collect::<Vec<_>>();
	let mut title_tokens = Vec::new();
	let mut year = None;

	for token in &tokens {
		if is_episode_token(token) || is_release_marker(token) {
			break;
		}

		if year.is_none() && is_year_token(token) {
			year = token.parse::<i32>().ok();
			break;
		}

		title_tokens.push((*token).to_string());
	}

	let title = if title_tokens.is_empty() {
		normalized.clone()
	} else {
		title_tokens.join(" ")
	};

	(cleanup_title(&title), year)
}

fn preferred_flat_display_name(category_root: &Path, media_directory: &Path, source_name: &str) -> String {
	if media_directory != category_root {
		if let Some(folder_name) = media_directory.file_name().and_then(OsStr::to_str) {
			if !looks_like_season_folder(folder_name) {
				return folder_name.to_string();
			}
		}
	}

	source_name.to_string()
}

fn file_stem(path: &Path) -> String {
	path.file_stem()
		.and_then(OsStr::to_str)
		.unwrap_or("Untitled")
		.to_string()
}

fn normalize_name(raw: &str) -> String {
	raw.chars()
		.map(|character| match character {
			'.' | '_' => ' ',
			'[' | ']' | '(' | ')' | '{' | '}' => ' ',
			_ => character
		})
		.collect::<String>()
		.split_whitespace()
		.collect::<Vec<_>>()
		.join(" ")
}

fn cleanup_title(raw: &str) -> String {
	raw.trim().trim_matches('-').trim().to_string()
}

fn looks_like_season_folder(raw: &str) -> bool {
	parse_season_number(raw).is_some()
}

fn parse_season_number(raw: &str) -> Option<u32> {
	let tokens = normalize_name(raw);
	let words = tokens.split_whitespace().collect::<Vec<_>>();

	if words.iter().any(|word| {
		matches!(word.to_ascii_lowercase().as_str(), "extras" | "specials")
	}) {
		return Some(0);
	}

	for (index, word) in words.iter().enumerate() {
		let lower = word.to_ascii_lowercase();

		if lower == "season" || lower == "temporada" {
			if let Some(next_word) = words.get(index + 1) {
				if let Ok(number) = next_word.parse::<u32>() {
					return Some(number);
				}
			}
		}

		if let Some(number) = lower.strip_prefix('s').and_then(|value| value.parse::<u32>().ok()) {
			return Some(number);
		}
	}

	None
}

fn parse_episode_number(raw: &str) -> Option<u32> {
	for word in normalize_name(raw).split_whitespace() {
		let lower = word.to_ascii_lowercase();

		if let Some((_, episode)) = parse_sxe_token(&lower) {
			return Some(episode);
		}

		if let Some((_, episode)) = parse_nx_token(&lower) {
			return Some(episode);
		}

		if looks_like_episode_number_token(word) {
			return word.parse::<u32>().ok();
		}
	}

	None
}

fn parse_episode_title(raw: &str, episode_number: Option<i32>) -> String {
	let normalized = normalize_name(raw);
	let tokens = normalized.split_whitespace().collect::<Vec<_>>();
	let start_index = tokens.iter().position(|token| {
		let lower = token.to_ascii_lowercase();
		is_episode_token(&lower) || looks_like_episode_number_token(token)
	});

	if let Some(index) = start_index {
		let title = tokens
			.iter()
			.skip(index + 1)
			.take_while(|token| !is_release_marker(token) && !is_year_token(token))
			.cloned()
			.collect::<Vec<_>>()
			.join(" ");

		if !title.is_empty() {
			return cleanup_title(&title);
		}
	}

	if let Some(number) = episode_number {
		return format!("Episode {:02}", number);
	}

	cleanup_title(raw)
}

fn parse_track_number(raw: &str) -> Option<u32> {
	let normalized = normalize_name(raw);
	let first_token = normalized.split_whitespace().next()?;

	if first_token.chars().all(|character| character.is_ascii_digit()) {
		return first_token.parse::<u32>().ok().filter(|number| *number > 0);
	}

	None
}

fn parse_track_title(raw: &str, track_number: Option<i32>) -> String {
	let normalized = normalize_name(raw);
	let tokens = normalized.split_whitespace().collect::<Vec<_>>();

	if let Some(first_token) = tokens.first() {
		if first_token.chars().all(|character| character.is_ascii_digit()) {
			let title = tokens.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");

			if !title.is_empty() {
				return cleanup_title(&title);
			}
		}
	}

	if let Some(number) = track_number {
		return format!("Faixa {:02}", number);
	}

	cleanup_title(&normalized)
}

fn parse_music_album_name(raw: &str) -> (String, Option<i32>) {
	let fallback_year = parse_year_in_parentheses(raw);
	let normalized_title = normalize_name(&remove_year_parentheses(raw));
	let title = cleanup_title(if normalized_title.is_empty() { raw } else { &normalized_title });

	(title, fallback_year)
}

fn read_audio_year(audio_path: &Path) -> Option<i32> {
	let tagged_file = Probe::open(audio_path).ok()?.read().ok()?;
	let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag())?;

	for key in [
		ItemKey::RecordingDate,
		ItemKey::Year,
		ItemKey::ReleaseDate,
		ItemKey::OriginalReleaseDate
	] {
		if let Some(year) = tag.get_string(&key).and_then(parse_year_from_text) {
			return Some(year);
		}
	}

	None
}

fn parse_year_in_parentheses(raw: &str) -> Option<i32> {
	find_year_parentheses_range(raw).and_then(|(start, end)| parse_year_from_text(&raw[start..end]))
}

fn remove_year_parentheses(raw: &str) -> String {
	let Some((start, end)) = find_year_parentheses_range(raw) else {
		return raw.to_string();
	};

	let prefix = raw[..start.saturating_sub(1)].trim_end();
	let suffix = raw[end + 1..].trim_start();

	match (prefix.is_empty(), suffix.is_empty()) {
		(true, true) => String::new(),
		(true, false) => suffix.to_string(),
		(false, true) => prefix.to_string(),
		(false, false) => format!("{} {}", prefix, suffix)
	}
}

fn find_year_parentheses_range(raw: &str) -> Option<(usize, usize)> {
	let mut open_index = None;

	for (index, character) in raw.char_indices() {
		if character == '(' {
			open_index = Some(index);
			continue;
		}

		if character == ')' {
			let start = open_index?;
			let content_start = start + '('.len_utf8();
			let content = raw[content_start..index].trim();

			if parse_year_from_text(content).is_some() {
				return Some((content_start, index));
			}

			open_index = None;
		}
	}

	None
}

fn parse_year_from_text(raw: &str) -> Option<i32> {
	raw
		.split(|character: char| !character.is_ascii_digit())
		.find(|token| is_year_token(token))
		.and_then(|token| token.parse::<i32>().ok())
}

fn is_episode_token(raw: &str) -> bool {
	let lower = raw.to_ascii_lowercase();
	parse_sxe_token(&lower).is_some() || parse_nx_token(&lower).is_some()
}

fn parse_sxe_token(raw: &str) -> Option<(u32, u32)> {
	let without_s = raw.strip_prefix('s')?;
	let (season, episode) = without_s.split_once('e')?;
	Some((season.parse().ok()?, episode.parse().ok()?))
}

fn parse_nx_token(raw: &str) -> Option<(u32, u32)> {
	let (season, episode) = raw.split_once('x')?;
	Some((season.parse().ok()?, episode.parse().ok()?))
}

fn looks_like_episode_number_token(raw: &str) -> bool {
	raw.len() <= 3
		&& raw.chars().all(|character| character.is_ascii_digit())
		&& raw.parse::<u32>().map(|number| number > 0).unwrap_or(false)
}

fn is_year_token(raw: &str) -> bool {
	raw.len() == 4
		&& raw.chars().all(|character| character.is_ascii_digit())
		&& raw.parse::<i32>().map(|year| (1900..=2100).contains(&year)).unwrap_or(false)
}

fn is_release_marker(raw: &str) -> bool {
	matches!(
		raw.to_ascii_lowercase().as_str(),
		"2160p"
			| "1080p"
			| "720p"
			| "480p"
			| "hdr"
			| "hdrip"
			| "bluray"
			| "brrip"
			| "webrip"
			| "webdl"
			| "web-dl"
			| "dvdrip"
			| "hdtv"
			| "remux"
			| "x264"
			| "x265"
			| "h264"
			| "h265"
			| "hevc"
			| "aac"
			| "dts"
			| "10bit"
			| "proper"
			| "repack"
			| "extended"
			| "dubbed"
			| "dual"
			| "multi"
	)
}

fn find_nearest_poster(media_directory: &Path, shows_root: &Path, series_folder_name: &str) -> Option<String> {
	find_companion_file(media_directory, &["poster", "cover"], IMAGE_EXTENSIONS).or_else(|| {
		let series_root = shows_root.join(series_folder_name);
		find_companion_file(&series_root, &["poster", "cover"], IMAGE_EXTENSIONS)
	})
}

fn find_nearest_cover(media_directory: &Path, shows_root: &Path, series_folder_name: &str) -> Option<String> {
	find_companion_file(media_directory, &["cover", "poster"], IMAGE_EXTENSIONS).or_else(|| {
		let series_root = shows_root.join(series_folder_name);
		find_companion_file(&series_root, &["cover", "poster"], IMAGE_EXTENSIONS)
	})
}

const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "mov", "wmv", "m4v", "webm"];
const AUDIO_EXTENSIONS: &[&str] = &["flac", "mp3", "m4a", "aac", "ogg", "opus", "wav", "wma"];
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "jfif", "png", "webp", "gif", "bmp", "avif"];
const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa", "vtt", "sub"];
const CACHED_IMAGE_EXTENSIONS: &[&str] = &["jpg", "png", "gif", "webp", "bmp"];
