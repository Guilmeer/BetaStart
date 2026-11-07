use std::sync::Mutex;

use rusqlite::Connection;

use crate::core::config::Config;

/// Estado compartilhado gerenciado pelo Tauri e injetado nos comandos.
pub struct AppState {
    pub config: Mutex<Config>,
    pub db: Mutex<Connection>,
}

impl AppState {
    pub fn new(config: Config, db: Connection) -> Self {
        Self {
            config: Mutex::new(config),
            db: Mutex::new(db),
        }
    }
}
