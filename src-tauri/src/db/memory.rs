use std::collections::HashMap;
use std::path::Path;

use rusqlite::types::Value;
use rusqlite::{Connection, params, params_from_iter};

use crate::error::AppResult;
use crate::models::{MemoryFile, MemoryHit, MemoryRow};

/// `file_path` -> (`updated_at` em ms, projeto) do índice atual.
///
/// Base do reindex incremental: arquivos com o mesmo mtime não são relidos.
pub fn snapshot(conn: &Connection) -> AppResult<HashMap<String, (i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT file_path, COALESCE(updated_at, 0), COALESCE(project, '') FROM memory_index",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (path, updated_at, project) = row?;
        out.insert(path, (updated_at, project));
    }
    Ok(out)
}

/// Insere/atualiza apenas as linhas informadas (os triggers mantêm o FTS).
pub fn upsert_rows(conn: &Connection, rows: &[MemoryRow]) -> AppResult<usize> {
    if rows.is_empty() {
        return Ok(0);
    }
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO memory_index
               (file_path, kind, title, project, tags, links, content, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(file_path) DO UPDATE SET
               kind = excluded.kind,
               title = excluded.title,
               project = excluded.project,
               tags = excluded.tags,
               links = excluded.links,
               content = excluded.content,
               updated_at = excluded.updated_at",
        )?;
        for row in rows {
            let tags = serde_json::to_string(&row.tags)?;
            let links = serde_json::to_string(&row.links)?;
            stmt.execute(params![
                row.file_path,
                row.kind,
                row.title,
                row.project,
                tags,
                links,
                row.content,
                row.updated_at
            ])?;
        }
    }
    tx.commit()?;
    Ok(rows.len())
}

/// Remove do índice os arquivos que não existem mais.
pub fn delete_paths(conn: &Connection, paths: &[String]) -> AppResult<usize> {
    if paths.is_empty() {
        return Ok(0);
    }
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare("DELETE FROM memory_index WHERE file_path = ?1")?;
        for path in paths {
            stmt.execute(params![path])?;
        }
    }
    tx.commit()?;
    Ok(paths.len())
}

#[allow(dead_code)] // usado nos testes e como caminho de reindexação total
pub fn replace_all(conn: &Connection, rows: &[MemoryRow]) -> AppResult<usize> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM memory_index", [])?;
    {
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO memory_index
               (file_path, kind, title, project, tags, links, content, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for row in rows {
            let tags = serde_json::to_string(&row.tags)?;
            let links = serde_json::to_string(&row.links)?;
            stmt.execute(params![
                row.file_path,
                row.kind,
                row.title,
                row.project,
                tags,
                links,
                row.content,
                row.updated_at
            ])?;
        }
    }
    tx.commit()?;
    // Mantém a tabela FTS em sincronia (best-effort).
    let _ = conn.execute("INSERT INTO memory_fts(memory_fts) VALUES('rebuild')", []);
    Ok(rows.len())
}

pub fn list_files(conn: &Connection, project: Option<&str>) -> AppResult<Vec<MemoryFile>> {
    let mut stmt = conn.prepare(
        "SELECT file_path, kind, title, project, tags
           FROM memory_index
          WHERE (?1 IS NULL OR project = ?1)
            AND kind IN ('note', 'task', 'project')
          ORDER BY project COLLATE NOCASE, title COLLATE NOCASE",
    )?;

    let rows = stmt.query_map(params![project], |row| {
        let path: String = row.get(0)?;
        let tags: String = row.get(4)?;
        Ok(MemoryFile {
            name: Path::new(&path)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
            path,
            kind: row.get(1)?,
            title: row.get(2)?,
            project: row.get(3)?,
            tags: serde_json::from_str(&tags).unwrap_or_default(),
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn search(
    conn: &Connection,
    project: Option<&str>,
    query: &str,
    tags: &[String],
    limit: usize,
) -> AppResult<Vec<MemoryHit>> {
    // FTS5: prefixo + ranking por relevância (título pesa mais que o corpo).
    if let Some(fts) = crate::db::fts_query(query) {
        let result = (|| -> AppResult<Vec<MemoryHit>> {
            let mut stmt = conn.prepare(
                "SELECT m.file_path, m.kind, m.title, m.project, m.tags,
                        substr(COALESCE(m.content, ''), 1, 240)
                   FROM memory_fts
                   JOIN memory_index m ON m.id = memory_fts.rowid
                  WHERE memory_fts MATCH ?1
                    AND (?2 IS NULL OR m.project = ?2)
                  ORDER BY bm25(memory_fts, 10.0, 1.0, 5.0), m.updated_at DESC
                  LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![fts, project, (limit.max(1) * 3) as i64], |row| {
                let tags: String = row.get(4)?;
                Ok(MemoryHit {
                    file_path: row.get(0)?,
                    kind: row.get(1)?,
                    title: row.get(2)?,
                    project: row.get(3)?,
                    tags: serde_json::from_str(&tags).unwrap_or_default(),
                    snippet: row.get(5)?,
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
                return Ok(filter_memory_tags(items, tags, limit));
            }
        }
    }

    // Fallback: busca por substring (cobre termos que não são prefixo).
    // "santa polished" exige as duas palavras em algum lugar
    // (título, corpo, tags ou caminho), em qualquer ordem.
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|term| term.replace('%', "").replace('_', ""))
        .filter(|term| !term.is_empty())
        .take(8)
        .collect();

    let mut sql = String::from(
        "SELECT file_path, kind, title, project, tags, substr(COALESCE(content, ''), 1, 240)
           FROM memory_index
          WHERE (?1 IS NULL OR project = ?1)",
    );
    for index in 0..terms.len() {
        sql.push_str(&format!(
            " AND (title LIKE ?{0} OR COALESCE(content, '') LIKE ?{0} OR tags LIKE ?{0} OR file_path LIKE ?{0})",
            index + 2
        ));
    }
    sql.push_str(" ORDER BY updated_at DESC LIMIT ?");
    sql.push_str(&(terms.len() + 2).to_string());

    let mut values: Vec<Value> = Vec::with_capacity(terms.len() + 2);
    values.push(match project {
        Some(project) => Value::Text(project.to_string()),
        None => Value::Null,
    });
    for term in &terms {
        values.push(Value::Text(format!("%{term}%")));
    }
    values.push(Value::Integer((limit.max(1) * 3) as i64));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(values), |row| {
        let tags: String = row.get(4)?;
        Ok(MemoryHit {
            file_path: row.get(0)?,
            kind: row.get(1)?,
            title: row.get(2)?,
            project: row.get(3)?,
            tags: serde_json::from_str(&tags).unwrap_or_default(),
            snippet: row.get(5)?,
        })
    })?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(filter_memory_tags(items, tags, limit))
}

fn filter_memory_tags(items: Vec<MemoryHit>, tags: &[String], limit: usize) -> Vec<MemoryHit> {
    items
        .into_iter()
        .filter(|hit| {
            tags.iter()
                .all(|tag| hit.tags.iter().any(|item| item.eq_ignore_ascii_case(tag)))
        })
        .take(limit)
        .collect()
}

/// Linha enxuta para o `project.brief`, já ordenada por "importância":
/// grau aproximado (links + tags) primeiro, depois recência.
pub struct BriefRow {
    pub file_path: String,
    pub kind: String,
    pub title: String,
    pub tags: Vec<String>,
    pub updated_at: i64,
}

pub fn brief_rows(conn: &Connection, project: Option<&str>) -> AppResult<Vec<BriefRow>> {
    let mut stmt = conn.prepare(
        "SELECT file_path, kind, title, tags,
                COALESCE((length(links) - length(replace(links, '\"', ''))) / 2, 0) AS link_count,
                COALESCE(updated_at, 0)
           FROM memory_index
          WHERE (?1 IS NULL OR project = ?1)",
    )?;
    let rows = stmt.query_map(params![project], |row| {
        let tags: String = row.get(3)?;
        let links: i64 = row.get(4)?;
        Ok((
            BriefRow {
                file_path: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                tags: serde_json::from_str(&tags).unwrap_or_default(),
                updated_at: row.get(5)?,
            },
            links,
        ))
    })?;

    let mut out = Vec::new();
    for row in rows {
        let (item, links) = row?;
        out.push((item, links));
    }
    out.sort_by(|left, right| {
        let left_weight = left.0.tags.len() as i64 + left.1;
        let right_weight = right.0.tags.len() as i64 + right.1;
        right_weight
            .cmp(&left_weight)
            .then(right.0.updated_at.cmp(&left.0.updated_at))
            .then(left.0.file_path.cmp(&right.0.file_path))
    });
    Ok(out.into_iter().map(|(item, _)| item).collect())
}

#[allow(dead_code)]
pub fn count(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM memory_index", [], |row| row.get(0))?)
}
