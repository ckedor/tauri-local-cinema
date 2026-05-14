use crate::app::state::AppState;

pub struct SettingsModule;

const LIBRARY_ROOT_PATHS_KEY: &str = "library_root_paths";
const LEGACY_LIBRARY_ROOT_PATH_KEY: &str = "library_root_path";

impl SettingsModule {
	pub fn library_root_path(state: &AppState) -> Result<Option<String>, String> {
		Ok(Self::library_root_paths(state)?.into_iter().next())
	}

	pub fn library_root_paths(state: &AppState) -> Result<Vec<String>, String> {
		if let Some(raw) = state.database.get_setting(LIBRARY_ROOT_PATHS_KEY)? {
			if raw.trim().is_empty() {
				return Ok(Vec::new());
			}

			let paths = serde_json::from_str::<Vec<String>>(&raw).map_err(|error| error.to_string())?;
			return Ok(normalize_library_root_paths(paths));
		}

		Ok(normalize_library_root_paths(
			state
				.database
				.get_setting(LEGACY_LIBRARY_ROOT_PATH_KEY)?
				.into_iter()
				.collect()
		))
	}

	pub fn save_library_root_path(state: &AppState, path: &str) -> Result<(), String> {
		Self::save_library_root_paths(state, &[path.to_string()])
	}

	pub fn save_library_root_paths(state: &AppState, paths: &[String]) -> Result<(), String> {
		let normalized_paths = normalize_library_root_paths(paths.to_vec());
		let raw_paths = serde_json::to_string(&normalized_paths).map_err(|error| error.to_string())?;

		state.database.set_setting(LIBRARY_ROOT_PATHS_KEY, &raw_paths)?;
		state.database.set_setting(
			LEGACY_LIBRARY_ROOT_PATH_KEY,
			normalized_paths.first().map(String::as_str).unwrap_or("")
		)
	}

	pub fn set_initial_scan_completed(state: &AppState) -> Result<(), String> {
		state.database.set_setting("initial_scan_completed_at", &format!("{}", chrono_like_now()))
	}
}

fn normalize_library_root_paths(paths: Vec<String>) -> Vec<String> {
	let mut normalized: Vec<String> = Vec::new();

	for path in paths {
		let trimmed = path.trim();
		if trimmed.is_empty() {
			continue;
		}

		if normalized.iter().any(|existing| existing.eq_ignore_ascii_case(trimmed)) {
			continue;
		}

		normalized.push(trimmed.to_string());
	}

	normalized
}

fn chrono_like_now() -> i64 {
	use std::time::{SystemTime, UNIX_EPOCH};

	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|duration| duration.as_millis() as i64)
		.unwrap_or_default()
}
