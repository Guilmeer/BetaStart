use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::core::config::Config;
use crate::core::paths;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::extensions;
use crate::models::{
    ClipEntry, InstalledExtension, RunResult, RuntimeStatus, SearchResults, StoreItem,
};
use crate::platform::{self, AppEntry, MonitorInfo, WindowInfo};
use crate::state::AppState;
use crate::window_manager::{self, ApplyResult, WindowLayout, WindowRule};

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
    pub files_indexed: i64,
    pub clipboard_count: i64,
    pub extensions_count: i64,
    pub open_windows: usize,
    pub win_hook_active: bool,
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
    let (updated, start_on_boot_changed) = {
        let current = state
            .config
            .lock()
            .map_err(|_| AppError::Config("config lock envenenado".into()))?;
        let previous = current.tray.start_on_boot;
        let updated = current.merged(patch)?;
        let changed = previous != updated.tray.start_on_boot;
        (updated, changed)
    };
    if start_on_boot_changed {
        platform::autostart::set(updated.tray.start_on_boot)?;
    }
    let path = paths::config_path(app)?;
    updated.save(&path)?;
    *state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))? = updated.clone();
    Ok(updated)
}

/// Guarda o último layout aplicado para o "restaurar ao iniciar".
pub(crate) fn remember_layout(app: &AppHandle, state: &AppState, id: i64) {
    if let Ok(path) = paths::config_path(app) {
        let updated = {
            let Ok(mut config) = state.config.lock() else {
                return;
            };
            if config.windows.last_layout_id == Some(id) {
                return;
            }
            config.windows.last_layout_id = Some(id);
            config.clone()
        };
        let _ = updated.save(&path);
    }
}

/// Re-registra o atalho do launcher + os atalhos de todos os layouts.
fn reload_shortcuts(app: &AppHandle, state: &AppState) -> AppResult<()> {
    let launcher = state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))?
        .hotkeys
        .launcher
        .clone();

    let layout_hotkeys = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::windows::layout_hotkeys(&conn)?
    };

    crate::core::hotkey::sync(app, &launcher, &layout_hotkeys).map_err(AppError::Platform)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyValidation {
    pub valid: bool,
    pub conflict: Option<String>,
}

/// Valida uma combinação contra o launcher, layouts e o próprio sistema.
fn validate_hotkey_inner(
    app: &AppHandle,
    state: &AppState,
    combo: &str,
    exclude_layout_id: Option<i64>,
    exclude_launcher: bool,
) -> AppResult<HotkeyValidation> {
    let combo = combo.trim();
    if combo.is_empty() {
        return Ok(HotkeyValidation {
            valid: false,
            conflict: Some("atalho vazio".into()),
        });
    }
    if crate::core::hotkey::parse_combo(combo).is_err() {
        return Ok(HotkeyValidation {
            valid: false,
            conflict: Some("combinação inválida".into()),
        });
    }

    if !exclude_launcher {
        let launcher = state
            .config
            .lock()
            .map_err(|_| AppError::Config("config lock envenenado".into()))?
            .hotkeys
            .launcher
            .clone();
        if crate::core::hotkey::same(combo, &launcher) {
            return Ok(HotkeyValidation {
                valid: false,
                conflict: Some("já usado pelo atalho do launcher".into()),
            });
        }
    }

    let layouts = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::windows::list_layouts(&conn)?
    };
    for layout in layouts {
        if Some(layout.id) == exclude_layout_id {
            continue;
        }
        if let Some(hotkey) = &layout.hotkey {
            if crate::core::hotkey::same(combo, hotkey) {
                return Ok(HotkeyValidation {
                    valid: false,
                    conflict: Some(format!("já usado pelo layout '{}'", layout.name)),
                });
            }
        }
    }

    // Conflito com o sistema/outro app: só é conflito se não for um atalho nosso.
    if !crate::core::hotkey::is_registered(app, combo)
        && crate::core::hotkey::test_register(app, combo).is_err()
    {
        return Ok(HotkeyValidation {
            valid: false,
            conflict: Some("em uso pelo sistema ou outro aplicativo".into()),
        });
    }

    Ok(HotkeyValidation {
        valid: true,
        conflict: None,
    })
}

/// Avalia uma expressão matemática simples para o resultado de calculadora.
fn calculate(query: &str) -> Option<String> {
    if query.is_empty() {
        return None;
    }
    let has_operator = query.chars().any(|c| "+-*/^%()".contains(c));
    let has_digit = query.chars().any(|c| c.is_ascii_digit());
    if !has_operator || !has_digit {
        return None;
    }

    match meval::eval_str(query) {
        Ok(value) if value.is_finite() => Some(if (value.fract()).abs() < 1e-9 {
            format!("{}", value as i64)
        } else {
            format!("{value}")
        }),
        _ => None,
    }
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

/// Aplica atalho + modo da tecla Win em tempo real (usado pelo onboarding e settings).
#[tauri::command]
pub fn apply_launcher_settings(
    app: AppHandle,
    state: State<AppState>,
    mode: String,
    hotkey: String,
) -> AppResult<Config> {
    let mode = match mode.as_str() {
        "replace" => "replace",
        _ => "off",
    }
    .to_string();
    let hotkey = hotkey.trim().to_string();
    if hotkey.is_empty() {
        return Err(AppError::Config("atalho vazio".into()));
    }

    let check = validate_hotkey_inner(&app, state.inner(), &hotkey, None, true)?;
    if !check.valid {
        return Err(AppError::Config(
            check.conflict.unwrap_or_else(|| "atalho inválido".into()),
        ));
    }

    let old_combo = state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))?
        .hotkeys
        .launcher
        .clone();

    let layout_hotkeys = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::windows::layout_hotkeys(&conn)?
    };

    if let Err(err) = crate::core::hotkey::sync(&app, &hotkey, &layout_hotkeys) {
        let _ = crate::core::hotkey::sync(&app, &old_combo, &layout_hotkeys);
        return Err(AppError::Config(format!(
            "não foi possível registrar '{hotkey}': {err}"
        )));
    }

    if mode == "replace" {
        if let Err(err) = crate::core::win_hook::install(app.clone()) {
            let _ = crate::core::hotkey::sync(&app, &old_combo, &layout_hotkeys);
            return Err(AppError::Platform(format!("hook da tecla Win: {err}")));
        }
    } else {
        crate::core::win_hook::uninstall();
    }

    apply_patch(
        &app,
        state.inner(),
        serde_json::json!({
            "startMenu": { "mode": mode },
            "hotkeys": { "launcher": hotkey }
        }),
    )
}

#[tauri::command]
pub fn validate_hotkey(
    app: AppHandle,
    state: State<AppState>,
    combo: String,
    exclude_layout_id: Option<i64>,
    exclude_launcher: Option<bool>,
) -> AppResult<HotkeyValidation> {
    validate_hotkey_inner(
        &app,
        state.inner(),
        &combo,
        exclude_layout_id,
        exclude_launcher.unwrap_or(false),
    )
}

/// Suspende os atalhos globais enquanto o usuário grava uma nova combinação.
#[tauri::command]
pub fn suspend_hotkeys(app: AppHandle) -> AppResult<()> {
    crate::core::hotkey::unregister_all(&app).map_err(AppError::Platform)?;
    crate::core::win_hook::uninstall();
    Ok(())
}

#[tauri::command]
pub fn resume_hotkeys(app: AppHandle, state: State<AppState>) -> AppResult<()> {
    reload_shortcuts(&app, state.inner())?;
    let replace = state
        .config
        .lock()
        .map(|cfg| cfg.start_menu.mode == "replace")
        .unwrap_or(false);
    if replace {
        let _ = crate::core::win_hook::install(app.clone());
    }
    Ok(())
}

#[tauri::command]
pub fn search(
    state: State<AppState>,
    query: String,
    limit: Option<usize>,
) -> AppResult<SearchResults> {
    let limit = limit.unwrap_or(12).clamp(1, 50);
    let (include_apps, include_files) = {
        let cfg = state
            .config
            .lock()
            .map_err(|_| AppError::Config("config lock envenenado".into()))?;
        (cfg.search.include_apps, cfg.search.include_files)
    };

    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;

    let q = query.trim();
    let mut apps = if include_apps {
        db::apps::search(&conn, q, limit)?
    } else {
        Vec::new()
    };
    let mut files = if include_files && q.chars().count() >= 2 {
        db::files::search(&conn, q, limit.min(10))?
    } else {
        Vec::new()
    };
    let clipboard = if q.is_empty() {
        db::clipboard::recent(&conn, 4)?
    } else {
        db::clipboard::search(&conn, q, 4)?
    };
    let mut commands = if q.is_empty() {
        Vec::new()
    } else {
        db::extensions::commands(&conn, q, 5)?
    };

    // Resultados usados com mais frequência sobem, mantendo a ordem original
    // (relevância/nome) como desempate.
    let file_keys: Vec<String> = files
        .iter()
        .map(|file| format!("file:{}", file.path))
        .collect();
    let file_scores = db::usage::score_map(&conn, "file", &file_keys)?;
    sort_by_usage(&mut files, &file_scores, |file| {
        format!("file:{}", file.path)
    });

    let command_keys: Vec<String> = commands
        .iter()
        .map(|command| format!("cmd:{}:{}", command.ext_id, command.command_id))
        .collect();
    let command_scores = db::usage::score_map(&conn, "command", &command_keys)?;
    sort_by_usage(&mut commands, &command_scores, |command| {
        format!("cmd:{}:{}", command.ext_id, command.command_id)
    });

    // Extração de ícones pode custar alguns ms; fora do lock do banco.
    drop(conn);
    for app in &mut apps {
        app.icon_path = platform::icons::data_url(&app.exe_path);
    }

    Ok(SearchResults {
        apps,
        files,
        clipboard,
        commands,
        calculation: calculate(q),
    })
}

fn sort_by_usage<T>(items: &mut [T], scores: &HashMap<String, i64>, key: impl Fn(&T) -> String) {
    items.sort_by_key(|item| std::cmp::Reverse(scores.get(&key(item)).copied().unwrap_or(0)));
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
    let mut apps = db::apps::search(&conn, &query, limit.unwrap_or(8))?;
    drop(conn);
    for app in &mut apps {
        app.icon_path = platform::icons::data_url(&app.exe_path);
    }
    Ok(apps)
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
pub fn open_path(path: String) -> AppResult<()> {
    platform::launch(&path)
}

#[tauri::command]
pub fn set_native_dialog_open(open: bool) {
    crate::state::NATIVE_DIALOG_OPEN.store(open, std::sync::atomic::Ordering::SeqCst);
}

#[tauri::command]
pub fn open_extension_logs() -> AppResult<()> {
    let dir = paths::extension_logs_dir();
    std::fs::create_dir_all(&dir)?;
    platform::launch(&dir.to_string_lossy())
}

#[tauri::command]
pub fn notify(app: AppHandle, title: String, body: String) -> AppResult<()> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| AppError::Platform(e.to_string()))
}

#[tauri::command]
pub fn touch_usage(state: State<AppState>, kind: String, key: String) -> AppResult<()> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::usage::touch(&conn, &kind, &key)
}

#[tauri::command]
pub fn copy_clipboard(state: State<AppState>, id: i64) -> AppResult<()> {
    let entry = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::clipboard::get(&conn, id)?
            .ok_or_else(|| AppError::NotFound("item de clipboard não encontrado".into()))?
    };

    let mut clipboard = arboard::Clipboard::new().map_err(|e| AppError::Platform(e.to_string()))?;
    if entry.kind == "image" {
        let path = entry
            .image_path
            .ok_or_else(|| AppError::NotFound("imagem do clipboard não encontrada".into()))?;
        let image = image::open(&path).map_err(|e| AppError::Platform(e.to_string()))?;
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        clipboard
            .set_image(arboard::ImageData {
                width: width as usize,
                height: height as usize,
                bytes: std::borrow::Cow::Owned(rgba.into_raw()),
            })
            .map_err(|e| AppError::Platform(e.to_string()))?;
    } else {
        clipboard
            .set_text(entry.text.unwrap_or_default())
            .map_err(|e| AppError::Platform(e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub fn clipboard_image(state: State<AppState>, id: i64) -> AppResult<String> {
    use base64::Engine as _;

    let path = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::clipboard::get(&conn, id)?
            .and_then(|entry| entry.image_path)
            .ok_or_else(|| AppError::NotFound("imagem do clipboard não encontrada".into()))?
    };
    let bytes = std::fs::read(path)?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[tauri::command]
pub fn copy_text(text: String) -> AppResult<()> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| AppError::Platform(e.to_string()))?;
    clipboard
        .set_text(text)
        .map_err(|e| AppError::Platform(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn list_clipboard(state: State<AppState>, limit: Option<usize>) -> AppResult<Vec<ClipEntry>> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::clipboard::recent(&conn, limit.unwrap_or(50))
}

#[tauri::command]
pub fn delete_clipboard(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::clipboard::delete(&conn, id)
}

#[tauri::command]
pub fn clear_unpinned_clipboard(state: State<AppState>) -> AppResult<usize> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::clipboard::clear_unpinned(&conn)
}

#[tauri::command]
pub fn pin_clipboard(state: State<AppState>, id: i64, pinned: bool) -> AppResult<()> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::clipboard::set_pinned(&conn, id, pinned)
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

/// Dispara a reindexação de arquivos em background.
#[tauri::command]
pub fn rebuild_files_index(app: AppHandle, state: State<AppState>) -> AppResult<()> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))?
        .clone();
    let db_path = paths::db_path(&app)?;
    let dirs = cfg.search.index_dirs.clone();
    let excludes = cfg.search.exclude_patterns.clone();

    std::thread::spawn(move || {
        crate::indexer::run_file_index(db_path, dirs, excludes);
    });
    Ok(())
}

#[tauri::command]
pub async fn get_status(app: AppHandle, state: State<'_, AppState>) -> AppResult<StatusInfo> {
    let (cfg, db_path) = {
        let cfg = state
            .config
            .lock()
            .map_err(|_| AppError::Config("config lock envenenado".into()))?
            .clone();
        let db_path = paths::db_path(&app)?;
        (cfg, db_path)
    };
    let paths_info = paths_info(&app, &cfg)?;
    let version = env!("CARGO_PKG_VERSION").to_string();

    // Enumerar janelas e consultar o banco pode custar alguns milissegundos;
    // mantém fora da thread da interface.
    tauri::async_runtime::spawn_blocking(move || -> AppResult<StatusInfo> {
        let conn = db::open(&db_path)?;
        let apps_indexed = conn
            .query_row("SELECT COUNT(*) FROM apps_index", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap_or(0);
        Ok(StatusInfo {
            version,
            apps_indexed,
            files_indexed: db::files::count(&conn).unwrap_or(0),
            clipboard_count: db::clipboard::count(&conn).unwrap_or(0),
            extensions_count: db::extensions::count(&conn).unwrap_or(0),
            open_windows: platform::windows::list_open_windows().len(),
            win_hook_active: crate::core::win_hook::is_installed(),
            paths: paths_info,
        })
    })
    .await
    .map_err(|e| AppError::Platform(format!("falha ao consultar o status: {e}")))?
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRequest {
    pub kind: String,
    pub value: String,
}

#[tauri::command]
pub fn list_extensions(state: State<AppState>) -> AppResult<Vec<InstalledExtension>> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::extensions::list(&conn)
}

#[tauri::command]
pub async fn install_extension(
    app: AppHandle,
    state: State<'_, AppState>,
    request: InstallRequest,
) -> AppResult<InstalledExtension> {
    let root = paths::extensions_dir(&app)?;
    let kind = request.kind;
    let value = request.value;
    let install_kind = kind.clone();
    let install_value = value.clone();

    // Clonar do Git pode levar segundos: não bloqueia a thread da interface.
    let (manifest, destination) = tauri::async_runtime::spawn_blocking(move || {
        let manifest = match install_kind.as_str() {
            "local" => extensions::install_from_dir(&root, std::path::Path::new(&install_value)),
            "git" => extensions::install_from_git(&root, &install_value),
            other => {
                return Err(format!("tipo de instalação desconhecido: {other}"));
            }
        }?;
        let destination = root.join(extensions::folder_name(&manifest.id));
        Ok((manifest, destination))
    })
    .await
    .map_err(|e| AppError::Platform(format!("falha ao instalar extensão: {e}")))?
    .map_err(AppError::Platform)?;

    {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::extensions::upsert(
            &conn,
            &manifest,
            &kind,
            Some(&value),
            &destination.to_string_lossy(),
        )?;
    }

    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::extensions::get_installed(&conn, &manifest.id)?
        .ok_or_else(|| AppError::NotFound("extensão não encontrada após instalar".into()))
}

#[tauri::command]
pub fn uninstall_extension(
    app: AppHandle,
    state: State<AppState>,
    ext_id: String,
) -> AppResult<()> {
    let root = paths::extensions_dir(&app)?;
    extensions::uninstall(&root, &ext_id).map_err(AppError::Platform)?;
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::extensions::remove(&conn, &ext_id)
}

#[tauri::command]
pub fn set_extension_enabled(
    state: State<AppState>,
    ext_id: String,
    enabled: bool,
) -> AppResult<()> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::extensions::set_enabled(&conn, &ext_id, enabled)
}

#[tauri::command]
pub fn set_extension_permissions(
    state: State<AppState>,
    ext_id: String,
    permissions: Vec<String>,
) -> AppResult<InstalledExtension> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::extensions::set_permissions(&conn, &ext_id, &permissions)?;
    db::extensions::get_installed(&conn, &ext_id)?
        .ok_or_else(|| AppError::NotFound("extensão não encontrada".into()))
}

#[tauri::command]
pub async fn run_extension_command(
    state: State<'_, AppState>,
    ext_id: String,
    command_id: String,
    args: Option<serde_json::Value>,
) -> AppResult<RunResult> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))?
        .clone();

    let row = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::extensions::get_row(&conn, &ext_id)?
            .ok_or_else(|| AppError::NotFound("extensão não instalada".into()))?
    };

    if !row.enabled {
        return Err(AppError::Platform("extensão desabilitada".into()));
    }
    if row.pending {
        return Err(AppError::Platform(
            "permissões da extensão não aprovadas".into(),
        ));
    }
    if !row.manifest.commands.iter().any(|c| c.id == command_id) {
        return Err(AppError::NotFound("comando não encontrado".into()));
    }

    let deno =
        extensions::runtime::find_deno(cfg.extensions.deno_path.as_deref()).ok_or_else(|| {
            AppError::Platform("runtime Deno não encontrado. Instale em 🧩 Extensões.".into())
        })?;

    let workspace = paths::expand(&cfg.workspace.path);
    let runtime_dir = paths::runtime_dir();
    let install_path = std::path::PathBuf::from(&row.install_path);
    let manifest = row.manifest.clone();
    let granted = row.granted.clone();
    let args = args.unwrap_or(serde_json::Value::Null);

    // A extensão pode levar até 20s (timeout do runner): executa fora da
    // thread da interface para o launcher não congelar.
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        extensions::runtime::run_command(
            &deno,
            &runtime_dir,
            &install_path,
            &manifest,
            &command_id,
            args,
            &granted,
            Some(&workspace),
        )
    })
    .await
    .map_err(|e| AppError::Platform(format!("falha ao executar a extensão: {e}")))?
    .map_err(AppError::Platform)?;

    if !outcome.ok {
        let hint = outcome
            .log_path
            .as_deref()
            .map(|path| format!("\nLog: {path}"))
            .unwrap_or_default();
        return Err(AppError::Platform(format!(
            "{}{hint}",
            outcome.error.unwrap_or_else(|| "falha na extensão".into())
        )));
    }

    // `ui.toast` acompanha a ação; antes ele fazia o host retornar cedo e
    // descartava um `copy`/`open` pedido no mesmo comando.
    let toast = outcome.toast;
    let result = outcome.result;

    if let Some(object) = result.as_object() {
        match object.get("type").and_then(|value| value.as_str()) {
            Some("copy") => {
                if !row.granted.iter().any(|p| p == "clipboard:write") {
                    return Err(AppError::Platform(
                        "permissão 'clipboard:write' não concedida".into(),
                    ));
                }
                let text = object
                    .get("text")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                let mut clipboard =
                    arboard::Clipboard::new().map_err(|e| AppError::Platform(e.to_string()))?;
                clipboard
                    .set_text(text.clone())
                    .map_err(|e| AppError::Platform(e.to_string()))?;
                return Ok(RunResult {
                    kind: "copy".into(),
                    text: Some(text),
                    toast,
                });
            }
            Some("open") => {
                if !row.granted.iter().any(|p| p == "shell") {
                    return Err(AppError::Platform("permissão 'shell' não concedida".into()));
                }
                let path = object
                    .get("path")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                platform::launch(path)?;
                return Ok(RunResult {
                    kind: "opened".into(),
                    text: None,
                    toast,
                });
            }
            Some("message") => {
                let text = object
                    .get("text")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                return Ok(RunResult {
                    kind: "message".into(),
                    text: Some(text),
                    toast: None,
                });
            }
            _ => {}
        }
    }

    if let Some(text) = result.as_str() {
        return Ok(RunResult {
            kind: "message".into(),
            text: Some(text.to_string()),
            toast: None,
        });
    }

    if let Some(toast) = toast {
        return Ok(RunResult {
            kind: "message".into(),
            text: Some(toast),
            toast: None,
        });
    }

    Ok(RunResult {
        kind: "done".into(),
        text: None,
        toast: None,
    })
}

#[tauri::command]
pub async fn runtime_status(state: State<'_, AppState>) -> AppResult<RuntimeStatus> {
    let configured = state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))?
        .extensions
        .deno_path
        .clone();

    // `deno --version` inicia um processo: nunca na thread da interface.
    tauri::async_runtime::spawn_blocking(move || {
        let found = extensions::runtime::find_deno(configured.as_deref());
        RuntimeStatus {
            available: found.is_some(),
            path: found.as_ref().map(|path| path.display().to_string()),
            version: found
                .as_ref()
                .and_then(|path| extensions::runtime::version(path)),
        }
    })
    .await
    .map_err(|e| AppError::Platform(format!("falha ao consultar o runtime: {e}")))
}

#[tauri::command]
pub async fn ensure_runtime(state: State<'_, AppState>) -> AppResult<RuntimeStatus> {
    let configured = state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))?
        .extensions
        .deno_path
        .clone();

    let status = tauri::async_runtime::spawn_blocking(move || {
        let deno = extensions::runtime::ensure_deno(configured.as_deref())?;
        Ok::<RuntimeStatus, String>(RuntimeStatus {
            available: true,
            path: Some(deno.display().to_string()),
            version: extensions::runtime::version(&deno),
        })
    })
    .await
    .map_err(|e| AppError::Platform(e.to_string()))?
    .map_err(AppError::Platform)?;

    Ok(status)
}

#[tauri::command]
pub async fn list_store(
    state: State<'_, AppState>,
    url: Option<String>,
) -> AppResult<Vec<StoreItem>> {
    let configured = state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))?
        .extensions
        .store_url
        .clone();
    let target = url
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(configured);

    // Download de índice da loja: fora da thread da interface.
    tauri::async_runtime::spawn_blocking(move || -> AppResult<Vec<StoreItem>> {
        let raw = fetch_text(&target).map_err(AppError::Platform)?;
        serde_json::from_str(&raw)
            .map_err(|e| AppError::Config(format!("índice da loja inválido: {e}")))
    })
    .await
    .map_err(|e| AppError::Platform(format!("falha ao carregar a loja: {e}")))?
}

fn fetch_text(target: &str) -> Result<String, String> {
    if target.starts_with("http://") || target.starts_with("https://") {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(15))
            .timeout_read(Duration::from_secs(30))
            .timeout(Duration::from_secs(60))
            .build();
        let response = agent.get(target).call().map_err(|e| e.to_string())?;
        response.into_string().map_err(|e| e.to_string())
    } else {
        let path = paths::expand(target);
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn list_monitors() -> Vec<MonitorInfo> {
    window_manager::list_monitors()
}

#[tauri::command]
pub fn list_windows_with_monitor() -> Vec<WindowInfo> {
    let monitors = window_manager::list_monitors();
    window_manager::snapshot_windows(&monitors)
}

#[tauri::command]
pub fn list_window_layouts(state: State<AppState>) -> AppResult<Vec<WindowLayout>> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::windows::list_layouts(&conn)
}

#[tauri::command]
pub fn save_current_layout(
    app: AppHandle,
    state: State<AppState>,
    name: String,
    hotkey: Option<String>,
) -> AppResult<WindowLayout> {
    let monitors = window_manager::list_monitors();
    let slots = window_manager::capture_layout(&monitors);
    if slots.is_empty() {
        return Err(AppError::Platform(
            "nenhuma janela capturável encontrada".into(),
        ));
    }
    let hotkey = hotkey.filter(|value| !value.trim().is_empty());
    if let Some(combo) = &hotkey {
        let check = validate_hotkey_inner(&app, state.inner(), combo, None, false)?;
        if !check.valid {
            return Err(AppError::Config(
                check.conflict.unwrap_or_else(|| "atalho inválido".into()),
            ));
        }
    }

    let id = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::windows::save_layout(&conn, &name, hotkey.as_deref(), &monitors, &slots)?
    };

    reload_shortcuts(&app, state.inner())?;

    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::windows::get_layout(&conn, id)?
        .ok_or_else(|| AppError::NotFound("layout não encontrado".into()))
}

#[tauri::command]
pub fn delete_window_layout(app: AppHandle, state: State<AppState>, id: i64) -> AppResult<()> {
    {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::windows::delete_layout(&conn, id)?;
    }
    reload_shortcuts(&app, state.inner())
}

#[tauri::command]
pub fn apply_window_layout(
    app: AppHandle,
    state: State<AppState>,
    id: i64,
) -> AppResult<ApplyResult> {
    let result = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        window_manager::apply_layout_by_id(&conn, id).map_err(AppError::Platform)?
    };
    remember_layout(&app, state.inner(), id);
    Ok(result)
}

#[tauri::command]
pub fn set_window_layout_hotkey(
    app: AppHandle,
    state: State<AppState>,
    id: i64,
    hotkey: Option<String>,
) -> AppResult<()> {
    let hotkey = hotkey.filter(|value| !value.trim().is_empty());
    if let Some(combo) = &hotkey {
        let check = validate_hotkey_inner(&app, state.inner(), combo, Some(id), false)?;
        if !check.valid {
            return Err(AppError::Config(
                check.conflict.unwrap_or_else(|| "atalho inválido".into()),
            ));
        }
    }
    {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::windows::set_layout_hotkey(&conn, id, hotkey.as_deref())?;
    }
    reload_shortcuts(&app, state.inner())
}

#[tauri::command]
pub fn list_window_rules(state: State<AppState>) -> AppResult<Vec<WindowRule>> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::windows::list_rules(&conn)
}

#[tauri::command]
pub fn upsert_window_rule(state: State<AppState>, rule: WindowRule) -> AppResult<i64> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::windows::upsert_rule(&conn, &rule)
}

#[tauri::command]
pub fn delete_window_rule(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::windows::delete_rule(&conn, id)
}

#[tauri::command]
pub fn apply_window_rules(state: State<AppState>) -> AppResult<ApplyResult> {
    let rules = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::windows::list_rules(&conn)?
    };
    let monitors = window_manager::list_monitors();
    Ok(window_manager::apply_rules(&rules, &monitors))
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
pub fn quit_app(app: AppHandle, state: State<AppState>) {
    if let Ok(conn) = state.db.lock() {
        db::checkpoint(&conn);
    }
    app.exit(0);
}
