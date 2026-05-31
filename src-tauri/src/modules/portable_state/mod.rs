use crate::app::state::AppState;
use crate::infrastructure::db::MediaItemRecord;
use crate::modules::library::LibraryModule;
use crate::modules::settings::SettingsModule;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const LOCAL_CINEMA_DIR_NAME: &str = ".local-cinema";
const LIBRARY_STATE_FILE_NAME: &str = ".local-cinema-state.json";
const LIBRARY_STATE_VERSION: u32 = 1;

pub struct PortableStateModule;

#[derive(Debug, Clone)]
pub struct PortableNewItemRecord {
	pub item: MediaItemRecord,
	pub added_at: i64
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortableLibraryState {
	#[serde(default = "default_library_state_version")]
	version: u32,
	#[serde(default)]
	new_items: Vec<PortableNewItemState>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortableNewItemState {
	relative_path: String,
	media_type: String,
	added_at: i64
}

impl PortableStateModule {
	pub fn list_new_items(state: &AppState) -> Result<Vec<PortableNewItemRecord>, String> {
		let roots = SettingsModule::library_root_paths(state)?;
		if roots.is_empty() {
			return Ok(Vec::new());
		}

		let items_by_key = build_current_item_index(state, &roots)?;
		let mut records = Vec::new();

		for root in &roots {
			let library_state = read_library_state(Path::new(root))?;

			for entry in library_state.new_items {
				let key = build_state_key(root, &entry.relative_path);
				if let Some(item) = items_by_key.get(&key) {
					records.push(PortableNewItemRecord {
						item: item.clone(),
						added_at: entry.added_at
					});
				}
			}
		}

		records.sort_by(|left, right| right.added_at.cmp(&left.added_at));
		Ok(records)
	}

	pub fn upsert_new_item(state: &AppState, media_path: &str, media_type: &str, added_at: i64) -> Result<(), String> {
		if !supports_new_items(media_type) {
			return Ok(());
		}

		let roots = SettingsModule::library_root_paths(state)?;
		let Some((root_path, relative_path)) = resolve_root_and_relative_path(&roots, media_path) else {
			return Err("Nao foi possivel relacionar a midia com uma biblioteca configurada.".to_string());
		};
		let mut library_state = read_library_state(&root_path)?;

		library_state.new_items.retain(|entry| entry.relative_path != relative_path);
		library_state.new_items.push(PortableNewItemState {
			relative_path,
			media_type: media_type.to_string(),
			added_at
		});
		library_state.new_items.sort_by(|left, right| right.added_at.cmp(&left.added_at));
		write_library_state(&root_path, &library_state)
	}

	pub fn remove_new_item(state: &AppState, media_path: &str) -> Result<(), String> {
		let roots = SettingsModule::library_root_paths(state)?;
		let Some((root_path, relative_path)) = resolve_root_and_relative_path(&roots, media_path) else {
			return Ok(());
		};
		let mut library_state = read_library_state(&root_path)?;
		let original_len = library_state.new_items.len();
		library_state.new_items.retain(|entry| entry.relative_path != relative_path);

		if library_state.new_items.len() == original_len {
			return Ok(());
		}

		write_library_state(&root_path, &library_state)
	}
}

fn build_current_item_index(state: &AppState, roots: &[String]) -> Result<HashMap<String, MediaItemRecord>, String> {
	let mut items_by_key = HashMap::new();

	for item in LibraryModule::list_media(state)? {
		if !supports_new_items(&item.media_type) {
			continue;
		}

		let Some((root_path, relative_path)) = resolve_root_and_relative_path(roots, &item.media_path) else {
			continue;
		};

		items_by_key.insert(build_state_key(root_path.to_string_lossy().as_ref(), &relative_path), item);
	}

	Ok(items_by_key)
}

fn read_library_state(root_path: &Path) -> Result<PortableLibraryState, String> {
	let state_path = library_state_path(root_path);
	if !state_path.exists() {
		let legacy_state_path = legacy_library_state_path(root_path);
		if legacy_state_path.exists() {
			let raw = fs::read_to_string(&legacy_state_path).map_err(|error| error.to_string())?;
			let mut state = serde_json::from_str::<PortableLibraryState>(&raw).map_err(|error| error.to_string())?;
			state.version = LIBRARY_STATE_VERSION;
			state.new_items.retain(|entry| !entry.relative_path.trim().is_empty());
			write_library_state(root_path, &state)?;
			let _ = fs::remove_file(&legacy_state_path);
			return Ok(state);
		}

		return Ok(PortableLibraryState {
			version: LIBRARY_STATE_VERSION,
			new_items: Vec::new()
		});
	}

	let raw = fs::read_to_string(&state_path).map_err(|error| error.to_string())?;
	let mut state = serde_json::from_str::<PortableLibraryState>(&raw).map_err(|error| error.to_string())?;
	state.version = LIBRARY_STATE_VERSION;
	state.new_items.retain(|entry| !entry.relative_path.trim().is_empty());
	Ok(state)
}

fn write_library_state(root_path: &Path, state: &PortableLibraryState) -> Result<(), String> {
	let state_path = library_state_path(root_path);
	let legacy_state_path = legacy_library_state_path(root_path);

	if state.new_items.is_empty() {
		if state_path.exists() {
			fs::remove_file(state_path).map_err(|error| error.to_string())?;
		}
		if legacy_state_path.exists() {
			fs::remove_file(legacy_state_path).map_err(|error| error.to_string())?;
		}
		return Ok(());
	}

	if let Some(state_parent) = state_path.parent() {
		fs::create_dir_all(state_parent).map_err(|error| error.to_string())?;
	}

	let json = serde_json::to_string_pretty(state).map_err(|error| error.to_string())?;
	fs::write(state_path, json).map_err(|error| error.to_string())
}

fn resolve_root_and_relative_path(roots: &[String], media_path: &str) -> Option<(PathBuf, String)> {
	let media = Path::new(media_path);

	roots
		.iter()
		.filter_map(|root| {
			let root_path = Path::new(root);
			let relative = media.strip_prefix(root_path).ok()?;
			Some((root_path.to_path_buf(), normalize_relative_path(relative)))
		})
		.max_by_key(|(root_path, _)| root_path.as_os_str().len())
}

fn normalize_relative_path(path: &Path) -> String {
	path
		.to_string_lossy()
		.replace('\\', "/")
		.trim_start_matches('/')
		.to_string()
}

fn library_state_path(root_path: &Path) -> PathBuf {
	root_path.join(LOCAL_CINEMA_DIR_NAME).join(LIBRARY_STATE_FILE_NAME)
}

fn legacy_library_state_path(root_path: &Path) -> PathBuf {
	root_path.join(LIBRARY_STATE_FILE_NAME)
}

fn build_state_key(root: &str, relative_path: &str) -> String {
	format!("{}|{}", root.to_ascii_lowercase(), relative_path.to_ascii_lowercase())
}

fn supports_new_items(media_type: &str) -> bool {
	matches!(media_type, "movie" | "show" | "concert" | "standup" | "documentary")
}

fn default_library_state_version() -> u32 {
	LIBRARY_STATE_VERSION
}
