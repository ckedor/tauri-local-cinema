use crate::app::state::AppState;
use crate::contracts::player::{PlayerRectDto, PlayerSessionDto, PlayerStatusDto};
use crate::modules::library::LibraryModule;
use crate::modules::scanner::ScannerModule;
use crate::modules::settings::SettingsModule;
use rfd::FileDialog;
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn ping() -> &'static str {
  "pong"
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialSetupStatusDto {
  pub library_root_path: Option<String>,
  pub library_root_paths: Vec<String>,
  pub has_library_root: bool,
  pub media_count: usize
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItemDto {
  pub id: String,
  pub title: String,
  pub media_type: String,
  pub year: Option<i32>,
  pub poster_path: Option<String>,
  pub cover_path: Option<String>,
  pub genre: Option<String>,
  pub director: Option<String>,
  pub actors: Option<String>,
  pub imdb_rating: Option<String>,
  pub media_path: String,
  pub subtitle_path: Option<String>
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowEpisodeDto {
  pub id: String,
  pub season_number: i32,
  pub episode_number: Option<i32>,
  pub title: String,
  pub media_path: String,
  pub subtitle_path: Option<String>
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowSeasonDto {
  pub season_number: i32,
  pub episodes: Vec<ShowEpisodeDto>
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowDetailDto {
  pub show_id: String,
  pub seasons: Vec<ShowSeasonDto>
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicArtistDetailDto {
  pub artist_id: String,
  pub albums: Vec<MediaItemDto>
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicTrackDto {
  pub id: String,
  pub track_number: Option<i32>,
  pub title: String,
  pub media_path: String,
  pub poster_path: Option<String>
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicAlbumDetailDto {
  pub album_id: String,
  pub tracks: Vec<MusicTrackDto>
}

#[tauri::command]
pub fn get_initial_setup_status(state: State<'_, AppState>) -> Result<InitialSetupStatusDto, String> {
  let library_root_paths = SettingsModule::library_root_paths(&state)?;
  let media_count = LibraryModule::list_media(&state)?.len();

  Ok(InitialSetupStatusDto {
    library_root_path: library_root_paths.first().cloned(),
    has_library_root: !library_root_paths.is_empty(),
    library_root_paths,
    media_count
  })
}

#[tauri::command]
pub fn pick_library_root() -> Option<String> {
  FileDialog::new()
    .pick_folder()
    .map(|path| path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn save_library_root(state: State<'_, AppState>, path: String) -> Result<(), String> {
  SettingsModule::save_library_root_path(&state, &path)
}

#[tauri::command]
pub fn save_library_roots(state: State<'_, AppState>, paths: Vec<String>) -> Result<(), String> {
  SettingsModule::save_library_root_paths(&state, &paths)
}

#[tauri::command]
pub fn start_initial_scan(state: State<'_, AppState>) -> Result<usize, String> {
  ScannerModule::rescan_library(&state)
}

#[tauri::command]
pub fn rescan_library(state: State<'_, AppState>) -> Result<usize, String> {
  ScannerModule::rescan_library(&state)
}

#[tauri::command]
pub fn reset_library_database_and_rescan(state: State<'_, AppState>) -> Result<usize, String> {
  ScannerModule::rescan_library(&state)
}

#[tauri::command]
pub fn list_home_media(state: State<'_, AppState>) -> Result<Vec<MediaItemDto>, String> {
  LibraryModule::list_media(&state).map(|items| {
    items.into_iter().map(to_media_item_dto).collect()
  })
}

#[tauri::command]
pub fn get_media_item(state: State<'_, AppState>, media_id: String) -> Result<Option<MediaItemDto>, String> {
  LibraryModule::get_media_item(&state, &media_id).map(|item| item.map(to_media_item_dto))
}

#[tauri::command]
pub fn get_show_detail(state: State<'_, AppState>, show_id: String) -> Result<ShowDetailDto, String> {
  let episodes = LibraryModule::list_show_episodes(&state, &show_id)?;
  let mut seasons = BTreeMap::<i32, Vec<ShowEpisodeDto>>::new();

  for episode in episodes {
    let season_number = episode.season_number.unwrap_or(1);
    seasons.entry(season_number).or_default().push(ShowEpisodeDto {
      id: episode.id,
      season_number,
      episode_number: episode.episode_number,
      title: episode.title,
      media_path: episode.media_path,
      subtitle_path: episode.subtitle_path
    });
  }

  Ok(ShowDetailDto {
    show_id,
    seasons: seasons
      .into_iter()
      .map(|(season_number, episodes)| ShowSeasonDto {
        season_number,
        episodes
      })
      .collect()
  })
}

#[tauri::command]
pub fn get_music_artist_detail(
  state: State<'_, AppState>,
  artist_id: String
) -> Result<MusicArtistDetailDto, String> {
  let albums = LibraryModule::list_music_albums(&state, &artist_id)?;

  Ok(MusicArtistDetailDto {
    artist_id,
    albums: albums.into_iter().map(to_media_item_dto).collect()
  })
}

#[tauri::command]
pub fn list_music_albums(state: State<'_, AppState>) -> Result<Vec<MediaItemDto>, String> {
  LibraryModule::list_all_music_albums(&state).map(|items| items.into_iter().map(to_media_item_dto).collect())
}

#[tauri::command]
pub fn get_music_album_detail(
  state: State<'_, AppState>,
  album_id: String
) -> Result<MusicAlbumDetailDto, String> {
  let tracks = LibraryModule::list_music_tracks(&state, &album_id)?;

  Ok(MusicAlbumDetailDto {
    album_id,
    tracks: tracks
      .into_iter()
      .map(|track| MusicTrackDto {
        id: track.id,
        track_number: track.season_number,
        title: track.title,
        media_path: track.media_path,
        poster_path: track.poster_path
      })
      .collect()
  })
}

fn to_media_item_dto(item: crate::infrastructure::db::MediaItemRecord) -> MediaItemDto {
  MediaItemDto {
    id: item.id,
    title: item.title,
    media_type: item.media_type,
    year: item.year,
    poster_path: item.poster_path,
    cover_path: item.cover_path,
    genre: item.genre,
    director: item.director,
    actors: item.actors,
    imdb_rating: item.imdb_rating,
    media_path: item.media_path,
    subtitle_path: item.subtitle_path
  }
}

// ---- Embedded player commands -------------------------------------------------

#[tauri::command]
pub fn player_load(
  app: AppHandle,
  state: State<'_, AppState>,
  session: PlayerSessionDto,
  rect: PlayerRectDto
) -> Result<PlayerStatusDto, String> {
  let mut engine = state.player.lock().map_err(|_| "player lock".to_string())?;
  engine.load(&app, session, rect)?;
  Ok(engine.status())
}

#[tauri::command]
pub fn player_set_rect(state: State<'_, AppState>, rect: PlayerRectDto) -> Result<(), String> {
  let mut engine = state.player.lock().map_err(|_| "player lock".to_string())?;
  engine.set_rect(rect);
  Ok(())
}

#[tauri::command]
pub fn player_toggle_pause(state: State<'_, AppState>) -> Result<PlayerStatusDto, String> {
  let mut engine = state.player.lock().map_err(|_| "player lock".to_string())?;
  engine.toggle_pause()?;
  Ok(engine.status())
}

#[tauri::command]
pub fn player_set_paused(state: State<'_, AppState>, paused: bool) -> Result<PlayerStatusDto, String> {
  let mut engine = state.player.lock().map_err(|_| "player lock".to_string())?;
  engine.set_paused(paused)?;
  Ok(engine.status())
}

#[tauri::command]
pub fn player_set_volume(state: State<'_, AppState>, volume_percent: f64) -> Result<PlayerStatusDto, String> {
  let mut engine = state.player.lock().map_err(|_| "player lock".to_string())?;
  engine.set_volume(volume_percent)?;
  Ok(engine.status())
}

#[tauri::command]
pub fn player_set_muted(state: State<'_, AppState>, muted: bool) -> Result<PlayerStatusDto, String> {
  let mut engine = state.player.lock().map_err(|_| "player lock".to_string())?;
  engine.set_muted(muted)?;
  Ok(engine.status())
}

#[tauri::command]
pub fn player_seek(state: State<'_, AppState>, seconds: f64) -> Result<(), String> {
  let mut engine = state.player.lock().map_err(|_| "player lock".to_string())?;
  engine.seek(seconds)
}

#[tauri::command]
pub fn player_seek_to(state: State<'_, AppState>, seconds: f64) -> Result<(), String> {
  let mut engine = state.player.lock().map_err(|_| "player lock".to_string())?;
  engine.seek_to(seconds)
}

#[tauri::command]
pub fn player_stop(state: State<'_, AppState>) -> Result<PlayerStatusDto, String> {
  let mut engine = state.player.lock().map_err(|_| "player lock".to_string())?;
  engine.stop();
  Ok(engine.status())
}

#[tauri::command]
pub fn player_status(state: State<'_, AppState>) -> Result<PlayerStatusDto, String> {
  let engine = state.player.lock().map_err(|_| "player lock".to_string())?;
  Ok(engine.status())
}
