//! Camada de persistência auxiliar (índices, clipboard, tasks, tokens MCP).
//! A memória do usuário NÃO fica aqui: ela é composta por arquivos Markdown no workspace.

pub mod apps;
pub mod clipboard;
pub mod files;
pub mod schema;

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

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

/// Timestamp atual em segundos (UTC).
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
