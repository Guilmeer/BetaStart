use rusqlite::{Connection, params};

use crate::error::AppResult;
use crate::platform::AppEntry;

/// Reindexa os aplicativos do Menu Iniciar no banco.
pub fn refresh(conn: &Connection) -> AppResult<usize> {
    let apps = crate::platform::apps::list_installed_apps();
    let now = super::now_secs();

    let tx = conn.unchecked_transaction()?;
    for app in &apps {
        tx.execute(
            "INSERT INTO apps_index (name, exe_path, keywords, indexed_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(exe_path) DO UPDATE SET
               name       = excluded.name,
               keywords   = excluded.keywords,
               indexed_at = excluded.indexed_at",
            params![app.name, app.exe_path, app.keywords.join(","), now],
        )?;
    }
    tx.commit()?;
    Ok(apps.len())
}

/// Busca aplicativos por nome (case-insensitive), priorizando os mais usados.
pub fn search(conn: &Connection, query: &str, limit: usize) -> AppResult<Vec<AppEntry>> {
    let pattern = format!("%{}%", query.replace('%', "").replace('_', ""));
    let mut stmt = conn.prepare(
        "SELECT name, exe_path, keywords
           FROM apps_index
          WHERE name LIKE ?1 OR keywords LIKE ?1
          ORDER BY CASE WHEN name LIKE ?1 THEN 0 ELSE 1 END,
                   launch_count DESC,
                   name COLLATE NOCASE ASC
          LIMIT ?2",
    )?;

    let rows = stmt.query_map(params![pattern, limit as i64], |row| {
        let keywords: String = row.get(2)?;
        Ok(AppEntry {
            name: row.get(0)?,
            exe_path: row.get(1)?,
            icon_path: None,
            keywords: keywords
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect(),
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn all(conn: &Connection) -> AppResult<Vec<AppEntry>> {
    search(conn, "", 1_000)
}

/// Marca um app como usado (para ranquear os mais frequentes).
pub fn mark_launched(conn: &Connection, exe_path: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE apps_index
            SET launch_count = launch_count + 1,
                last_launched = ?2
          WHERE exe_path = ?1",
        params![exe_path, super::now_secs()],
    )?;
    Ok(())
}
