//! Gerenciador de janelas: layouts, regras por app e multi-monitor.

use serde::{Deserialize, Serialize};

use crate::platform::{MonitorInfo, Rect, WindowInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSlot {
    /// Processo/título/classe a casar (contains, case-insensitive).
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default = "default_field")]
    pub match_field: String,
    pub monitor: i32,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowLayout {
    pub id: i64,
    pub name: String,
    pub hotkey: Option<String>,
    pub slots: Vec<LayoutSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowRule {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    pub app_match: String,
    #[serde(default = "default_field")]
    pub match_field: String,
    pub monitor: i32,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub moved: usize,
    pub missed: Vec<String>,
}

fn default_field() -> String {
    "process".into()
}

fn default_true() -> bool {
    true
}

pub fn list_monitors() -> Vec<MonitorInfo> {
    crate::platform::windows::list_monitors()
}

pub fn snapshot_windows(monitors: &[MonitorInfo]) -> Vec<WindowInfo> {
    let mut windows = crate::platform::windows::list_open_windows();
    for window in &mut windows {
        window.monitor = monitor_index_for_rect(&window.rect, monitors);
    }
    windows
}

fn monitor_index_for_rect(rect: &Rect, monitors: &[MonitorInfo]) -> i32 {
    if monitors.is_empty() {
        return 0;
    }
    let cx = rect.x + rect.w / 2;
    let cy = rect.y + rect.h / 2;
    for monitor in monitors {
        if cx >= monitor.x
            && cx < monitor.x + monitor.width
            && cy >= monitor.y
            && cy < monitor.y + monitor.height
        {
            return monitor.index;
        }
    }
    0
}

fn monitor_at(monitors: &[MonitorInfo], index: i32) -> Option<&MonitorInfo> {
    monitors
        .get(index.max(0) as usize)
        .or_else(|| monitors.first())
}

fn target_rect(monitor: &MonitorInfo, x: f64, y: f64, w: f64, h: f64) -> (i32, i32, i32, i32) {
    let left = monitor.work_x + (x * monitor.work_width as f64) as i64;
    let top = monitor.work_y + (y * monitor.work_height as f64) as i64;
    let width = (w * monitor.work_width as f64).max(50.0) as i64;
    let height = (h * monitor.work_height as f64).max(50.0) as i64;
    (left as i32, top as i32, width as i32, height as i32)
}

fn field_value<'a>(window: &'a WindowInfo, field: &str) -> &'a str {
    match field {
        "title" => &window.title,
        "class" => window.class.as_deref().unwrap_or(""),
        _ => &window.process,
    }
}

fn matches_field(window: &WindowInfo, field: &str, needle: &str) -> bool {
    let needle = needle.trim().to_lowercase();
    if needle.is_empty() {
        return false;
    }
    field_value(window, field).to_lowercase().contains(&needle)
}

fn is_own_window(window: &WindowInfo) -> bool {
    window.process.to_lowercase().contains("betastart")
}

pub fn matches_window(rule: &WindowRule, window: &WindowInfo) -> bool {
    matches_field(window, &rule.match_field, &rule.app_match)
}

/// Captura a disposição atual das janelas como slots relativos ao work area.
pub fn capture_layout(monitors: &[MonitorInfo]) -> Vec<LayoutSlot> {
    if monitors.is_empty() {
        return Vec::new();
    }

    let windows = snapshot_windows(monitors);
    let mut slots = Vec::new();

    for window in windows {
        if is_own_window(&window) || window.title.trim().is_empty() {
            continue;
        }
        if let Some(class) = &window.class {
            let class = class.to_lowercase();
            if class == "shell_traywnd" || class == "progman" || class == "workerw" {
                continue;
            }
        }

        let Some(monitor) = monitor_at(monitors, window.monitor) else {
            continue;
        };
        if monitor.work_width <= 0 || monitor.work_height <= 0 {
            continue;
        }

        let x =
            ((window.rect.x - monitor.work_x) as f64 / monitor.work_width as f64).clamp(0.0, 1.0);
        let y =
            ((window.rect.y - monitor.work_y) as f64 / monitor.work_height as f64).clamp(0.0, 1.0);
        let w = (window.rect.w as f64 / monitor.work_width as f64).clamp(0.0, 1.0);
        let h = (window.rect.h as f64 / monitor.work_height as f64).clamp(0.0, 1.0);

        slots.push(LayoutSlot {
            app: Some(window.process.clone()),
            match_field: "process".into(),
            monitor: window.monitor,
            x,
            y,
            w,
            h,
        });
    }

    slots
}

pub fn apply_layout(
    layout: &WindowLayout,
    windows: &[WindowInfo],
    monitors: &[MonitorInfo],
) -> ApplyResult {
    let mut used: Vec<u64> = Vec::new();
    let mut moved = 0;
    let mut missed = Vec::new();

    if monitors.is_empty() {
        return ApplyResult {
            moved: 0,
            missed: layout
                .slots
                .iter()
                .map(|slot| slot.app.clone().unwrap_or_else(|| "(foco)".into()))
                .collect(),
        };
    }

    let focused = crate::platform::windows::foreground_window();

    for slot in &layout.slots {
        let candidate = if let Some(app) = &slot.app {
            windows.iter().find(|window| {
                !used.contains(&window.handle)
                    && !is_own_window(window)
                    && matches_field(window, &slot.match_field, app)
            })
        } else {
            focused.and_then(|handle| {
                windows
                    .iter()
                    .find(|window| window.handle == handle && !used.contains(&window.handle))
            })
        };

        let Some(window) = candidate else {
            missed.push(slot.app.clone().unwrap_or_else(|| "(foco)".into()));
            continue;
        };
        let Some(monitor) = monitor_at(monitors, slot.monitor) else {
            continue;
        };

        let (left, top, width, height) = target_rect(monitor, slot.x, slot.y, slot.w, slot.h);
        if crate::platform::windows::move_window_rect(window.handle, left, top, width, height)
            .is_ok()
        {
            used.push(window.handle);
            moved += 1;
        } else {
            missed.push(slot.app.clone().unwrap_or_else(|| "(foco)".into()));
        }
    }

    ApplyResult { moved, missed }
}

pub fn apply_rule(rule: &WindowRule, window: &WindowInfo, monitors: &[MonitorInfo]) -> bool {
    if !rule.enabled || !matches_window(rule, window) {
        return false;
    }
    let Some(monitor) = monitor_at(monitors, rule.monitor) else {
        return false;
    };
    let (left, top, width, height) = target_rect(monitor, rule.x, rule.y, rule.w, rule.h);
    crate::platform::windows::move_window_rect(window.handle, left, top, width, height).is_ok()
}

/// Aplica todas as regras habilitadas às janelas abertas (primeira ocorrência por regra).
pub fn apply_rules(rules: &[WindowRule], monitors: &[MonitorInfo]) -> ApplyResult {
    let windows = snapshot_windows(monitors);
    let mut moved = 0;
    let mut missed = Vec::new();

    for rule in rules.iter().filter(|rule| rule.enabled) {
        let mut applied = false;
        for window in &windows {
            if is_own_window(window) {
                continue;
            }
            if matches_window(rule, window) && apply_rule(rule, window, monitors) {
                moved += 1;
                applied = true;
                break;
            }
        }
        if !applied {
            missed.push(rule.name.clone());
        }
    }

    ApplyResult { moved, missed }
}

pub fn apply_layout_by_id(conn: &rusqlite::Connection, id: i64) -> Result<ApplyResult, String> {
    let layout = crate::db::windows::get_layout(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "layout não encontrado".to_string())?;
    let monitors = list_monitors();
    let windows = snapshot_windows(&monitors);
    Ok(apply_layout(&layout, &windows, &monitors))
}
