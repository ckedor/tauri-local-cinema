use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSessionDto {
  pub media_id: String,
  pub media_path: String,
  pub media_title: String,
  pub subtitle_path: Option<String>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerRectDto {
  pub x: i32,
  pub y: i32,
  pub width: i32,
  pub height: i32
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStatusDto {
  pub session: Option<PlayerSessionDto>,
  pub is_playing: bool,
  pub is_paused: bool,
  pub position_sec: f64,
  pub duration_sec: f64,
  pub volume_percent: f64,
  pub is_muted: bool,
  pub last_error: Option<String>
}
