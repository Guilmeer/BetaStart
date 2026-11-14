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
    // Mantém o índice FTS externo em sincronia com files_index.
    let _ = conn.execute("INSERT INTO files_fts(files_fts) VALUES('rebuild')", []);
    Ok(rows.len())
}

/// Busca arquivos/pastas por nome ou caminho, priorizando prefixo do nome.
pub fn search(conn: &Connection, query: &str, limit: usize) -> AppResult<Vec<FileEntry>> {
    if let Some(fts) = crate::db::fts_query(query) {
        let result = (|| -> AppResult<Vec<FileEntry>> {
            let mut stmt = conn.prepare(
                "SELECT f.path, f.name, f.ext, f.is_dir
                   FROM files_fts
                   JOIN files_index f ON f.id = files_fts.rowid
                  WHERE files_fts MATCH ?1
                  ORDER BY bm25(files_fts, 10.0, 1.0), f.name COLLATE NOCASE ASC
                  LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![fts, limit as i64], |row| {
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
        })();
        if let Ok(items) = result {
            if !items.is_empty() {
                return Ok(items);
            }
        }
    }

    // Fallback para buscas por substring que o prefixo do FTS não cobre.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searches_by_prefix_and_substring() {
        let path = std::env::temp_dir().join(format!(
            "betastart-files-test-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let conn = crate::db::open(&path).expect("db");
        replace_all(
            &conn,
            &[FileRow {
                path: "C:\\docs\\relatorio-final.pdf".into(),
                name: "relatorio-final.pdf".into(),
                ext: Some("pdf".into()),
                size: 1,
                mtime: 0,
                is_dir: false,
            }],
        )
        .expect("index");

        let prefix = search(&conn, "relat", 10).expect("prefix");
        assert_eq!(prefix.len(), 1);
        assert_eq!(prefix[0].name, "relatorio-final.pdf");

        let substring = search(&conn, "atorio", 10).expect("substring");
        assert_eq!(substring.len(), 1);

        let _ = std::fs::remove_file(&path);
    }
}
