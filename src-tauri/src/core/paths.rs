use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

/// Diretório de dados do app (`%APPDATA%\com.betastart.app` no Windows).
pub fn app_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Config(e.to_string()))?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn config_path(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app_data_dir(app)?.join("config.json"))
}

pub fn db_path(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app_data_dir(app)?.join("data.db"))
}

/// Expande `~/` para o home do usuário.
pub fn expand(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Garante que a árvore base do workspace de memória exista.
pub fn ensure_workspace(path: &str) -> AppResult<PathBuf> {
    let root = expand(path);
    for sub in ["global/inbox", "global/tasks", "projects"] {
        std::fs::create_dir_all(root.join(sub))?;
    }
    Ok(root)
}

/// Pasta onde as extensões ficam instaladas.
pub fn extensions_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app_data_dir(app)?.join("extensions");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Pasta do runtime (assets do runner + Deno gerenciado).
pub fn runtime_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("BetaStart")
        .join("runtime")
}

/// Pasta temporária para payload/resultado das execuções de extensão.
pub fn ext_temp_dir() -> PathBuf {
    std::env::temp_dir().join("betastart-ext")
}
