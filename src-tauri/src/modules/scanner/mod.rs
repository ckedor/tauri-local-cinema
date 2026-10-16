use crate::app::state::AppState;
use crate::infrastructure::db::{DirectorRecord, MediaDirectorLinkRecord, MediaItemRecord};
use crate::modules::metadata::{DirectorMetadata, MetadataApiKeys, MetadataModule, VideoMetadata};
use crate::modules::settings::SettingsModule;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::ffi::OsStr;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

pub struct ScannerModule;

const LIBRARY_SCAN_PROGRESS_EVENT: &str = "library://scan-progress";
const TMDB_API_KEY_FILE_NAME: &str = "TMDB_API_KEY.txt";

#[derive(Default)]
struct LibraryScanResult {
	items: Vec<MediaItemRecord>,
	directors: Vec<DirectorRecord>,
	director_links: Vec<MediaDirectorLinkRecord>,
	top_level_count: usize
}

#[derive(Default)]
struct FlatScanResult {
	items: Vec<MediaItemRecord>,
	directors: Vec<DirectorRecord>,
	director_links: Vec<MediaDirectorLinkRecord>
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

#[derive(Clone, Copy)]
struct ScanOptions {
	force_remote_refresh: bool
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanProgressPayload {
	phase: String,
	message: String,
	processed: usize,
	total: usize,
	percent: u8
}

struct ScanProgress {
	app: Option<AppHandle>,
	processed: usize,
	total: usize
}

impl ScanProgress {
	fn new(app: Option<AppHandle>, total: usize) -> Self {
		Self {
			app,
			processed: 0,
			total: total.max(1)
		}
	}

	fn emit_status(&self, phase: &str, message: impl Into<String>) {
		self.emit(phase, message.into(), self.processed);
	}

	fn advance(&mut self, phase: &str, message: impl Into<String>) {
		self.processed = self.processed.saturating_add(1).min(self.total);
		self.emit(phase, message.into(), self.processed);
	}

	fn finish(&mut self, message: impl Into<String>) {
		self.processed = self.total;
		self.emit("done", message.into(), self.total);
	}

	fn emit(&self, phase: &str, message: String, processed: usize) {
		let Some(app) = &self.app else {
			return;
		};

		let percent = ((processed as f32 / self.total as f32) * 100.0)
			.round()
			.clamp(0.0, 100.0) as u8;
		let _ = app.emit(
			LIBRARY_SCAN_PROGRESS_EVENT,
			ScanProgressPayload {
				phase: phase.to_string(),
				message,
				processed,
				total: self.total,
				percent
			}
		);
	}
}

impl ScannerModule {
	pub fn scan_library(state: &AppState, app: Option<AppHandle>) -> Result<usize, String> {
		Self::rescan_library(state, app)
	}

	pub fn reset_and_scan_library(state: &AppState, app: Option<AppHandle>) -> Result<usize, String> {
		let roots = SettingsModule::library_root_paths(state)?;
		let api_keys = SettingsModule::metadata_api_keys(state)?;
		if roots.is_empty() {
			return Err("Library directories are not configured".to_string());
		}

		state.database.clear_all_data()?;
		SettingsModule::save_library_root_paths(state, &roots)?;
		SettingsModule::save_metadata_api_keys(state, api_keys.tmdb_api_key.as_deref(), api_keys.anthropic_api_key.as_deref())?;

		scan_and_sync_libraries(state, &roots, app, ScanOptions { force_remote_refresh: true })
	}

	pub fn rescan_library(state: &AppState, app: Option<AppHandle>) -> Result<usize, String> {
		let roots = SettingsModule::library_root_paths(state)?;
		if roots.is_empty() {
			return Err("Library directories are not configured".to_string());
		}

		SettingsModule::save_library_root_paths(state, &roots)?;

		scan_and_sync_libraries(state, &roots, app, ScanOptions { force_remote_refresh: false })
	}

	/// Indexes one item folder (a movie folder or a whole series folder) and
	/// upserts it without touching the rest of the library, so adding a title
	/// doesn't walk the whole drive. `force_remote_refresh` re-fetches TMDb
	/// metadata and rewrites the item's local metadata file.
	pub fn index_item(
		state: &AppState,
		app: Option<AppHandle>,
		category_root: &Path,
		item_root: &Path,
		media_type: &str,
		force_remote_refresh: bool
	) -> Result<(), String> {
		let app_data_dir = state
			.database
			.db_path()
			.parent()
			.ok_or_else(|| "App data directory is not available".to_string())?;
		let api_keys = SettingsModule::metadata_api_keys(state)?;
		let api_keys = MetadataApiKeys { tmdb_api_key: api_keys.tmdb_api_key };
		let mut progress = ScanProgress::new(app, 1);
		let options = ScanOptions { force_remote_refresh };

		if media_type == "show" {
			let result = scan_show_category(category_root, item_root, app_data_dir, &api_keys, &mut progress, options)?;
			let items = result.series_items.into_iter().chain(result.episode_items).collect::<Vec<_>>();
			return state.database.upsert_media_items(&items, &[], &[]);
		}

		let result = scan_flat_category(category_root, item_root, media_type, app_data_dir, &api_keys, &mut progress, options)?;
		state.database.upsert_media_items(&result.items, &result.directors, &result.director_links)
	}
}

fn scan_and_sync_libraries(
	state: &AppState,
	roots: &[String],
	app: Option<AppHandle>,
	options: ScanOptions
) -> Result<usize, String> {
	let app_data_dir = state
		.database
		.db_path()
		.parent()
		.ok_or_else(|| "App data directory is not available".to_string())?;
	let mut combined_scan_result = LibraryScanResult::default();
	let mut progress = ScanProgress::new(app, estimate_scan_work_units(roots).saturating_add(1));
	progress.emit_status("starting", "Preparando reescaneamento da biblioteca...");
	if let Some(tmdb_api_key) = find_tmdb_api_key_file(roots) {
		SettingsModule::save_tmdb_api_key(state, Some(&tmdb_api_key))?;
		progress.emit_status("starting", format!("TMDb API key carregada de {}.", TMDB_API_KEY_FILE_NAME));
	}
	let api_keys = SettingsModule::metadata_api_keys(state)?;

	for root in roots {
		progress.emit_status("root", format!("Lendo raiz {}", root));
		let scan_result = scan_root(Path::new(root), app_data_dir, &MetadataApiKeys {
			tmdb_api_key: api_keys.tmdb_api_key.clone()
		}, &mut progress, options)?;
		combined_scan_result.top_level_count += scan_result.top_level_count;
		combined_scan_result.items.extend(scan_result.items);
		combined_scan_result.directors.extend(scan_result.directors);
		combined_scan_result.director_links.extend(scan_result.director_links);
	}

	progress.emit_status("sync", "Salvando biblioteca no banco...");
	state.database.sync_library(
		&combined_scan_result.items,
		&combined_scan_result.directors,
		&combined_scan_result.director_links
	)?;
	progress.advance("sync", "Biblioteca salva no banco.");
	SettingsModule::set_initial_scan_completed(state)?;
	progress.finish(format!(
		"Biblioteca sincronizada com {} {}.",
		combined_scan_result.top_level_count,
		if combined_scan_result.top_level_count == 1 { "item" } else { "itens" }
	));

	Ok(combined_scan_result.top_level_count)
}

fn find_tmdb_api_key_file(roots: &[String]) -> Option<String> {
	for root in roots {
		let candidate = Path::new(root).join(TMDB_API_KEY_FILE_NAME);
		let Ok(raw) = fs::read_to_string(&candidate) else {
			continue;
		};
		let key = raw
			.lines()
			.map(str::trim)
			.find(|line| !line.is_empty() && !line.starts_with('#'))
			.map(str::to_string);

		if key.is_some() {
			return key;
		}
	}

	None
}

fn scan_root(
	root: &Path,
	app_data_dir: &Path,
	api_keys: &MetadataApiKeys,
	progress: &mut ScanProgress,
	options: ScanOptions
) -> Result<LibraryScanResult, String> {
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
		let flat_result = scan_flat_category(&path, &path, media_type, app_data_dir, api_keys, progress, options)?;
		result.top_level_count += flat_result.items.len();
		result.items.extend(flat_result.items);
		result.directors.extend(flat_result.directors);
		result.director_links.extend(flat_result.director_links);
	}

	let shows_root = root.join("shows");
	let show_result = scan_show_category(&shows_root, &shows_root, app_data_dir, api_keys, progress, options)?;
	result.top_level_count += show_result.series_items.len();
	result.items.extend(show_result.series_items);
	result.items.extend(show_result.episode_items);

	let music_result = scan_music_category(&root.join("music"), &app_data_dir.join("music-covers"), progress)?;
	result.top_level_count += music_result.artist_items.len();
	result.items.extend(music_result.artist_items);
	result.items.extend(music_result.album_items);
	result.items.extend(music_result.track_items);

	Ok(result)
}

fn scan_flat_category(
	category_root: &Path,
	walk_root: &Path,
	media_type: &str,
	app_data_dir: &Path,
	api_keys: &MetadataApiKeys,
	progress: &mut ScanProgress,
	options: ScanOptions
) -> Result<FlatScanResult, String> {
	if !category_root.exists() {
		return Ok(FlatScanResult::default());
	}

	progress.emit_status("scan", format!("Escaneando {}", walk_root.display()));
	let mut result = FlatScanResult::default();

	for entry in WalkDir::new(walk_root).follow_links(true).into_iter().filter_map(Result::ok) {
		let path = entry.path();

		if !entry.file_type().is_file() || !is_video_file(path) {
			continue;
		}

		// Library contract: a movie's Extras/ folder is bonus material, not more movies.
		let in_extras = path
			.strip_prefix(category_root)
			.map(|relative| relative.components().any(|component| component.as_os_str().eq_ignore_ascii_case("extras")))
			.unwrap_or(false);
		if in_extras {
			continue;
		}

		let media_directory = path.parent().unwrap_or(category_root);
		let source_name = file_stem(path);
		let display_name = preferred_flat_display_name(category_root, media_directory, &source_name);
		let (title, year) = parse_contract_item_name(&display_name).unwrap_or_else(|| parse_media_name(&display_name));

		let mut item = MediaItemRecord {
			id: path.to_string_lossy().to_string(),
			title,
			media_type: media_type.to_string(),
			year,
			poster_path: find_companion_file(media_directory, &["poster", "cover"], IMAGE_EXTENSIONS),
			cover_path: find_companion_file(media_directory, &["cover", "poster"], IMAGE_EXTENSIONS),
			overview: None,
			english_title: None,
			original_title: None,
			genre: None,
			director: None,
			actors: None,
			country: None,
			imdb_rating: None,
			imdb_id: None,
			tmdb_id: None,
			media_path: path.to_string_lossy().to_string(),
			subtitle_path: find_video_subtitle(path)
				.or_else(|| find_companion_file(media_directory, &["subtitle", "subtitles"], SUBTITLE_EXTENSIONS))
				.or_else(|| find_any_with_extension(media_directory, SUBTITLE_EXTENSIONS)),
			source_name,
			show_id: None,
			season_number: None,
			episode_number: None
		};

		if matches!(media_type, "movie" | "concert" | "documentary" | "standup") {
			let metadata = MetadataModule::resolve_video_metadata(
				media_directory,
				&item.title,
				item.year,
				media_type,
				item.poster_path.clone(),
				item.cover_path.clone(),
				api_keys,
				app_data_dir,
				true,
				options.force_remote_refresh
			);
			let (directors, director_links) = apply_video_metadata(&mut item, metadata);
			result.directors.extend(directors);
			result.director_links.extend(director_links);
		}

		progress.advance("scan", format!("Processando {}", item.title));
		result.items.push(item);
	}

	Ok(result)
}

fn scan_show_category(
	shows_root: &Path,
	walk_root: &Path,
	app_data_dir: &Path,
	api_keys: &MetadataApiKeys,
	progress: &mut ScanProgress,
	options: ScanOptions
) -> Result<ShowScanResult, String> {
	if !shows_root.exists() {
		return Ok(ShowScanResult::default());
	}

	progress.emit_status("scan", format!("Escaneando {}", walk_root.display()));
	let mut shows = BTreeMap::<String, ShowAggregate>::new();

	for entry in WalkDir::new(walk_root).follow_links(true).into_iter().filter_map(Result::ok) {
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
		let (series_title, year) = parse_contract_item_name(series_folder_name).unwrap_or_else(|| parse_media_name(series_folder_name));
		// Library contract: "Specials" is season 0, "Extras" is EXTRAS_SEASON;
		// contract-named files ("S01E02 - Name") keep their title verbatim.
		let season_number = match season_folder_name.map(str::to_ascii_lowercase).as_deref() {
			Some("specials") => 0,
			Some("extras") => EXTRAS_SEASON,
			_ => season_folder_name.and_then(parse_season_number).unwrap_or(1) as i32
		};
		let (episode_number, episode_title) = if season_number == EXTRAS_SEASON {
			(None, source_name.clone())
		} else if let Some((episode, title)) = parse_contract_episode_name(&source_name) {
			(Some(episode), title)
		} else if let Some(title) = parse_contract_special_name(&source_name) {
			(None, title)
		} else {
			let episode_number = parse_episode_number(&source_name).map(|value| value as i32);
			(episode_number, parse_episode_title(&source_name, episode_number))
		};
		let poster_path = find_nearest_poster(media_directory, shows_root, series_folder_name);
		let cover_path = find_nearest_cover(media_directory, shows_root, series_folder_name);

		let episode_record = MediaItemRecord {
			id: path.to_string_lossy().to_string(),
			title: episode_title,
			media_type: "show_episode".to_string(),
			year,
			poster_path: None,
			cover_path: None,
			overview: None,
			english_title: None,
			original_title: None,
			genre: None,
			director: None,
			actors: None,
			country: None,
			imdb_rating: None,
			imdb_id: None,
			tmdb_id: None,
			media_path: path.to_string_lossy().to_string(),
			// A season folder holds many episodes, so only a subtitle named after
			// this file belongs to it.
			subtitle_path: find_video_subtitle(path),
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
		// Unnumbered entries (specials catalogued as movies, extras) go after the
		// numbered ones, oldest first.
		show.episodes.sort_by(|left, right| {
			left
				.season_number
				.unwrap_or(1)
				.cmp(&right.season_number.unwrap_or(1))
				.then(left.episode_number.unwrap_or(i32::MAX).cmp(&right.episode_number.unwrap_or(i32::MAX)))
				.then(parse_year_in_parentheses(&left.source_name).cmp(&parse_year_in_parentheses(&right.source_name)))
				.then_with(|| left.source_name.cmp(&right.source_name))
		});

		// Specials TMDb lists as movies carry their display title in the show's
		// metadata file ("specials": { "<file name>": { "title": ... } }).
		let special_titles = read_special_titles(Path::new(&show.media_path));
		for episode in show.episodes.iter_mut().filter(|episode| episode.season_number == Some(0)) {
			let file_name = Path::new(&episode.media_path).file_name().and_then(OsStr::to_str).unwrap_or_default();
			if let Some(title) = special_titles.get(file_name) {
				episode.title = title.clone();
			}
		}

		let mut series_item = MediaItemRecord {
			id: show.show_id,
			title: show.title,
			media_type: "show".to_string(),
			year: show.year,
			poster_path: show.poster_path,
			cover_path: show.cover_path,
			overview: None,
			english_title: None,
			original_title: None,
			genre: None,
			director: None,
			actors: None,
			country: None,
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
			api_keys,
			app_data_dir,
			true,
			options.force_remote_refresh
		);
		let _ = apply_video_metadata(&mut series_item, metadata);
		progress.advance("scan", format!("Processando serie {}", series_item.title));
		result.series_items.push(series_item);

		result.episode_items.extend(show.episodes);
	}

	Ok(result)
}

fn apply_video_metadata(item: &mut MediaItemRecord, metadata: VideoMetadata) -> (Vec<DirectorRecord>, Vec<MediaDirectorLinkRecord>) {
	let VideoMetadata {
		year,
		poster_path,
		cover_path,
		overview,
		english_title,
		original_title,
		genre,
		director,
		directors,
		actors,
		country,
		imdb_rating,
		imdb_id,
		tmdb_id
	} = metadata;

	if year.is_some() {
		item.year = year;
	}

	if poster_path.is_some() {
		item.poster_path = poster_path;
	}

	if cover_path.is_some() {
		item.cover_path = cover_path;
	}

	item.english_title = english_title;
	item.original_title = original_title;
	item.overview = overview;

	item.genre = genre;
	item.director = director.clone();
	item.actors = actors;
	item.country = country;
	item.imdb_rating = imdb_rating;
	item.imdb_id = imdb_id;
	item.tmdb_id = tmdb_id;

	collect_director_scan_data(item, director.as_deref(), &directors)
}

fn collect_director_scan_data(
	item: &MediaItemRecord,
	director_text: Option<&str>,
	directors: &[DirectorMetadata]
) -> (Vec<DirectorRecord>, Vec<MediaDirectorLinkRecord>) {
	if item.media_type != "movie" {
		return (Vec::new(), Vec::new());
	}

	let director_candidates = if directors.is_empty() {
		director_text.map(parse_director_names).unwrap_or_default()
	} else {
		directors.to_vec()
	};

	let mut seen_director_ids = BTreeSet::new();
	let mut directors = Vec::new();
	let mut director_links = Vec::new();

	for (index, director) in director_candidates.into_iter().enumerate() {
		let Some(director_id) = build_director_id(&director.name) else {
			continue;
		};

		if !seen_director_ids.insert(director_id.clone()) {
			continue;
		}

		directors.push(DirectorRecord {
			id: director_id.clone(),
			name: director.name,
			tmdb_person_id: director.tmdb_person_id,
			image_path: director.image_path,
			biography: None
		});
		director_links.push(MediaDirectorLinkRecord {
			media_item_id: item.id.clone(),
			director_id,
			ordinal: index as i32
		});
	}

	(directors, director_links)
}

fn parse_director_names(raw: &str) -> Vec<DirectorMetadata> {
	raw
		.split(',')
		.map(str::trim)
		.filter(|name| !name.is_empty())
		.map(|name| DirectorMetadata {
			name: name.to_string(),
			tmdb_person_id: None,
			image_path: None
		})
		.collect()
}

fn build_director_id(name: &str) -> Option<String> {
	let mut normalized = String::with_capacity(name.len());

	for character in name.chars() {
		if character.is_alphanumeric() {
			normalized.extend(character.to_lowercase());
		} else {
			normalized.push('-');
		}
	}

	let normalized = normalized
		.split('-')
		.filter(|segment| !segment.is_empty())
		.collect::<Vec<_>>()
		.join("-");

	(!normalized.is_empty()).then(|| format!("director:{}", normalized))
}

fn scan_music_category(music_root: &Path, cover_cache_root: &Path, progress: &mut ScanProgress) -> Result<MusicScanResult, String> {
	if !music_root.exists() {
		return Ok(MusicScanResult::default());
	}

	progress.emit_status("scan", format!("Escaneando {}", music_root.display()));
	let mut artists = BTreeMap::<String, MusicArtistAggregate>::new();
	let mut scanned_track_count = 0usize;

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
			overview: None,
			english_title: None,
			original_title: None,
			genre: None,
			director: None,
			actors: None,
			country: None,
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
		scanned_track_count += 1;

		if scanned_track_count == 1 || scanned_track_count % 12 == 0 {
			if let Some(current_track) = album_entry.tracks.last() {
				progress.emit_status("scan", format!("Indexando musica {}", current_track.title));
			}
		}
	}

	let mut result = MusicScanResult::default();

	for (_, artist) in artists {
		progress.emit_status("scan", format!("Organizando artista {}", artist.title));
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
			overview: None,
			english_title: None,
			original_title: None,
			genre: None,
			director: None,
			actors: None,
			country: None,
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
				overview: None,
				english_title: None,
				original_title: None,
				genre: None,
				director: None,
				actors: None,
				country: None,
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

fn estimate_scan_work_units(roots: &[String]) -> usize {
	roots.iter().map(|root| estimate_root_work_units(Path::new(root))).sum()
}

fn estimate_root_work_units(root: &Path) -> usize {
	let flat_video_total = [
		root.join("movies"),
		root.join("documentary"),
		root.join("documentaries"),
		root.join("standup"),
		root.join("concerts")
	]
	.into_iter()
	.map(|path| count_video_files(&path))
	.sum::<usize>();
	let show_total = count_show_series(&root.join("shows"));

	flat_video_total + show_total
}

fn count_video_files(root: &Path) -> usize {
	if !root.exists() {
		return 0;
	}

	WalkDir::new(root)
		.follow_links(true)
		.into_iter()
		.filter_map(Result::ok)
		.filter(|entry| entry.file_type().is_file() && is_video_file(entry.path()))
		.count()
}

fn count_show_series(root: &Path) -> usize {
	if !root.exists() {
		return 0;
	}

	let mut series = BTreeSet::new();

	for entry in WalkDir::new(root).follow_links(true).into_iter().filter_map(Result::ok) {
		let path = entry.path();
		if !entry.file_type().is_file() || !is_video_file(path) {
			continue;
		}

		let relative = path.strip_prefix(root).unwrap_or(path);
		let Some(series_folder) = relative.components().next().and_then(|component| component.as_os_str().to_str()) else {
			continue;
		};

		series.insert(series_folder.to_string());
	}

	series.len()
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

pub(crate) fn is_video_file(path: &Path) -> bool {
	has_extension(path, VIDEO_EXTENSIONS)
}

fn is_audio_file(path: &Path) -> bool {
	has_extension(path, AUDIO_EXTENSIONS)
}

pub(crate) fn has_extension(path: &Path, extensions: &[&str]) -> bool {
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

/// "Title (Year)" (the library contract) -> ("Title", Year), title verbatim.
fn parse_contract_item_name(name: &str) -> Option<(String, Option<i32>)> {
	let (title, rest) = name.rsplit_once(" (")?;
	let year = rest.strip_suffix(')')?;
	if year.len() != 4 || !year.chars().all(|character| character.is_ascii_digit()) || title.trim().is_empty() {
		return None;
	}
	Some((title.trim().to_string(), year.parse().ok()))
}

/// The subtitles of one video, `<stem>.<ext>` or `<stem>.<lang>[.<qualifier>].<ext>`
/// (library contract), with their tag (`""`, `"en"`, `"pt-br.sdh"`): the
/// untagged one first, then Portuguese, then the rest.
pub(crate) fn list_video_subtitles(video: &Path) -> Vec<(PathBuf, String)> {
	let (Some(stem), Some(folder)) = (video.file_stem().and_then(OsStr::to_str), video.parent()) else {
		return Vec::new();
	};
	let mut matches = std::fs::read_dir(folder)
		.map(|entries| {
			entries
				.filter_map(Result::ok)
				.map(|entry| entry.path())
				.filter(|path| has_extension(path, SUBTITLE_EXTENSIONS))
				.filter_map(|path| {
					let subtitle_stem = path.file_stem().and_then(OsStr::to_str)?.to_string();
					let tag = if subtitle_stem == stem {
						String::new()
					} else {
						subtitle_stem.strip_prefix(stem)?.strip_prefix('.')?.to_ascii_lowercase()
					};
					let rank = if tag.is_empty() { 0 } else if tag.starts_with("pt") || tag.starts_with("por") { 1 } else { 2 };
					Some((rank, path, tag))
				})
				.collect::<Vec<_>>()
		})
		.unwrap_or_default();
	matches.sort();
	matches.into_iter().map(|(_, path, tag)| (path, tag)).collect()
}

pub(crate) fn find_video_subtitle(video: &Path) -> Option<String> {
	list_video_subtitles(video).into_iter().next().map(|(path, _)| path.to_string_lossy().to_string())
}

/// "S01E02 - Episode Name" (the library contract) -> (2, "Episode Name").
fn parse_contract_episode_name(stem: &str) -> Option<(i32, String)> {
	let (token, title) = stem.split_once(" - ")?;
	let (_, episode) = parse_sxe_token(&token.to_ascii_lowercase())?;
	let title = title.trim();
	(!title.is_empty()).then(|| (episode as i32, title.to_string()))
}

/// "Special - Post COVID (2021)" (a special TMDb lists as a movie) -> "Post COVID".
fn parse_contract_special_name(stem: &str) -> Option<String> {
	let rest = stem.strip_prefix("Special - ")?;
	let title = match find_year_parentheses_range(rest) {
		Some((content_start, _)) => rest[..content_start.saturating_sub(1)].trim(),
		None => rest.trim()
	};
	(!title.is_empty()).then(|| title.to_string())
}

fn read_special_titles(show_root: &Path) -> BTreeMap<String, String> {
	let Ok(raw) = fs::read_to_string(show_root.join(".local-cinema.json")) else {
		return BTreeMap::new();
	};
	let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else {
		return BTreeMap::new();
	};

	json.get("specials")
		.and_then(serde_json::Value::as_object)
		.map(|specials| {
			specials
				.iter()
				.filter_map(|(file_name, entry)| Some((file_name.clone(), entry.get("title")?.as_str()?.to_string())))
				.collect()
		})
		.unwrap_or_default()
}

pub(crate) fn parse_media_name(raw: &str) -> (String, Option<i32>) {
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

pub(crate) fn parse_season_number(raw: &str) -> Option<u32> {
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

pub(crate) fn parse_episode_number(raw: &str) -> Option<u32> {
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

/// Season number of a show's "Extras" folder (bonus material, not episodes).
pub const EXTRAS_SEASON: i32 = -1;
pub(crate) const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "mov", "wmv", "m4v", "webm", "m2ts", "ts", "mpg", "mpeg"];
const AUDIO_EXTENSIONS: &[&str] = &["flac", "mp3", "m4a", "aac", "ogg", "opus", "wav", "wma"];
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "jfif", "png", "webp", "gif", "bmp", "avif"];
pub(crate) const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa", "vtt", "sub"];
const CACHED_IMAGE_EXTENSIONS: &[&str] = &["jpg", "png", "gif", "webp", "bmp"];
