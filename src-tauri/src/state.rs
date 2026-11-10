use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use rusqlite::Connection;

use crate::core::config::Config;

/// Evita que o `hide_on_blur` esconda a janela enquanto um diálogo nativo está aberto.
pub static NATIVE_DIALOG_OPEN: AtomicBool = AtomicBool::new(false);

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
