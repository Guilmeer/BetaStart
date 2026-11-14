use rusqlite::{Connection, OptionalExtension, params};

use crate::error::AppResult;
use crate::models::ClipEntry;

/// Insere um item de texto, evitando duplicata consecutiva, e poda os antigos.
pub fn insert(conn: &Connection, text: &str, max_entries: i64) -> AppResult<bool> {
    let last: Option<String> = conn
        .query_row(
            "SELECT text FROM clipboard_history
              WHERE kind = 'text'
              ORDER BY copied_at DESC, id DESC
              LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;

    if last.as_deref() == Some(text) {
        return Ok(false);
    }

    conn.execute(
        "INSERT INTO clipboard_history (kind, text, copied_at) VALUES ('text', ?1, ?2)",
        params![text, super::now_secs()],
    )?;

    conn.execute(
        "DELETE FROM clipboard_history
          WHERE pinned = 0
            AND id NOT IN (
              SELECT id FROM clipboard_history
               WHERE pinned = 0
               ORDER BY copied_at DESC, id DESC
               LIMIT ?1
            )",
        params![max_entries.max(1)],
    )?;

    Ok(true)
}

/// Insere uma imagem já salva em disco e poda o histórico.
pub fn insert_image(conn: &Connection, image_path: &str, max_entries: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO clipboard_history (kind, image_path, copied_at)
         VALUES ('image', ?1, ?2)",
        params![image_path, super::now_secs()],
    )?;
    conn.execute(
        "DELETE FROM clipboard_history
          WHERE pinned = 0
            AND id NOT IN (
              SELECT id FROM clipboard_history
               WHERE pinned = 0
               ORDER BY copied_at DESC, id DESC
               LIMIT ?1
            )",
        params![max_entries.max(1)],
    )?;
    cleanup_images(conn);
    Ok(())
}

/// Remove arquivos de imagem que não estão mais referenciados pelo histórico.
pub fn cleanup_images(conn: &Connection) {
    use std::collections::HashSet;

    let dir = crate::core::paths::data_dir().join("clipboard");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut keep: HashSet<String> = HashSet::new();
    if let Ok(mut stmt) =
        conn.prepare("SELECT image_path FROM clipboard_history WHERE image_path IS NOT NULL")
    {
        if let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) {
            for row in rows.flatten() {
                keep.insert(row);
            }
        }
    }
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("png") {
            continue;
        }
        let value = path.to_string_lossy().to_string();
        if !keep.contains(&value) {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Option<ClipEntry>> {
    let row = conn
        .query_row(
            "SELECT id, kind, text, image_path, copied_at, pinned
               FROM clipboard_history WHERE id = ?1",
            params![id],
            map_entry,
        )
        .optional()?;
    Ok(row)
}

fn map_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClipEntry> {
    Ok(ClipEntry {
        id: row.get(0)?,
        kind: row.get(1)?,
        text: row.get(2)?,
        image_path: row.get(3)?,
        copied_at: row.get(4)?,
        pinned: row.get::<_, i64>(5)? != 0,
    })
}

pub fn recent(conn: &Connection, limit: usize) -> AppResult<Vec<ClipEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, text, image_path, copied_at, pinned
           FROM clipboard_history
          ORDER BY pinned DESC, copied_at DESC, id DESC
          LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], map_entry)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn search(conn: &Connection, query: &str, limit: usize) -> AppResult<Vec<ClipEntry>> {
    let escaped = query.replace('%', "").replace('_', "");
    let like = format!("%{escaped}%");
    let mut stmt = conn.prepare(
        "SELECT id, kind, text, image_path, copied_at, pinned
           FROM clipboard_history
          WHERE text LIKE ?1
          ORDER BY pinned DESC, copied_at DESC, id DESC
          LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![like, limit as i64], map_entry)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM clipboard_history WHERE id = ?1", params![id])?;
    cleanup_images(conn);
    Ok(())
}

pub fn clear_unpinned(conn: &Connection) -> AppResult<usize> {
    let removed = conn.execute("DELETE FROM clipboard_history WHERE pinned = 0", [])?;
    cleanup_images(conn);
    Ok(removed)
}

pub fn set_pinned(conn: &Connection, id: i64, pinned: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE clipboard_history SET pinned = ?2 WHERE id = ?1",
        params![id, pinned as i64],
    )?;
    Ok(())
}

pub fn count(conn: &Connection) -> AppResult<i64> {
    Ok(
        conn.query_row("SELECT COUNT(*) FROM clipboard_history", [], |row| {
            row.get(0)
        })?,
    )
}
