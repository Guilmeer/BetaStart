use rusqlite::{Connection, params};

use crate::error::AppResult;
use crate::models::FileEntry;

/// Linha de arquivo pronta para indexação.
pub struct FileRow {
    pub path: String,
    pub name: String,
    pub ext: Option<String>,
    pub size: i64,
    pub mtime: i64,
    pub is_dir: bool,
}

/// Substitui todo o índice de arquivos de forma atômica.
pub fn replace_all(conn: &Connection, rows: &[FileRow]) -> AppResult<usize> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM files_index", [])?;
    {
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO files_index (path, name, ext, size, mtime, is_dir)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for row in rows {
            stmt.execute(params![
                row.path,
                row.name,
                row.ext,
                row.size,
                row.mtime,
                row.is_dir as i64
            ])?;
        }
    }
    tx.commit()?;
    Ok(rows.len())
}

/// Busca arquivos/pastas por nome ou caminho, priorizando prefixo do nome.
pub fn search(conn: &Connection, query: &str, limit: usize) -> AppResult<Vec<FileEntry>> {
    let escaped = query.replace('%', "").replace('_', "");
    let like = format!("%{escaped}%");
    let prefix = format!("{escaped}%");

    let mut stmt = conn.prepare(
        "SELECT path, name, ext, is_dir
           FROM files_index
          WHERE name LIKE ?1 OR path LIKE ?1
          ORDER BY CASE WHEN name LIKE ?2 THEN 0 ELSE 1 END,
                   name COLLATE NOCASE ASC
          LIMIT ?3",
    )?;

    let rows = stmt.query_map(params![like, prefix, limit as i64], |row| {
        Ok(FileEntry {
            path: row.get(0)?,
            name: row.get(1)?,
            ext: row.get(2)?,
            is_dir: row.get::<_, i64>(3)? != 0,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn count(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM files_index", [], |row| row.get(0))?)
}
