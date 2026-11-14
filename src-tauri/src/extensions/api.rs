//! Ponte local entre uma extensão isolada e as APIs do app.
//!
//! O runner Deno chama `http://127.0.0.1:<porta>/tool` com um token efêmero.
//! O token carrega a extensão e as permissões aprovadas; o servidor só expõe
//! as tools MCP correspondentes. Assim o SDK não precisa de acesso direto ao
//! banco nem ao sistema de arquivos do app.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Mutex, OnceLock};

use axum::extract::State as AxumState;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::McpContext;
use crate::models::McpClient;

static PORT: AtomicU16 = AtomicU16::new(0);
static TOKENS: OnceLock<Mutex<HashMap<String, ExtensionAuth>>> = OnceLock::new();

#[derive(Clone)]
pub struct ExtensionAuth {
    pub ext_id: String,
    pub granted: Vec<String>,
}

pub struct HostBridge {
    pub url: String,
    pub token: String,
    pub port: u16,
}

#[derive(Clone)]
struct ApiState {
    app_data: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolRequest {
    token: String,
    name: String,
    #[serde(default)]
    args: Value,
}

#[derive(Deserialize)]
struct ClipboardRequest {
    token: String,
}

fn tokens() -> &'static Mutex<HashMap<String, ExtensionAuth>> {
    TOKENS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Cria um token efêmero para uma execução de extensão.
pub fn bridge(ext_id: &str, granted: &[String]) -> Option<HostBridge> {
    let port = PORT.load(Ordering::SeqCst);
    if port == 0 {
        return None;
    }
    let token = uuid::Uuid::new_v4().simple().to_string();
    tokens().lock().ok()?.insert(
        token.clone(),
        ExtensionAuth {
            ext_id: ext_id.to_string(),
            granted: granted.to_vec(),
        },
    );
    Some(HostBridge {
        url: format!("http://127.0.0.1:{port}"),
        token,
        port,
    })
}

pub fn revoke(token: &str) {
    if let Ok(mut tokens) = tokens().lock() {
        tokens.remove(token);
    }
}

/// Sobe o servidor em uma porta livre de localhost. Retorna a porta usada.
pub fn start(app_data: PathBuf) -> Result<u16, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    PORT.store(port, Ordering::SeqCst);

    let router = Router::new()
        .route("/tool", post(tool))
        .route("/clipboard/read", post(clipboard_read))
        .with_state(ApiState { app_data });

    // Thread própria para funcionar tanto no app (Tauri) quanto no processo
    // headless do MCP stdio, onde não há runtime do Tauri inicializado.
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("[betastart ext] falha ao criar runtime da API: {error}");
                return;
            }
        };
        runtime.block_on(async move {
            match tokio::net::TcpListener::from_std(listener) {
                Ok(listener) => {
                    if let Err(error) = axum::serve(listener, router).await {
                        eprintln!("[betastart ext] servidor de API parou: {error}");
                    }
                }
                Err(error) => eprintln!("[betastart ext] falha ao abrir a API local: {error}"),
            }
        });
    });

    Ok(port)
}

fn authorize(token: &str) -> Result<ExtensionAuth, Response> {
    let Ok(tokens) = tokens().lock() else {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "token store").into_response());
    };
    tokens
        .get(token)
        .cloned()
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "invalid token").into_response())
}

fn has_permission(auth: &ExtensionAuth, permission: &str) -> bool {
    auth.granted.iter().any(|item| item == permission)
}

/// Mapeia as permissões do manifesto para os escopos das tools MCP.
fn synthetic_scopes(auth: &ExtensionAuth) -> Vec<String> {
    let mut scopes = Vec::new();
    for (permission, scope) in [
        ("memory", "memory"),
        ("tasks", "tasks"),
        ("windows", "windows"),
        ("search", "search"),
        // `clipboard.list` é a tool de leitura do clipboard no MCP.
        ("clipboard:read", "search"),
    ] {
        if has_permission(auth, permission) && !scopes.iter().any(|item| item == scope) {
            scopes.push(scope.to_string());
        }
    }
    scopes
}

fn tool_allowed(auth: &ExtensionAuth, name: &str) -> bool {
    match name {
        "memory.list" | "memory.read" | "memory.write" | "memory.search" | "project.list"
        | "project.create" | "project.link" | "project.unlink" | "project.brief" | "graph.get" => {
            has_permission(auth, "memory")
        }
        "tasks.list" | "tasks.create" | "tasks.create_many" | "tasks.update" | "tasks.delete" => {
            has_permission(auth, "tasks")
        }
        "windows.list" | "windows.move_resize" | "windows.apply_layout" | "windows.save_layout" => {
            has_permission(auth, "windows")
        }
        "search.query" => has_permission(auth, "search"),
        "clipboard.list" => has_permission(auth, "clipboard:read"),
        _ => false,
    }
}

fn synthetic_client(auth: &ExtensionAuth) -> McpClient {
    McpClient {
        id: 0,
        name: format!("extension:{}", auth.ext_id),
        scopes: synthetic_scopes(auth),
        created_at: 0,
        last_seen: None,
        revoked: false,
    }
}

fn tool_result(response: Value) -> Result<Value, String> {
    if let Some(error) = response.get("error") {
        return Err(error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("erro na tool")
            .to_string());
    }
    let result = response.get("result").cloned().unwrap_or(Value::Null);
    let is_error = result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let text = result
        .get("content")
        .and_then(|content| content.get(0))
        .and_then(|content| content.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if is_error {
        return Err(text.to_string());
    }
    // As tools devolvem JSON em texto; mantém strings simples como estão.
    Ok(serde_json::from_str::<Value>(text).unwrap_or_else(|_| Value::String(text.to_string())))
}

async fn tool(AxumState(state): AxumState<ApiState>, Json(request): Json<ToolRequest>) -> Response {
    let auth = match authorize(&request.token) {
        Ok(auth) => auth,
        Err(response) => return response,
    };
    if !tool_allowed(&auth, &request.name) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "ok": false,
                "error": format!("permission denied: {}", request.name)
            })),
        )
            .into_response();
    }

    let name = request.name.clone();
    let args = request.args;
    let app_data = state.app_data.clone();
    let result = tokio::task::spawn_blocking(move || {
        let mut ctx = McpContext::load_from(&app_data).map_err(|e| e.to_string())?;
        let client = synthetic_client(&auth);
        let message = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": name, "arguments": args }
        });
        let response = crate::mcp::handle(&mut ctx, &client, message)
            .ok_or_else(|| "sem resposta do host".to_string())?;
        tool_result(response)
    })
    .await;

    match result {
        Ok(Ok(value)) => Json(json!({ "ok": true, "result": value })).into_response(),
        Ok(Err(error)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": error })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": error.to_string() })),
        )
            .into_response(),
    }
}

#[cfg(test)]
pub fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth(granted: &[&str]) -> ExtensionAuth {
        ExtensionAuth {
            ext_id: "autor/ext".into(),
            granted: granted.iter().map(|item| item.to_string()).collect(),
        }
    }

    #[test]
    fn maps_permissions_to_scopes() {
        let scopes = synthetic_scopes(&auth(&["memory", "clipboard:read"]));
        assert!(scopes.contains(&"memory".to_string()));
        assert!(scopes.contains(&"search".to_string()));
        assert!(!scopes.contains(&"tasks".to_string()));
    }

    #[test]
    fn restricts_tools_by_permission() {
        let memory = auth(&["memory"]);
        assert!(tool_allowed(&memory, "memory.list"));
        assert!(!tool_allowed(&memory, "tasks.list"));

        let clipboard = auth(&["clipboard:read"]);
        assert!(tool_allowed(&clipboard, "clipboard.list"));
        assert!(!tool_allowed(&clipboard, "search.query"));
    }

    #[test]
    fn serves_tool_calls_with_ephemeral_token() {
        let _guard = test_lock();
        let root =
            std::env::temp_dir().join(format!("betastart-ext-api-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp dir");

        let mut config = crate::core::config::Config::default();
        config.workspace.path = root.join("workspace").to_string_lossy().to_string();
        config.save(&root.join("config.json")).expect("config");

        start(root.clone()).expect("api server");
        let bridge = bridge("test/ext", &["memory".to_string()]).expect("bridge");
        let url = format!("{}/tool", bridge.url);

        let response = ureq::post(&url)
            .set("content-type", "application/json")
            .send_string(
                &json!({
                    "token": bridge.token,
                    "name": "memory.list",
                    "args": {}
                })
                .to_string(),
            )
            .expect("memory.list");
        let payload: Value =
            serde_json::from_str(&response.into_string().expect("body")).expect("json");
        assert_eq!(payload["ok"], true);

        let error = ureq::post(&url)
            .set("content-type", "application/json")
            .send_string(
                &json!({
                    "token": bridge.token,
                    "name": "tasks.list",
                    "args": {}
                })
                .to_string(),
            )
            .expect_err("tasks.list deve ser negado");
        match error {
            ureq::Error::Status(code, _) => assert_eq!(code, 403),
            other => panic!("erro inesperado: {other}"),
        }

        revoke(&bridge.token);
        let error = ureq::post(&url)
            .set("content-type", "application/json")
            .send_string(
                &json!({
                    "token": bridge.token,
                    "name": "memory.list",
                    "args": {}
                })
                .to_string(),
            )
            .expect_err("token revogado");
        match error {
            ureq::Error::Status(code, _) => assert_eq!(code, 401),
            other => panic!("erro inesperado: {other}"),
        }

        let _ = std::fs::remove_dir_all(&root);
    }
}

async fn clipboard_read(
    AxumState(_): AxumState<ApiState>,
    Json(request): Json<ClipboardRequest>,
) -> Response {
    let auth = match authorize(&request.token) {
        Ok(auth) => auth,
        Err(response) => return response,
    };
    if !has_permission(&auth, "clipboard:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "ok": false, "error": "permission denied: clipboard:read" })),
        )
            .into_response();
    }

    let text = tokio::task::spawn_blocking(|| {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.get_text())
            .map_err(|e| e.to_string())
    })
    .await;

    match text {
        Ok(Ok(text)) => Json(json!({ "ok": true, "text": text })).into_response(),
        Ok(Err(error)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": error })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": error.to_string() })),
        )
            .into_response(),
    }
}
