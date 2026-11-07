use rusqlite::{Connection, OptionalExtension, params};

use crate::error::AppResult;
use crate::platform::MonitorInfo;
use crate::window_manager::{LayoutSlot, WindowLayout, WindowRule};

pub fn list_layouts(conn: &Connection) -> AppResult<Vec<WindowLayout>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, hotkey, windows_json
           FROM window_layouts
          ORDER BY name COLLATE NOCASE ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        let slots_json: String = row.get(3)?;
        let slots: Vec<LayoutSlot> = serde_json::from_str(&slots_json).unwrap_or_default();
        Ok(WindowLayout {
            id: row.get(0)?,
            name: row.get(1)?,
            hotkey: row.get(2)?,
            slots,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn get_layout(conn: &Connection, id: i64) -> AppResult<Option<WindowLayout>> {
    let row = conn
        .query_row(
            "SELECT id, name, hotkey, windows_json FROM window_layouts WHERE id = ?1",
            params![id],
            |row| {
                let slots_json: String = row.get(3)?;
                let slots: Vec<LayoutSlot> = serde_json::from_str(&slots_json).unwrap_or_default();
                Ok(WindowLayout {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    hotkey: row.get(2)?,
                    slots,
                })
            },
        )
        .optional()?;
    Ok(row)
}

pub fn save_layout(
    conn: &Connection,
    name: &str,
    hotkey: Option<&str>,
    monitors: &[MonitorInfo],
    slots: &[LayoutSlot],
) -> AppResult<i64> {
    let monitors_json = serde_json::to_string(monitors)?;
    let slots_json = serde_json::to_string(slots)?;
    let now = crate::db::now_secs();

    conn.execute(
        "INSERT INTO window_layouts (name, hotkey, monitors_json, windows_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![name, hotkey, monitors_json, slots_json, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_layout(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM window_layouts WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_layout_hotkey(conn: &Connection, id: i64, hotkey: Option<&str>) -> AppResult<()> {
    conn.execute(
        "UPDATE window_layouts SET hotkey = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, hotkey, crate::db::now_secs()],
    )?;
    Ok(())
}

pub fn layout_hotkeys(conn: &Connection) -> AppResult<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT hotkey FROM window_layouts WHERE hotkey IS NOT NULL AND hotkey <> ''")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn list_rules(conn: &Connection) -> AppResult<Vec<WindowRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, app_match, match_field, monitor, x, y, w, h, enabled
           FROM window_rules
          ORDER BY name COLLATE NOCASE ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(WindowRule {
            id: row.get(0)?,
            name: row.get(1)?,
            app_match: row.get(2)?,
            match_field: row.get(3)?,
            monitor: row.get(4)?,
            x: row.get::<_, Option<f64>>(5)?.unwrap_or(0.0),
            y: row.get::<_, Option<f64>>(6)?.unwrap_or(0.0),
            w: row.get::<_, Option<f64>>(7)?.unwrap_or(1.0),
            h: row.get::<_, Option<f64>>(8)?.unwrap_or(1.0),
            enabled: row.get::<_, i64>(9)? != 0,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn upsert_rule(conn: &Connection, rule: &WindowRule) -> AppResult<i64> {
    if rule.id > 0 {
        conn.execute(
            "UPDATE window_rules
                SET name = ?2, app_match = ?3, match_field = ?4, monitor = ?5,
                    x = ?6, y = ?7, w = ?8, h = ?9, enabled = ?10
              WHERE id = ?1",
            params![
                rule.id,
                rule.name,
                rule.app_match,
                rule.match_field,
                rule.monitor,
                rule.x,
                rule.y,
                rule.w,
                rule.h,
                rule.enabled as i64
            ],
        )?;
        Ok(rule.id)
    } else {
        conn.execute(
            "INSERT INTO window_rules
               (name, app_match, match_field, monitor, x, y, w, h, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                rule.name,
                rule.app_match,
                rule.match_field,
                rule.monitor,
                rule.x,
                rule.y,
                rule.w,
                rule.h,
                rule.enabled as i64
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }
}

pub fn delete_rule(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM window_rules WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::MonitorInfo;
    use crate::window_manager::LayoutSlot;

    fn temp_db() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "betastart-wm-test-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn layout_and_rule_roundtrip() {
        let path = temp_db();
        let conn = crate::db::open(&path).expect("db");

        let monitors = vec![MonitorInfo {
            index: 0,
            name: "TEST".into(),
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            work_x: 0,
            work_y: 0,
            work_width: 1920,
            work_height: 1040,
            primary: true,
        }];
        let slots = vec![LayoutSlot {
            app: Some("chrome.exe".into()),
            match_field: "process".into(),
            monitor: 0,
            x: 0.75,
            y: 0.0,
            w: 0.25,
            h: 1.0,
            x_px: Some(1440),
            y_px: Some(0),
            w_px: Some(480),
            h_px: Some(1040),
            work_width: Some(1920),
            work_height: Some(1040),
        }];

        let id = save_layout(&conn, "teste", Some("Win+Shift+F7"), &monitors, &slots)
            .expect("salvar layout");
        assert_eq!(list_layouts(&conn).expect("listar").len(), 1);
        assert_eq!(layout_hotkeys(&conn).expect("hotkeys").len(), 1);

        let layout = get_layout(&conn, id).expect("get").expect("layout");
        assert_eq!(layout.hotkey.as_deref(), Some("Win+Shift+F7"));
        assert_eq!(layout.slots.len(), 1);

        let rule = WindowRule {
            id: 0,
            name: "Discord à esquerda".into(),
            app_match: "discord".into(),
            match_field: "process".into(),
            monitor: 0,
            x: 0.0,
            y: 0.0,
            w: 0.75,
            h: 1.0,
            enabled: true,
        };
        let rule_id = upsert_rule(&conn, &rule).expect("salvar regra");
        assert_eq!(list_rules(&conn).expect("listar regras").len(), 1);

        delete_rule(&conn, rule_id).expect("remover regra");
        assert!(list_rules(&conn).expect("listar regras").is_empty());

        delete_layout(&conn, id).expect("remover layout");
        assert!(list_layouts(&conn).expect("listar").is_empty());

        let _ = std::fs::remove_file(&path);
    }
}
