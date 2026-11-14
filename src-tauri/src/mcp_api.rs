//! Comandos Tauri para gerenciar clientes/tokens MCP.

use serde::Serialize;
use tauri::State;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::mcp;
use crate::models::{McpAudit, McpClient, McpClientCreated};
use crate::state::AppState;

const SCOPES: &[&str] = &[
    "memory",
    "tasks",
    "windows",
    "extensions",
    "search",
    "settings",
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub enabled: bool,
    pub http_enabled: bool,
    pub host: String,
    pub port: u16,
    pub running: bool,
    pub clients: i64,
}

#[tauri::command]
pub fn mcp_status(state: State<AppState>) -> AppResult<McpStatus> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))?;
    let clients = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::mcp::count(&conn)?
    };

    Ok(McpStatus {
        enabled: cfg.mcp.enabled,
        http_enabled: cfg.mcp.http.enabled,
        host: cfg.mcp.http.host.clone(),
        port: cfg.mcp.http.port,
        running: mcp::http_running(),
        clients,
    })
}

#[tauri::command]
pub fn mcp_create_client(
    state: State<AppState>,
    name: String,
    scopes: Vec<String>,
) -> AppResult<McpClientCreated> {
    let name = if name.trim().is_empty() {
        "Cliente MCP".to_string()
    } else {
        name.trim().to_string()
    };
    let scopes: Vec<String> = scopes
        .into_iter()
        .filter(|scope| SCOPES.contains(&scope.as_str()))
        .collect();
    if scopes.is_empty() {
        return Err(AppError::Config(
            "selecione ao menos um escopo para o cliente".into(),
        ));
    }

    let token = mcp::auth::generate_token();
    let hash = mcp::auth::hash_token(&token).map_err(AppError::Platform)?;

    let id = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::mcp::create_client(&conn, &name, &hash, &scopes)?
    };

    let client = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::mcp::list_clients(&conn)?
            .into_iter()
            .find(|client| client.id == id)
            .ok_or_else(|| AppError::NotFound("cliente MCP não encontrado".into()))?
    };

    Ok(McpClientCreated { token, client })
}

#[tauri::command]
pub fn mcp_list_clients(state: State<AppState>) -> AppResult<Vec<McpClient>> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::mcp::list_clients(&conn)
}

#[tauri::command]
pub fn mcp_revoke_client(state: State<AppState>, id: i64) -> AppResult<()> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::mcp::revoke_client(&conn, id)
}

#[tauri::command]
pub fn mcp_audit(state: State<AppState>, limit: Option<usize>) -> AppResult<Vec<McpAudit>> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::mcp::recent_audit(&conn, limit.unwrap_or(50))
}

#[tauri::command]
pub fn mcp_exe_path() -> String {
    std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_default()
}
