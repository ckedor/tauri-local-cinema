use crate::app::state::AppState;
use crate::infrastructure::db::MediaItemRecord;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const SUPPORTED_ARTWORK_EXTENSIONS: &[&str] = &["jpg", "jpeg", "jfif", "png", "webp", "gif", "bmp", "avif"];

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
