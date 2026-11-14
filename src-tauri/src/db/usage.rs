use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{Connection, params, params_from_iter};

use crate::error::AppResult;

/// Registra o uso de um resultado do launcher.
pub fn touch(conn: &Connection, kind: &str, key: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO usage_stats (kind, key, uses, last_used)
         VALUES (?1, ?2, 1, ?3)
         ON CONFLICT(kind, key) DO UPDATE SET
           uses = uses + 1,
           last_used = excluded.last_used",
        params![kind, key, crate::db::now_secs()],
    )?;
    Ok(())
}

/// Mapa `key -> uses` para ordenar um lote de resultados sem N consultas.
pub fn score_map(
    conn: &Connection,
    kind: &str,
    keys: &[String],
) -> AppResult<HashMap<String, i64>> {
    if keys.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = (0..keys.len()).map(|_| "?").collect::<Vec<_>>().join(",");
    let sql =
        format!("SELECT key, uses FROM usage_stats WHERE kind = ?1 AND key IN ({placeholders})");
    let mut values: Vec<Value> = Vec::with_capacity(keys.len() + 1);
    values.push(Value::Text(kind.to_string()));
    values.extend(keys.iter().cloned().map(Value::Text));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(values), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    let mut scores = HashMap::new();
    for row in rows {
        let (key, uses) = row?;
        scores.insert(key, uses);
    }
    Ok(scores)
}
