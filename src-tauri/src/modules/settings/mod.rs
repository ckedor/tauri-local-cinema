use crate::app::state::AppState;

pub struct SettingsModule;

impl SettingsModule {
	pub fn library_root_path(state: &AppState) -> Result<Option<String>, String> {
		state.database.get_setting("library_root_path")
	}

	pub fn save_library_root_path(state: &AppState, path: &str) -> Result<(), String> {
		state.database.set_setting("library_root_path", path)
	}

	pub fn set_initial_scan_completed(state: &AppState) -> Result<(), String> {
		state.database.set_setting("initial_scan_completed_at", &format!("{}", chrono_like_now()))
	}
}

fn chrono_like_now() -> i64 {
	use std::time::{SystemTime, UNIX_EPOCH};

	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|duration| duration.as_millis() as i64)
		.unwrap_or_default()
}
