use crate::app::state::AppState;
use crate::infrastructure::db::{DirectorListRecord, DirectorRecord, MediaItemRecord};
use crate::modules::settings::SettingsModule;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::fs;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

const SUPPORTED_ARTWORK_EXTENSIONS: &[&str] = &["jpg", "jpeg", "jfif", "png", "webp", "gif", "bmp", "avif"];
const DIRECTOR_IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp", "avif"];

#[derive(Debug, Deserialize)]
struct TmdbPersonSearchResponse {
	#[serde(default)]
	results: Vec<TmdbPersonSearchResult>
}

#[derive(Debug, Deserialize)]
struct TmdbPersonSearchResult {
	id: i64,
	name: String,
	#[serde(default)]
	profile_path: Option<String>,
	#[serde(default)]
	popularity: Option<f64>
}

pub struct LibraryModule;

impl LibraryModule {
	pub fn list_media(state: &AppState) -> Result<Vec<MediaItemRecord>, String> {
		state
			.database
			.list_media_items()
			.map(|items| items.into_iter().map(normalize_artwork_paths).collect())
	}

	pub fn get_media_item(state: &AppState, media_id: &str) -> Result<Option<MediaItemRecord>, String> {
		state.database.get_media_item(media_id).map(|item| item.map(normalize_artwork_paths))
	}

	pub fn list_show_episodes(state: &AppState, show_id: &str) -> Result<Vec<MediaItemRecord>, String> {
		state.database.list_show_episode_items(show_id)
	}

	pub fn list_music_albums(state: &AppState, artist_id: &str) -> Result<Vec<MediaItemRecord>, String> {
		state.database.list_music_album_items(artist_id)
	}

	pub fn list_all_music_albums(state: &AppState) -> Result<Vec<MediaItemRecord>, String> {
		state.database.list_all_music_album_items()
	}

	pub fn list_music_tracks(state: &AppState, album_id: &str) -> Result<Vec<MediaItemRecord>, String> {
		state.database.list_music_track_items(album_id)
	}

	pub fn list_movie_directors(state: &AppState) -> Result<Vec<DirectorListRecord>, String> {
		let roots = SettingsModule::library_root_paths(state)?;
		let tmdb_api_key = SettingsModule::metadata_api_keys(state)?
			.tmdb_api_key
			.filter(|value| !value.trim().is_empty());
		let http_client = tmdb_api_key.as_deref().and_then(build_http_client);
		let app_data_dir = state
			.database
			.db_path()
			.parent()
			.ok_or_else(|| "App data directory is not available".to_string())?;
		let mut directors = state.database.list_movie_directors()?;
		let portable_director_cache_root = roots
			.first()
			.map(PathBuf::from)
			.map(|root| root.join(".local-cinema").join("people").join("directors"));

		for director in &mut directors {
			let mut resolved_tmdb_person_id = director.tmdb_person_id.clone();
			let mut resolved_path = resolve_director_image_path(
				director.image_path.as_deref(),
				resolved_tmdb_person_id.as_deref(),
				&roots,
				app_data_dir
			);

			if resolved_path.is_none() {
				if let (Some(client), Some(api_key), Some(cache_root)) = (
					http_client.as_ref(),
					tmdb_api_key.as_deref(),
					portable_director_cache_root.as_deref()
				) {
					if let Some((person_id, downloaded_path)) =
						download_director_image_from_tmdb(client, api_key, &director.name, cache_root)
					{
						resolved_tmdb_person_id = Some(person_id);
						resolved_path = Some(downloaded_path);
					}
				}
			}

			if resolved_path != director.image_path || resolved_tmdb_person_id != director.tmdb_person_id {
				state
					.database
					.update_director_tmdb_and_image(&director.id, resolved_tmdb_person_id.as_deref(), resolved_path.as_deref())?;
				director.tmdb_person_id = resolved_tmdb_person_id;
				director.image_path = resolved_path;
			}
		}

		Ok(directors)
	}

	pub fn get_director(state: &AppState, director_id: &str) -> Result<Option<DirectorRecord>, String> {
		let mut director = state.database.get_director(director_id)?;

		if let Some(record) = &mut director {
			let roots = SettingsModule::library_root_paths(state)?;
			let app_data_dir = state
				.database
				.db_path()
				.parent()
				.ok_or_else(|| "App data directory is not available".to_string())?;
			let resolved_path = resolve_director_image_path(
				record.image_path.as_deref(),
				record.tmdb_person_id.as_deref(),
				&roots,
				app_data_dir
			);

			if resolved_path != record.image_path {
				state
					.database
					.update_director_image_path(&record.id, resolved_path.as_deref())?;
				record.image_path = resolved_path;
			}
		}

		Ok(director)
	}

	pub fn list_director_movies(state: &AppState, director_id: &str) -> Result<Vec<MediaItemRecord>, String> {
		state
			.database
			.list_director_movie_items(director_id)
			.map(|items| items.into_iter().map(normalize_artwork_paths).collect())
	}
}

fn normalize_artwork_paths(mut item: MediaItemRecord) -> MediaItemRecord {
	if !matches!(item.media_type.as_str(), "movie" | "show" | "concert" | "documentary" | "standup") {
		return item;
	}

	let Some(artwork_directory) = resolve_artwork_directory(&item.media_path) else {
		return item;
	};

	let explicit_poster = find_named_image(&artwork_directory, &["poster"]);
	let explicit_cover = find_named_image(&artwork_directory, &["cover"]);

	item.poster_path = explicit_poster
		.clone()
		.or(item.poster_path.clone())
		.or(explicit_cover.clone())
		.or(item.cover_path.clone());

	item.cover_path = explicit_cover
		.clone()
		.or(item.cover_path.clone())
		.or(explicit_poster)
		.or(item.poster_path.clone());

	item
}

fn resolve_artwork_directory(media_path: &str) -> Option<PathBuf> {
	let path = Path::new(media_path);

	if path.is_dir() {
		return Some(path.to_path_buf());
	}

	path.parent().map(Path::to_path_buf)
}

fn find_named_image(directory: &Path, names: &[&str]) -> Option<String> {
	let entries = std::fs::read_dir(directory).ok()?;

	for entry in entries.filter_map(Result::ok) {
		let path = entry.path();
		let stem = path.file_stem().and_then(OsStr::to_str)?;
		let extension = path.extension().and_then(OsStr::to_str)?;
		let normalized_extension = extension.to_ascii_lowercase();

		if !SUPPORTED_ARTWORK_EXTENSIONS.contains(&normalized_extension.as_str()) {
			continue;
		}

		if names.iter().any(|name| stem.eq_ignore_ascii_case(name)) {
			return Some(path.to_string_lossy().to_string());
		}
	}

	None
}

fn resolve_director_image_path(
	current_image_path: Option<&str>,
	tmdb_person_id: Option<&str>,
	roots: &[String],
	app_data_dir: &Path
) -> Option<String> {
	if let Some(path) = current_image_path.filter(|path| !path.trim().is_empty()) {
		if Path::new(path).exists() {
			return Some(path.to_string());
		}
	}

	let Some(tmdb_person_id) = tmdb_person_id.filter(|value| !value.trim().is_empty()) else {
		return None;
	};

	for root in roots {
		let portable_root = Path::new(root).join(".local-cinema").join("people").join("directors");
		if let Some(path) = find_director_image_by_tmdb_id(&portable_root, tmdb_person_id) {
			return Some(path);
		}
	}

	let legacy_root = app_data_dir.join("people").join("directors");
	find_director_image_by_tmdb_id(&legacy_root, tmdb_person_id)
}

fn find_director_image_by_tmdb_id(root: &Path, tmdb_person_id: &str) -> Option<String> {
	for extension in DIRECTOR_IMAGE_EXTENSIONS {
		let candidate = root.join(format!("{}.{}", tmdb_person_id, extension));
		if candidate.exists() {
			return Some(candidate.to_string_lossy().to_string());
		}
	}

	None
}

fn build_http_client(_: &str) -> Option<Client> {
	Client::builder()
		.timeout(Duration::from_secs(4))
		.user_agent("tauri-local-cinema/0.1")
		.build()
		.ok()
}

fn download_director_image_from_tmdb(
	client: &Client,
	api_key: &str,
	director_name: &str,
	cache_root: &Path
) -> Option<(String, String)> {
	let search_response = client
		.get("https://api.themoviedb.org/3/search/person")
		.query(&[
			("api_key", api_key),
			("query", director_name),
			("include_adult", "false"),
			("language", "en-US")
		])
		.send()
		.ok()?
		.error_for_status()
		.ok()?
		.json::<TmdbPersonSearchResponse>()
		.ok()?;

	let best_match = search_response
		.results
		.into_iter()
		.filter(|candidate| candidate.profile_path.is_some())
		.max_by(|left, right| {
			let left_exact = left.name.eq_ignore_ascii_case(director_name);
			let right_exact = right.name.eq_ignore_ascii_case(director_name);

			left_exact
				.cmp(&right_exact)
				.then_with(|| {
					left
						.popularity
						.partial_cmp(&right.popularity)
						.unwrap_or(std::cmp::Ordering::Equal)
				})
		})?;

	let profile_path = best_match.profile_path?;
	let image_url = format!("https://image.tmdb.org/t/p/w500{}", profile_path);
	let extension = infer_image_extension(&image_url);

	fs::create_dir_all(cache_root).ok()?;
	let image_path = cache_root.join(format!("{}.{}", best_match.id, extension));

	if !image_path.exists() {
		let image_bytes = client
			.get(image_url)
			.send()
			.ok()?
			.error_for_status()
			.ok()?
			.bytes()
			.ok()?;
		fs::write(&image_path, &image_bytes).ok()?;
	}

	Some((best_match.id.to_string(), image_path.to_string_lossy().to_string()))
}

fn infer_image_extension(raw_url: &str) -> &'static str {
	let normalized = raw_url.split('?').next().unwrap_or(raw_url).to_ascii_lowercase();

	if normalized.ends_with(".png") {
		return "png";
	}

	if normalized.ends_with(".webp") {
		return "webp";
	}

	"jpg"
}
