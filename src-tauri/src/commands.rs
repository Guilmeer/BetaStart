use serde::Serialize;
use tauri::{AppHandle, State};

use crate::core::config::Config;
use crate::core::paths;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::platform::{self, AppEntry, WindowInfo};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathsInfo {
    pub config: String,
    pub db: String,
    pub workspace: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusInfo {
    pub version: String,
    pub apps_indexed: i64,
    pub open_windows: usize,
    pub paths: PathsInfo,
}

fn paths_info(app: &AppHandle, cfg: &Config) -> AppResult<PathsInfo> {
    Ok(PathsInfo {
        config: paths::config_path(app)?.display().to_string(),
        db: paths::db_path(app)?.display().to_string(),
        workspace: paths::expand(&cfg.workspace.path).display().to_string(),
    })
}

fn apply_patch(app: &AppHandle, state: &AppState, patch: serde_json::Value) -> AppResult<Config> {
    let updated = {
        let current = state
            .config
            .lock()
            .map_err(|_| AppError::Config("config lock envenenado".into()))?;
        current.merged(patch)?
    };
    let path = paths::config_path(app)?;
    updated.save(&path)?;
    *state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))? = updated.clone();
    Ok(updated)
}

#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> AppResult<Config> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))?;
    Ok(cfg.clone())
}

#[tauri::command]
pub fn get_paths(app: AppHandle, state: State<AppState>) -> AppResult<PathsInfo> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))?
        .clone();
    paths_info(&app, &cfg)
}

#[tauri::command]
pub fn set_config(
    app: AppHandle,
    state: State<AppState>,
    patch: serde_json::Value,
) -> AppResult<Config> {
    apply_patch(&app, state.inner(), patch)
}

#[tauri::command]
pub fn set_onboarding_done(app: AppHandle, state: State<AppState>) -> AppResult<Config> {
    apply_patch(
        &app,
        state.inner(),
        serde_json::json!({ "onboardingDone": true }),
    )
}

#[tauri::command]
pub fn search_apps(
    state: State<AppState>,
    query: String,
    limit: Option<usize>,
) -> AppResult<Vec<AppEntry>> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::apps::search(&conn, &query, limit.unwrap_or(8))
}

#[tauri::command]
pub fn list_installed_apps(state: State<AppState>) -> AppResult<Vec<AppEntry>> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::apps::all(&conn)
}

#[tauri::command]
pub fn launch_app(state: State<AppState>, exe_path: String) -> AppResult<()> {
    platform::launch(&exe_path)?;
    if let Ok(conn) = state.db.lock() {
        let _ = db::apps::mark_launched(&conn, &exe_path);
    }
    Ok(())
}

#[tauri::command]
pub fn list_open_windows() -> Vec<WindowInfo> {
    platform::windows::list_open_windows()
}

#[tauri::command]
pub fn rebuild_apps_index(state: State<AppState>) -> AppResult<usize> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::apps::refresh(&conn)
}

#[tauri::command]
pub fn get_status(app: AppHandle, state: State<AppState>) -> AppResult<StatusInfo> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))?
        .clone();

    let apps_indexed = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        conn.query_row("SELECT COUNT(*) FROM apps_index", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0)
    };

    Ok(StatusInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        apps_indexed,
        open_windows: platform::windows::list_open_windows().len(),
        paths: paths_info(&app, &cfg)?,
    })
}

#[tauri::command]
pub fn show_launcher(app: AppHandle) {
    crate::core::window::show(&app);
}

#[tauri::command]
pub fn hide_launcher(app: AppHandle) {
    crate::core::window::hide(&app);
}

#[tauri::command]
pub fn toggle_launcher(app: AppHandle) {
    crate::core::window::toggle(&app);
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
