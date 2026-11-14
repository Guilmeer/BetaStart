//! Camada de persistência auxiliar (índices, clipboard, tasks, tokens MCP).
//! A memória do usuário NÃO fica aqui: ela é composta por arquivos Markdown no workspace.

pub mod apps;
pub mod clipboard;
pub mod extensions;
pub mod files;
pub mod mcp;
pub mod memory;
pub mod schema;
pub mod tasks;
pub mod usage;
pub mod windows;

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;
use rusqlite::OptionalExtension;

use crate::error::AppResult;

/// Abre (ou cria) o banco e roda as migrações.
pub fn open(path: &Path) -> AppResult<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut conn = Connection::open(path)?;
    // Vários consumidores (UI, indexador, clipboard) compartilham o mesmo arquivo.
    conn.busy_timeout(Duration::from_secs(5))?;
    schema::init(&mut conn)?;
    Ok(conn)
}

/// Monta uma consulta FTS5 de prefixo segura a partir do texto do usuário.
pub fn fts_query(query: &str) -> Option<String> {
    let terms: Vec<String> = query
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .take(8)
        .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" AND "))
    }
}

/// Compacta o WAL no fechamento (evita `data.db-wal` crescendo sem limite).
pub fn checkpoint(conn: &Connection) {
    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
}

/// Lê um valor de `meta` (metadados internos do índice).
pub fn meta_get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let value = conn
        .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()?;
    Ok(value)
}

/// Grava um valor de `meta`.
pub fn meta_set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

/// Versão do índice de memória; muda a cada reindexação com alterações.
pub fn memory_version(conn: &Connection) -> String {
    meta_get(conn, "memory_version")
        .ok()
        .flatten()
        .unwrap_or_else(|| "0".into())
}

pub fn graph_cache_get(
    conn: &Connection,
    key: &str,
    max_age_secs: i64,
) -> AppResult<Option<String>> {
    let row = conn
        .query_row(
            "SELECT nodes_json, updated_at FROM graph_cache WHERE workspace_hash = ?1",
            [key],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    let Some((Some(json), updated_at)) = row else {
        return Ok(None);
    };
    if now_secs() - updated_at > max_age_secs {
        return Ok(None);
    }
    Ok(Some(json))
}

pub fn graph_cache_put(conn: &Connection, key: &str, json: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO graph_cache (workspace_hash, nodes_json, edges_json, updated_at)
         VALUES (?1, ?2, '', ?3)
         ON CONFLICT(workspace_hash) DO UPDATE SET
           nodes_json = excluded.nodes_json,
           updated_at = excluded.updated_at",
        rusqlite::params![key, json, now_secs()],
    )?;
    // Cache antigo de versões anteriores não serve mais.
    conn.execute(
        "DELETE FROM graph_cache WHERE updated_at < ?1",
        [now_secs() - 7 * 24 * 3600],
    )?;
    Ok(())
}

/// Timestamp atual em segundos (UTC).
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
