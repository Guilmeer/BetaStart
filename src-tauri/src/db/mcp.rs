use rusqlite::{Connection, params};

use crate::error::{AppError, AppResult};
use crate::models::{McpAudit, McpClient};

pub fn create_client(
    conn: &Connection,
    name: &str,
    token_hash: &str,
    scopes: &[String],
) -> AppResult<i64> {
    let scopes_json = serde_json::to_string(scopes)?;
    conn.execute(
        "INSERT INTO mcp_clients (client_name, token_hash, scopes_json, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![name, token_hash, scopes_json, crate::db::now_secs()],
    )?;
    Ok(conn.last_insert_rowid())
}

fn map_client(row: &rusqlite::Row<'_>) -> rusqlite::Result<McpClient> {
    let scopes: String = row.get(2)?;
    Ok(McpClient {
        id: row.get(0)?,
        name: row.get(1)?,
        scopes: serde_json::from_str(&scopes).unwrap_or_default(),
        created_at: row.get(3)?,
        last_seen: row.get(4)?,
        revoked: row.get::<_, i64>(5)? != 0,
    })
}

pub fn list_clients(conn: &Connection) -> AppResult<Vec<McpClient>> {
    let mut stmt = conn.prepare(
        "SELECT id, client_name, scopes_json, created_at, last_seen, revoked
           FROM mcp_clients
          ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], map_client)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn revoke_client(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE mcp_clients SET revoked = 1 WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

/// Remove definitively a revoked client. Active tokens must be revoked first,
/// so a stray click cannot delete a credential that is still in use.
pub fn delete_client(conn: &Connection, id: i64) -> AppResult<()> {
    let changed = conn.execute(
        "DELETE FROM mcp_clients WHERE id = ?1 AND revoked = 1",
        params![id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound(
            "revoked MCP client not found".into(),
        ));
    }
    Ok(())
}

/// Registra uma chamada de ferramenta MCP (best-effort).
pub fn audit(
    conn: &Connection,
    client_id: i64,
    client_name: &str,
    tool: &str,
    ok: bool,
    detail: Option<&str>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO mcp_audit (client_id, client_name, tool, ok, detail, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            client_id,
            client_name,
            tool,
            ok as i64,
            detail,
            crate::db::now_secs()
        ],
    )?;
    Ok(())
}

pub fn recent_audit(conn: &Connection, limit: usize) -> AppResult<Vec<McpAudit>> {
    let mut stmt = conn.prepare(
        "SELECT id, client_id, client_name, tool, ok, detail, created_at
           FROM mcp_audit
          ORDER BY created_at DESC, id DESC
          LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit.clamp(1, 500) as i64], |row| {
        Ok(McpAudit {
            id: row.get(0)?,
            client_id: row.get(1)?,
            client_name: row.get(2)?,
            tool: row.get(3)?,
            ok: row.get::<_, i64>(4)? != 0,
            detail: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn count(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM mcp_clients WHERE revoked = 0",
        [],
        |row| row.get(0),
    )?)
}

/// Verifica um token contra todos os clientes ativos (Argon2 é salted,
/// então não dá para consultar pelo hash).
pub fn verify_token(conn: &Connection, token: &str) -> AppResult<Option<McpClient>> {
    let mut stmt = conn.prepare(
        "SELECT id, client_name, scopes_json, created_at, last_seen, revoked, token_hash
           FROM mcp_clients
          WHERE revoked = 0",
    )?;
    let rows = stmt.query_map([], |row| {
        let scopes: String = row.get(2)?;
        Ok((
            McpClient {
                id: row.get(0)?,
                name: row.get(1)?,
                scopes: serde_json::from_str(&scopes).unwrap_or_default(),
                created_at: row.get(3)?,
                last_seen: row.get(4)?,
                revoked: row.get::<_, i64>(5)? != 0,
            },
            row.get::<_, String>(6)?,
        ))
    })?;

    for row in rows {
        let (client, hash) = row?;
        if crate::mcp::auth::verify_token(token, &hash) {
            // Best-effort: o app pode estar com o banco em uso; não falhar o auth por isso.
            let _ = conn.execute(
                "UPDATE mcp_clients SET last_seen = ?2 WHERE id = ?1",
                params![client.id, crate::db::now_secs()],
            );
            return Ok(Some(client));
        }
    }
    Ok(None)
}
