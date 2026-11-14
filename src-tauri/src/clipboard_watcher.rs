//! Watcher de clipboard: captura texto e imagens novas no histórico.
//!
//! Usa polling (a cada ~600ms) por ser simples e robusto no Windows.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
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
        crate::db::clipboard::cleanup_images(&conn);

        let mut last_seen = String::new();
        let mut last_image_hash: u64 = 0;
        loop {
            if let Ok(text) = clipboard.get_text() {
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() && trimmed != last_seen && trimmed.len() <= 100_000 {
                    last_seen = trimmed.clone();
                    last_image_hash = 0;
                    let _ = crate::db::clipboard::insert(&conn, &trimmed, max_entries);
                }
            } else if let Ok(image) = clipboard.get_image() {
                let pixels = image.width.saturating_mul(image.height);
                if image.width > 0
                    && image.height > 0
                    && pixels > 0
                    && image.bytes.len() == pixels * 4
                    && image.bytes.len() <= 32 * 1024 * 1024
                {
                    let mut hasher = DefaultHasher::new();
                    image.width.hash(&mut hasher);
                    image.height.hash(&mut hasher);
                    image.bytes.as_ref().hash(&mut hasher);
                    let hash = hasher.finish();
                    if hash != last_image_hash {
                        last_image_hash = hash;
                        if let Some(path) = save_image(&image, hash) {
                            let _ = crate::db::clipboard::insert_image(&conn, &path, max_entries);
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(600));
        }
    });
}

fn save_image(image: &arboard::ImageData<'_>, hash: u64) -> Option<String> {
    let dir = crate::core::paths::data_dir().join("clipboard");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{}-{hash:016x}.png", crate::db::now_secs()));
    image::save_buffer(
        &path,
        image.bytes.as_ref(),
        image.width as u32,
        image.height as u32,
        image::ColorType::Rgba8,
    )
    .ok()?;
    Some(path.to_string_lossy().to_string())
}
