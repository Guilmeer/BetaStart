//! Watcher de clipboard: captura texto novo e persiste no histórico.
//!
//! Usa polling (a cada ~600ms) por ser simples e robusto no Windows.

use std::path::PathBuf;
use std::time::Duration;

pub fn start(db_path: PathBuf, max_entries: i64) {
    std::thread::spawn(move || {
        let mut clipboard = match arboard::Clipboard::new() {
            Ok(clipboard) => clipboard,
            Err(err) => {
                eprintln!("[betastart] clipboard indisponível: {err}");
                return;
            }
        };
        let conn = match crate::db::open(&db_path) {
            Ok(conn) => conn,
            Err(err) => {
                eprintln!("[betastart] clipboard db: {err}");
                return;
            }
        };

        let mut last_seen = String::new();
        loop {
            if let Ok(text) = clipboard.get_text() {
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() && trimmed != last_seen && trimmed.len() <= 100_000 {
                    last_seen = trimmed.clone();
                    let _ = crate::db::clipboard::insert(&conn, &trimmed, max_entries);
                }
            }
            std::thread::sleep(Duration::from_millis(600));
        }
    });
}
