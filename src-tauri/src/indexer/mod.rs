//! Indexador de arquivos em background.
//!
//! Varre as pastas configuradas (Documents/Desktop/Downloads por padrão),
//! ignorando pastas pesadas/comuns, e substitui o índice no SQLite em uma
//! transação única (evita travar o banco durante a varredura).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::db::files::FileRow;

const MAX_ENTRIES: usize = 50_000;
const MAX_DEPTH: usize = 12;

pub fn run_file_index(db_path: PathBuf, dirs: Vec<String>, excludes: Vec<String>) {
    let mut excluded: HashSet<String> = HashSet::new();
    for item in excludes {
        excluded.insert(item.to_lowercase());
    }
    for hard in [
        "node_modules",
        ".git",
        "$recycle.bin",
        "system volume information",
        ".cache",
    ] {
        excluded.insert(hard.to_string());
    }

    let mut rows: Vec<FileRow> = Vec::new();
    for dir in dirs {
        let root = crate::core::paths::expand(&dir);
        walk(&root, 0, &excluded, &mut rows);
        if rows.len() >= MAX_ENTRIES {
            break;
        }
    }

    match crate::db::open(&db_path) {
        Ok(conn) => {
            let _ = crate::db::files::replace_all(&conn, &rows);
        }
        Err(err) => eprintln!("[betastart] file index: {err}"),
    }
}

fn walk(dir: &Path, depth: usize, excluded: &HashSet<String>, out: &mut Vec<FileRow>) {
    if depth > MAX_DEPTH || out.len() >= MAX_ENTRIES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        if out.len() >= MAX_ENTRIES {
            return;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(meta) = entry.metadata() else {
            continue;
        };

        if meta.is_dir() {
            let lower = name.to_lowercase();
            if name.starts_with('.') || excluded.contains(&lower) {
                continue;
            }
            push_row(&path, &name, &meta, true, out);
            walk(&path, depth + 1, excluded, out);
        } else {
            push_row(&path, &name, &meta, false, out);
        }
    }
}

fn push_row(
    path: &Path,
    name: &str,
    meta: &std::fs::Metadata,
    is_dir: bool,
    out: &mut Vec<FileRow>,
) {
    let ext = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase());
    let mtime = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);

    out.push(FileRow {
        path: path.to_string_lossy().to_string(),
        name: name.to_string(),
        ext,
        size: meta.len() as i64,
        mtime,
        is_dir,
    });
}
