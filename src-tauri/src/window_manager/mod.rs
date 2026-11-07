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
    /// Pixels exatos relativos à área útil do monitor no momento da captura.
    #[serde(default)]
    pub x_px: Option<i64>,
    #[serde(default)]
    pub y_px: Option<i64>,
    #[serde(default)]
    pub w_px: Option<i64>,
    #[serde(default)]
    pub h_px: Option<i64>,
    #[serde(default)]
    pub work_width: Option<i64>,
    #[serde(default)]
    pub work_height: Option<i64>,
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
    let left = monitor.work_x + (x * monitor.work_width as f64).round() as i64;
    let top = monitor.work_y + (y * monitor.work_height as f64).round() as i64;
    let width = (w * monitor.work_width as f64).round().max(50.0) as i64;
    let height = (h * monitor.work_height as f64).round().max(50.0) as i64;
    (left as i32, top as i32, width as i32, height as i32)
}

/// Prefere os pixels exatos salvos no layout; se a resolução mudou, escala
/// proporcionalmente. Layouts antigos (sem pixels) caem nas frações.
fn slot_target_rect(monitor: &MonitorInfo, slot: &LayoutSlot) -> (i32, i32, i32, i32) {
    if let (Some(x), Some(y), Some(w), Some(h), Some(saved_w), Some(saved_h)) = (
        slot.x_px,
        slot.y_px,
        slot.w_px,
        slot.h_px,
        slot.work_width,
        slot.work_height,
    ) {
        if saved_w > 0 && saved_h > 0 && monitor.work_width > 0 && monitor.work_height > 0 {
            let scale_x = monitor.work_width as f64 / saved_w as f64;
            let scale_y = monitor.work_height as f64 / saved_h as f64;
            return (
                (monitor.work_x as f64 + x as f64 * scale_x).round() as i32,
                (monitor.work_y as f64 + y as f64 * scale_y).round() as i32,
                (w as f64 * scale_x).round().max(50.0) as i32,
                (h as f64 * scale_y).round().max(50.0) as i32,
            );
        }
    }
    target_rect(monitor, slot.x, slot.y, slot.w, slot.h)
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
            x_px: Some(window.rect.x - monitor.work_x),
            y_px: Some(window.rect.y - monitor.work_y),
            w_px: Some(window.rect.w),
            h_px: Some(window.rect.h),
            work_width: Some(monitor.work_width),
            work_height: Some(monitor.work_height),
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

        let (left, top, width, height) = slot_target_rect(monitor, slot);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(work_width: i64, work_height: i64) -> MonitorInfo {
        MonitorInfo {
            index: 0,
            name: "TEST".into(),
            x: 0,
            y: 0,
            width: work_width,
            height: work_height,
            work_x: 0,
            work_y: 0,
            work_width,
            work_height,
            primary: true,
        }
    }

    fn slot() -> LayoutSlot {
        LayoutSlot {
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
        }
    }

    #[test]
    fn pixel_slot_restores_exactly() {
        assert_eq!(
            slot_target_rect(&monitor(1920, 1040), &slot()),
            (1440, 0, 480, 1040)
        );
    }

    #[test]
    fn pixel_slot_scales_on_resolution_change() {
        assert_eq!(
            slot_target_rect(&monitor(2560, 1400), &slot()),
            (1920, 0, 640, 1400)
        );
    }
}
