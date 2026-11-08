use rusqlite::{Connection, OptionalExtension, params};

use crate::error::AppResult;
use crate::models::{Task, TaskRow};

pub fn replace_all(conn: &Connection, rows: &[TaskRow]) -> AppResult<usize> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM tasks_index", [])?;

    {
        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO tasks_index
               (id, file_path, title, status, priority, due, remind, tags, project,
                parent_id, sort_order, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?;
        for (index, row) in rows.iter().enumerate() {
            let tags = serde_json::to_string(&row.tags)?;
            stmt.execute(params![
                row.id,
                row.file_path,
                row.title,
                row.status,
                row.priority,
                row.due,
                row.remind,
                tags,
                row.project,
                row.parent_id,
                index as i64,
                row.updated_at
            ])?;
        }
    }

    // Remove lembretes de tasks que não têm mais `remind`.
    tx.execute(
        "DELETE FROM reminders
          WHERE task_id NOT IN (SELECT id FROM tasks_index WHERE remind IS NOT NULL)",
        [],
    )?;

    {
        let mut stmt = tx.prepare(
            "INSERT INTO reminders (task_id, due_at, enabled, notified_at)
             VALUES (?1, ?2, 1, NULL)
             ON CONFLICT(task_id) DO UPDATE SET
               due_at = excluded.due_at,
               enabled = 1,
               notified_at = CASE
                 WHEN reminders.due_at <> excluded.due_at THEN NULL
                 ELSE reminders.notified_at
               END",
        )?;
        for row in rows {
            if let Some(remind) = row.remind {
                stmt.execute(params![row.id, remind])?;
            }
        }
    }

    tx.commit()?;
    Ok(rows.len())
}

fn iso_from_epoch(timestamp: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|datetime| datetime.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

fn map_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let due: Option<i64> = row.get(5)?;
    let remind: Option<i64> = row.get(6)?;
    let tags: String = row.get(7)?;

    Ok(Task {
        id: row.get(0)?,
        file_path: row.get(1)?,
        title: row.get(2)?,
        status: row.get(3)?,
        priority: row.get(4)?,
        due,
        due_iso: due.and_then(iso_from_epoch),
        remind,
        remind_iso: remind.and_then(iso_from_epoch),
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        project: row.get(8)?,
        parent_id: row.get(9)?,
    })
}

const SELECT_TASK: &str =
    "SELECT id, file_path, title, status, priority, due, remind, tags, project, parent_id
       FROM tasks_index";

pub fn list(
    conn: &Connection,
    project: Option<&str>,
    status: Option<&str>,
    query: Option<&str>,
) -> AppResult<Vec<Task>> {
    let escaped = query.unwrap_or("").replace('%', "").replace('_', "");
    let like = format!("%{escaped}%");

    let mut stmt = conn.prepare(&format!(
        "{SELECT_TASK}
          WHERE (?1 IS NULL OR project = ?1)
            AND (?2 IS NULL OR status = ?2)
            AND (?3 = '' OR title LIKE ?4)
          ORDER BY
            CASE status
              WHEN 'in_progress' THEN 0
              WHEN 'todo' THEN 1
              WHEN 'blocked' THEN 2
              WHEN 'done' THEN 3
              ELSE 4
            END,
            CASE priority
              WHEN 'high' THEN 0
              WHEN 'medium' THEN 1
              WHEN 'low' THEN 2
              ELSE 3
            END,
            due IS NULL, due, title COLLATE NOCASE"
    ))?;

    let rows = stmt.query_map(
        params![project, status, query.unwrap_or(""), like],
        map_task,
    )?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Task>> {
    let row = conn
        .query_row(
            &format!("{SELECT_TASK} WHERE id = ?1"),
            params![id],
            map_task,
        )
        .optional()?;
    Ok(row)
}

pub fn file_path_for_id(conn: &Connection, id: &str) -> AppResult<Option<String>> {
    let row = conn
        .query_row(
            "SELECT file_path FROM tasks_index WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(row)
}

pub fn due_reminders(conn: &Connection, now: i64) -> AppResult<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, t.title
           FROM reminders r
           JOIN tasks_index t ON t.id = r.task_id
          WHERE r.enabled = 1
            AND r.notified_at IS NULL
            AND r.due_at <= ?1
            AND t.status <> 'done'",
    )?;
    let rows = stmt.query_map(params![now], |row| Ok((row.get(0)?, row.get(1)?)))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn mark_notified(conn: &Connection, reminder_id: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE reminders SET notified_at = ?2 WHERE id = ?1",
        params![reminder_id, crate::db::now_secs()],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn count(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM tasks_index", [], |row| row.get(0))?)
}
