pub mod app;
pub mod contracts;
pub mod infrastructure;
pub mod modules;
pub mod support;

use tauri::Manager;

pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      let app_data_dir = app.path().app_data_dir().map_err(|error| error.to_string())?;
      let database = infrastructure::db::DatabaseInfrastructure::new(app_data_dir)?;

      database.initialize()?;
      app.manage(app::state::AppState::new(database));
      // The <video> player on Linux reads files over HTTP (see media_server.rs).
      #[cfg(not(windows))]
      app.manage(infrastructure::media_server::MediaServer::start()?);

      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      app::commands::ping,
      app::commands::exit_app,
      app::commands::open_media_in_explorer,
      app::commands::get_initial_setup_status,
      app::commands::pick_library_root,
      app::commands::pick_import_files,
      app::commands::save_library_root,
      app::commands::save_library_roots,
      app::commands::get_metadata_api_settings,
      app::commands::save_metadata_api_settings,
      app::commands::start_initial_scan,
      app::commands::rescan_library,
      app::commands::reset_library_database_and_rescan,
      app::commands::import_preview,
      app::commands::import_rebuild_plan,
      app::commands::import_apply,
      app::commands::refresh_preview,
      app::commands::refresh_apply,
      app::commands::subtitles_search,
      app::commands::subtitles_download,
      app::commands::subtitles_fetch_episode,
      app::commands::subtitles_sync_existing,
      app::commands::get_subtitle_settings,
      app::commands::save_subtitle_settings,
      app::commands::list_home_media,
      app::commands::list_portable_new_items,
      app::commands::upsert_portable_new_item,
      app::commands::remove_portable_new_item,
      app::commands::get_media_item,
      app::commands::get_show_detail,
      app::commands::get_music_artist_detail,
      app::commands::list_music_albums,
      app::commands::list_movie_directors,
      app::commands::get_movie_director_detail,
      app::commands::get_music_album_detail,
      app::commands::player_load,
      app::commands::player_set_rect,
      app::commands::player_toggle_pause,
      app::commands::player_set_paused,
      app::commands::player_set_volume,
      app::commands::player_set_muted,
      app::commands::player_seek,
      app::commands::player_seek_to,
      app::commands::player_stop,
      app::commands::player_status,
      app::commands::player_subtitle_tracks,
      app::commands::player_set_subtitle,
      app::commands::list_video_subtitles,
      app::commands::media_server_url,
      app::commands::log_frontend
    ])
    .run(tauri::generate_context!())
    .expect("failed to run NetCrico app");
}
