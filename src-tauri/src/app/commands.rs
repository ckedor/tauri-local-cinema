use crate::app::state::AppState;
use crate::contracts::player::{PlayerRectDto, PlayerSessionDto, PlayerStatusDto, SubtitleFileDto, SubtitleTrackDto};
use crate::modules::importer::{ImportIdentity, ImportPlanDto, ImporterModule};
use crate::modules::refresh::subtitles::{self, SeasonSubtitlesDto, SubtitleDownload, SubtitleDownloadResultDto};
use crate::modules::refresh::sync::SyncSummary;
use crate::modules::refresh::{RefreshModule, RefreshPlanDto, RefreshResultDto, RefreshSelection};
use crate::modules::settings::SubtitleSettings;
use crate::modules::library::LibraryModule;
use crate::modules::portable_state::PortableStateModule;
use crate::modules::scanner::ScannerModule;
use crate::modules::settings::SettingsModule;
use rfd::FileDialog;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use tauri::{AppHandle, State};

// Commands marked (async) run on a worker thread. Plain sync commands run on the
// main (GTK/UI) thread: anything that reads the disk or the database there
// freezes the window until it returns, which on Linux, with a library on a USB
// drive, can be minutes. Only the file dialog, exit and the player (its native
// window lives on the main thread on Windows) stay sync.

#[tauri::command]
pub fn ping() -> &'static str {
  "pong"
}

#[tauri::command]
pub fn exit_app(app: AppHandle) {
  app.exit(0);
}

#[tauri::command]
pub fn open_media_in_explorer(media_path: String) -> Result<(), String> {
  let resolved_path = resolve_media_explorer_path(&media_path)?;

  #[cfg(target_os = "windows")]
  {
    Command::new("explorer")
      .arg(&resolved_path)
      .spawn()
      .map_err(|error| error.to_string())?;
  }

  #[cfg(target_os = "macos")]
  {
    Command::new("open")
      .arg(&resolved_path)
      .spawn()
      .map_err(|error| error.to_string())?;
  }

  #[cfg(all(unix, not(target_os = "macos")))]
  {
    Command::new("xdg-open")
      .arg(&resolved_path)
      .spawn()
      .map_err(|error| error.to_string())?;
  }

  Ok(())
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
pub struct MetadataApiSettingsDto {
  pub tmdb_api_key: Option<String>,
  pub anthropic_api_key: Option<String>
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
  pub overview: Option<String>,
  pub english_title: Option<String>,
  pub original_title: Option<String>,
  pub genre: Option<String>,
  pub director: Option<String>,
  pub actors: Option<String>,
  pub country: Option<String>,
  pub imdb_rating: Option<String>,
  pub media_path: String,
  pub subtitle_path: Option<String>
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewItemEntryDto {
  #[serde(flatten)]
  pub item: MediaItemDto,
  pub added_at: i64
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorDto {
  pub id: String,
  pub name: String,
  pub image_path: Option<String>,
  pub movie_count: i64
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovieDirectorDetailDto {
  pub director_id: String,
  pub name: String,
  pub image_path: Option<String>,
  pub movies: Vec<MediaItemDto>
}

#[tauri::command(async)]
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
pub fn pick_import_files() -> Vec<String> {
  FileDialog::new()
    .add_filter("Video e legenda", &["mkv", "mp4", "avi", "mov", "wmv", "m4v", "webm", "srt", "ass", "ssa", "vtt", "sub"])
    .pick_files()
    .unwrap_or_default()
    .into_iter()
    .map(|path| path.to_string_lossy().to_string())
    .collect()
}

#[tauri::command(async)]
pub fn save_library_root(state: State<'_, AppState>, path: String) -> Result<(), String> {
  SettingsModule::save_library_root_path(&state, &path)
}

#[tauri::command(async)]
pub fn save_library_roots(state: State<'_, AppState>, paths: Vec<String>) -> Result<(), String> {
  SettingsModule::save_library_root_paths(&state, &paths)
}

#[tauri::command(async)]
pub fn get_metadata_api_settings(state: State<'_, AppState>) -> Result<MetadataApiSettingsDto, String> {
  let settings = SettingsModule::metadata_api_keys(&state)?;

  Ok(MetadataApiSettingsDto {
    tmdb_api_key: settings.tmdb_api_key,
    anthropic_api_key: settings.anthropic_api_key
  })
}

#[tauri::command(async)]
pub fn save_metadata_api_settings(
  state: State<'_, AppState>,
  tmdb_api_key: Option<String>,
  anthropic_api_key: Option<String>
) -> Result<(), String> {
  SettingsModule::save_metadata_api_keys(&state, tmdb_api_key.as_deref(), anthropic_api_key.as_deref())
}

// Scans fetch TMDb metadata with blocking HTTP, which panics on the async
// runtime, so they run on the blocking pool.

#[tauri::command]
pub async fn start_initial_scan(app: AppHandle, state: State<'_, AppState>) -> Result<usize, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || ScannerModule::rescan_library(&state, Some(app)))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn rescan_library(app: AppHandle, state: State<'_, AppState>) -> Result<usize, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || ScannerModule::rescan_library(&state, Some(app)))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn reset_library_database_and_rescan(app: AppHandle, state: State<'_, AppState>) -> Result<usize, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || ScannerModule::reset_and_scan_library(&state, Some(app)))
    .await
    .map_err(|error| error.to_string())?
}

// The import commands call TMDb and Anthropic with blocking HTTP, which
// panics on the async runtime (and the UI then waits forever), so they run on
// the blocking pool.

/// Identifies a dropped folder or files and returns the dry-run plan (nothing is written).
#[tauri::command]
pub async fn import_preview(
  app: AppHandle,
  state: State<'_, AppState>,
  source_paths: Vec<String>,
  category: Option<String>
) -> Result<ImportPlanDto, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || ImporterModule::preview(&state, Some(app), &source_paths, category))
    .await
    .map_err(|error| error.to_string())?
}

/// Rebuilds the plan after the user edits the identity, without the agent.
#[tauri::command]
pub async fn import_rebuild_plan(state: State<'_, AppState>, identity: ImportIdentity) -> Result<ImportPlanDto, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || ImporterModule::build_plan(&state, identity))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn import_apply(app: AppHandle, state: State<'_, AppState>, identity: ImportIdentity) -> Result<String, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || ImporterModule::apply(&state, Some(app), identity))
    .await
    .map_err(|error| error.to_string())?
}

/// "Atualizar" dry run for a show: metadata, artwork and episode renames (nothing is written).
#[tauri::command]
pub async fn refresh_preview(
  app: AppHandle,
  state: State<'_, AppState>,
  media_id: String,
  tmdb_id: Option<String>
) -> Result<RefreshPlanDto, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || RefreshModule::preview(&state, Some(app), &media_id, tmdb_id))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn refresh_apply(app: AppHandle, state: State<'_, AppState>, selection: RefreshSelection) -> Result<RefreshResultDto, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || RefreshModule::apply(&state, Some(app), selection))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn subtitles_search(
  app: AppHandle,
  state: State<'_, AppState>,
  media_id: String,
  season_number: Option<i32>
) -> Result<SeasonSubtitlesDto, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || subtitles::search(&state, Some(app), &media_id, season_number, None))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn subtitles_download(
  app: AppHandle,
  state: State<'_, AppState>,
  media_id: String,
  downloads: Vec<SubtitleDownload>
) -> Result<SubtitleDownloadResultDto, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || subtitles::download(&state, Some(app), &media_id, downloads))
    .await
    .map_err(|error| error.to_string())?
}

/// Aligns the subtitle files already downloaded for a season (or every season).
#[tauri::command]
pub async fn subtitles_sync_existing(
  app: AppHandle,
  state: State<'_, AppState>,
  media_id: String,
  season_number: Option<i32>
) -> Result<SyncSummary, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || subtitles::sync_existing(&state, Some(app), &media_id, season_number))
    .await
    .map_err(|error| error.to_string())?
}

/// One click on an episode without subtitles: search and download the best
/// one for each configured language.
#[tauri::command]
pub async fn subtitles_fetch_episode(
  app: AppHandle,
  state: State<'_, AppState>,
  media_id: String,
  episode_id: String
) -> Result<SubtitleDownloadResultDto, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || {
    let found = subtitles::search(&state, Some(app.clone()), &media_id, None, Some(&episode_id))?;
    if found.offers.is_empty() {
      return Err(found.notes.first().cloned().unwrap_or_else(|| "Nenhuma legenda encontrada.".to_string()));
    }
    let downloads = found.offers.into_iter().map(|offer| SubtitleDownload { file_id: offer.file_id, target_path: offer.target_path, episode_id: offer.episode_id, embedded_track: offer.embedded_track }).collect();
    subtitles::download(&state, Some(app), &media_id, downloads)
  })
  .await
  .map_err(|error| error.to_string())?
}

#[tauri::command(async)]
pub fn get_subtitle_settings(state: State<'_, AppState>) -> Result<SubtitleSettings, String> {
  SettingsModule::subtitle_settings(&state)
}

#[tauri::command(async)]
pub fn save_subtitle_settings(state: State<'_, AppState>, settings: SubtitleSettings) -> Result<(), String> {
  SettingsModule::save_subtitle_settings(&state, &settings)
}

#[tauri::command(async)]
pub fn list_home_media(state: State<'_, AppState>) -> Result<Vec<MediaItemDto>, String> {
  LibraryModule::list_media(&state).map(|items| {
    items.into_iter().map(to_media_item_dto).collect()
  })
}

#[tauri::command(async)]
pub fn list_portable_new_items(state: State<'_, AppState>) -> Result<Vec<NewItemEntryDto>, String> {
  PortableStateModule::list_new_items(&state).map(|items| {
    items
      .into_iter()
      .map(|entry| NewItemEntryDto {
        item: to_media_item_dto(entry.item),
        added_at: entry.added_at
      })
      .collect()
  })
}

#[tauri::command(async)]
pub fn upsert_portable_new_item(
  state: State<'_, AppState>,
  media_path: String,
  media_type: String,
  added_at: i64
) -> Result<(), String> {
  PortableStateModule::upsert_new_item(&state, &media_path, &media_type, added_at)
}

#[tauri::command(async)]
pub fn remove_portable_new_item(state: State<'_, AppState>, media_path: String) -> Result<(), String> {
  PortableStateModule::remove_new_item(&state, &media_path)
}

#[tauri::command(async)]
pub fn get_media_item(state: State<'_, AppState>, media_id: String) -> Result<Option<MediaItemDto>, String> {
  LibraryModule::get_media_item(&state, &media_id).map(|item| item.map(to_media_item_dto))
}

#[tauri::command(async)]
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

#[tauri::command(async)]
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

#[tauri::command(async)]
pub fn list_music_albums(state: State<'_, AppState>) -> Result<Vec<MediaItemDto>, String> {
  LibraryModule::list_all_music_albums(&state).map(|items| items.into_iter().map(to_media_item_dto).collect())
}

/// Downloads missing director photos from TMDb (blocking HTTP), so it runs on
/// the blocking pool like the scans.
#[tauri::command]
pub async fn list_movie_directors(state: State<'_, AppState>) -> Result<Vec<DirectorDto>, String> {
  let state = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || {
    LibraryModule::list_movie_directors(&state).map(|directors| directors.into_iter().map(to_director_dto).collect())
  })
  .await
  .map_err(|error| error.to_string())?
}

#[tauri::command(async)]
pub fn get_movie_director_detail(
  state: State<'_, AppState>,
  director_id: String
) -> Result<Option<MovieDirectorDetailDto>, String> {
  let Some(director) = LibraryModule::get_director(&state, &director_id)? else {
    return Ok(None);
  };

  let movies = LibraryModule::list_director_movies(&state, &director_id)?;

  Ok(Some(MovieDirectorDetailDto {
    director_id: director.id,
    name: director.name,
    image_path: director.image_path,
    movies: movies.into_iter().map(to_media_item_dto).collect()
  }))
}

#[tauri::command(async)]
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
    overview: item.overview,
    english_title: item.english_title,
    original_title: item.original_title,
    genre: item.genre,
    director: item.director,
    actors: item.actors,
    country: item.country,
    imdb_rating: item.imdb_rating,
    media_path: item.media_path,
    subtitle_path: item.subtitle_path
  }
}

fn to_director_dto(director: crate::infrastructure::db::DirectorListRecord) -> DirectorDto {
  DirectorDto {
    id: director.id,
    name: director.name,
    image_path: director.image_path,
    movie_count: director.movie_count
  }
}

fn resolve_media_explorer_path(media_path: &str) -> Result<String, String> {
  let path = Path::new(media_path);

  if path.is_dir() {
    return Ok(path.to_string_lossy().to_string());
  }

  if path.is_file() {
    return path
      .parent()
      .map(|parent| parent.to_string_lossy().to_string())
      .ok_or_else(|| "Nao foi possivel resolver a pasta da midia.".to_string());
  }

  path
    .parent()
    .filter(|parent| parent.exists())
    .map(|parent| parent.to_string_lossy().to_string())
    .ok_or_else(|| "Nao foi possivel abrir a pasta da midia.".to_string())
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

#[tauri::command]
pub fn player_subtitle_tracks(state: State<'_, AppState>) -> Result<Vec<SubtitleTrackDto>, String> {
  let engine = state.player.lock().map_err(|_| "player lock".to_string())?;
  Ok(engine.subtitle_tracks())
}

#[tauri::command]
pub fn player_set_subtitle(state: State<'_, AppState>, id: Option<String>) -> Result<(), String> {
  let mut engine = state.player.lock().map_err(|_| "player lock".to_string())?;
  engine.set_subtitle(id)
}

/// The subtitle files of one video (the <video> player loads them as tracks).
#[tauri::command(async)]
pub fn list_video_subtitles(media_path: String) -> Vec<SubtitleFileDto> {
  crate::modules::scanner::list_video_subtitles(std::path::Path::new(&media_path))
    .into_iter()
    .map(|(path, tag)| SubtitleFileDto { path: path.to_string_lossy().to_string(), tag })
    .collect()
}

/// Where the <video> player on Linux reads files from (see media_server.rs);
/// None where there is no media server (Windows, which plays through mpv).
#[tauri::command]
pub fn media_server_url(app: AppHandle) -> Option<String> {
  use tauri::Manager;
  app
    .try_state::<crate::infrastructure::media_server::MediaServer>()
    .map(|server| server.base_url.clone())
}

/// Lets the UI report problems it handles silently (a video falling back to
/// mpv, for instance) to the terminal running the app.
#[tauri::command]
pub fn log_frontend(message: String) {
  eprintln!("[frontend] {message}");
}
