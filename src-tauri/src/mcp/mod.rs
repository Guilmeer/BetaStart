//! Núcleo do servidor MCP: contexto, JSON-RPC e loop stdio.

pub mod auth;
pub mod http;
mod tools;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::core::config::Config;
use crate::error::AppResult;
use crate::models::McpClient;

pub struct McpContext {
    pub config: Config,
    pub config_path: PathBuf,
    pub db: rusqlite::Connection,
    pub workspace: PathBuf,
}

pub fn default_app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.betastart.app")
}

impl McpContext {
    pub fn load() -> AppResult<Self> {
        Self::load_from(&default_app_data_dir())
    }

    pub fn load_from(app_data: &Path) -> AppResult<Self> {
        let config_path = app_data.join("config.json");
        let config = Config::load_or_default(&config_path)?;
        crate::core::paths::ensure_workspace(&config.workspace.path)?;
        let db = crate::db::open(&app_data.join("data.db"))?;
        let workspace = crate::core::paths::expand(&config.workspace.path);
        Ok(Self {
            config,
            config_path,
            db,
            workspace,
        })
    }

    pub fn reindex(&self) -> AppResult<usize> {
        crate::memory::reindex(&self.db, &self.workspace)
    }

    pub fn save_config(&self) -> AppResult<()> {
        self.config.save(&self.config_path)
    }
}

pub fn http_running() -> bool {
    http::HTTP_RUNNING.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn handle(ctx: &mut McpContext, client: &McpClient, message: Value) -> Option<Value> {
    let id = message.get("id").cloned();
    let method = message
        .get("method")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(Value::Null);

    let result = match method {
        "initialize" => Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "betastart",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "notifications/initialized" | "notifications/cancelled" => None,
        "ping" => Some(json!({})),
        "tools/list" => Some(json!({ "tools": tools::definitions() })),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
            Some(match tools::call(ctx, client, name, arguments) {
                Ok(text) => json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": false
                }),
                Err(error) => json!({
                    "content": [{ "type": "text", "text": error }],
                    "isError": true
                }),
            })
        }
        "resources/list" => Some(json!({ "resources": [] })),
        "prompts/list" => Some(json!({ "prompts": [] })),
        _ => {
            let id = id?;
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("method not found: {method}")
                }
            }));
        }
    };

    let id = id?;
    result.map(|result| json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn parse_token_arg(args: &[String]) -> Option<String> {
    let mut iterator = args.iter().peekable();
    while let Some(arg) = iterator.next() {
        if arg == "--token" {
            return iterator.next().cloned();
        }
        if let Some(value) = arg.strip_prefix("--token=") {
            return Some(value.to_string());
        }
    }
    None
}

pub fn run_stdio() {
    let args: Vec<String> = std::env::args().collect();
    let token = parse_token_arg(&args).or_else(|| std::env::var("BETASTART_MCP_TOKEN").ok());

    let Some(token) = token else {
        eprintln!("[betastart mcp] token ausente: use --token <token> ou BETASTART_MCP_TOKEN");
        std::process::exit(2);
    };

    let mut ctx = match McpContext::load() {
        Ok(ctx) => ctx,
        Err(error) => {
            eprintln!("[betastart mcp] não foi possível carregar o estado: {error}");
            std::process::exit(3);
        }
    };

    let client = match crate::db::mcp::verify_token(&ctx.db, &token) {
        Ok(Some(client)) => client,
        Ok(None) => {
            eprintln!("[betastart mcp] token inválido ou revogado");
            std::process::exit(4);
        }
        Err(error) => {
            eprintln!("[betastart mcp] erro ao verificar token: {error}");
            std::process::exit(5);
        }
    };

    eprintln!(
        "[betastart mcp] stdio ativo para '{}' (escopos: {:?})",
        client.name, client.scopes
    );

    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let message: Value = match serde_json::from_str(line) {
            Ok(message) => message,
            Err(error) => {
                eprintln!("[betastart mcp] JSON inválido: {error}");
                continue;
            }
        };
        if let Some(response) = handle(&mut ctx, &client, message) {
            if writeln!(stdout, "{response}").is_err() {
                break;
            }
            let _ = stdout.flush();
        }
    }
}

pub fn start_http(app_data: PathBuf, host: String, port: u16) {
    http::start(app_data, host, port);
}

/// CLI headless: cria um cliente com todos os escopos e imprime o token.
pub fn run_token_cli() {
    let args: Vec<String> = std::env::args().collect();
    let name = args
        .iter()
        .position(|arg| arg == "mcp-token")
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| "Cliente MCP".to_string());

    let ctx = match McpContext::load() {
        Ok(ctx) => ctx,
        Err(error) => {
            eprintln!("[betastart mcp-token] {error}");
            std::process::exit(2);
        }
    };

    let token = auth::generate_token();
    let hash = match auth::hash_token(&token) {
        Ok(hash) => hash,
        Err(error) => {
            eprintln!("[betastart mcp-token] {error}");
            std::process::exit(3);
        }
    };

    let scopes: Vec<String> = [
        "memory",
        "tasks",
        "windows",
        "extensions",
        "search",
        "settings",
    ]
    .iter()
    .map(|scope| scope.to_string())
    .collect();

    if let Err(error) = crate::db::mcp::create_client(&ctx.db, &name, &hash, &scopes) {
        eprintln!("[betastart mcp-token] {error}");
        std::process::exit(4);
    }

    println!("{token}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_context() -> (PathBuf, McpContext) {
        let root = std::env::temp_dir().join(format!(
            "betastart-mcp-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp dir");

        let workspace = root.join("workspace");
        let mut config = Config::default();
        config.workspace.path = workspace.to_string_lossy().to_string();
        config.save(&root.join("config.json")).expect("config");

        let ctx = McpContext::load_from(&root).expect("context");
        (root, ctx)
    }

    fn test_client(scopes: Vec<&str>) -> McpClient {
        McpClient {
            id: 1,
            name: "test".into(),
            scopes: scopes.into_iter().map(|scope| scope.to_string()).collect(),
            created_at: 0,
            last_seen: None,
            revoked: false,
        }
    }

    #[test]
    fn initialize_lists_tools_and_calls_tool() {
        let (root, mut ctx) = test_context();
        let client = test_client(vec![
            "memory",
            "tasks",
            "windows",
            "extensions",
            "search",
            "settings",
        ]);

        let init = handle(
            &mut ctx,
            &client,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
        )
        .expect("initialize response");
        assert_eq!(init["result"]["serverInfo"]["name"], "betastart");

        let list = handle(
            &mut ctx,
            &client,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
        )
        .expect("tools/list response");
        let tools = list["result"]["tools"].as_array().expect("tools array");
        assert!(tools.len() >= 15);
        assert!(tools.iter().any(|tool| tool["name"] == "memory.list"));

        let call = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": { "name": "memory.list", "arguments": {} }
            }),
        )
        .expect("tools/call response");
        assert_eq!(call["result"]["isError"], false);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn denies_tool_without_scope() {
        let (root, mut ctx) = test_context();
        let client = test_client(vec![]);

        let call = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": { "name": "memory.list", "arguments": {} }
            }),
        )
        .expect("response");
        assert_eq!(call["result"]["isError"], true);

        let _ = std::fs::remove_dir_all(&root);
    }
}
