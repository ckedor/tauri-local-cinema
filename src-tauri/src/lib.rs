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

      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      app::commands::ping,
      app::commands::get_initial_setup_status,
      app::commands::pick_library_root,
      app::commands::save_library_root,
      app::commands::start_initial_scan,
      app::commands::reset_library_database_and_rescan,
      app::commands::list_home_media,
      app::commands::get_media_item,
      app::commands::get_show_detail,
      app::commands::get_music_artist_detail,
      app::commands::list_music_albums,
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
      app::commands::player_status
    ])
    .run(tauri::generate_context!())
    .expect("failed to run NetCrico app");
}
