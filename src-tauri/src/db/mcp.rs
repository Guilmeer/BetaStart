use rusqlite::{Connection, params};

use crate::error::AppResult;
use crate::models::McpClient;

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
            conn.execute(
                "UPDATE mcp_clients SET last_seen = ?2 WHERE id = ?1",
                params![client.id, crate::db::now_secs()],
            )?;
            return Ok(Some(client));
        }
    }
    Ok(None)
}
