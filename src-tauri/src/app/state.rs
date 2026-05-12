use std::sync::{Arc, Mutex};

use crate::infrastructure::db::DatabaseInfrastructure;
use crate::modules::player::PlayerEngine;

#[derive(Clone)]
pub struct AppState {
  pub player: Arc<Mutex<PlayerEngine>>,
  pub database: DatabaseInfrastructure
}

impl AppState {
  pub fn new(database: DatabaseInfrastructure) -> Self {
    Self {
      player: Arc::new(Mutex::new(PlayerEngine::new())),
      database
    }
  }
}
