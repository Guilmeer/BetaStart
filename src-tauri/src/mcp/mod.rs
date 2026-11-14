//! Núcleo do servidor MCP: contexto, JSON-RPC e loop stdio.

pub mod auth;
pub mod http;
mod tools;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::core::config::Config;
use crate::error::AppResult;
use crate::memory::LinkedRoot;
use crate::models::McpClient;

pub struct McpContext {
    pub config: Config,
    pub config_path: PathBuf,
    pub db: rusqlite::Connection,
    pub workspace: PathBuf,
    pub linked: Vec<LinkedRoot>,
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
        let linked = LinkedRoot::from_config(&config.workspace);
        Ok(Self {
            config,
            config_path,
            db,
            workspace,
            linked,
        })
    }

    pub fn reindex(&self) -> AppResult<usize> {
        crate::memory::reindex_with_linked(&self.db, &self.workspace, &self.linked)
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
            "capabilities": {
                "tools": {},
                "resources": { "subscribe": false, "listChanged": false },
                "prompts": { "listChanged": false }
            },
            "serverInfo": {
                "name": "betastart",
                "version": env!("CARGO_PKG_VERSION")
            },
            "instructions": INSTRUCTIONS
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
            let outcome = tools::call(ctx, client, name, arguments);
            let (ok, detail, result) = match outcome {
                Ok(text) => (
                    true,
                    None,
                    json!({
                        "content": [{ "type": "text", "text": text }],
                        "isError": false
                    }),
                ),
                Err(error) => {
                    let detail: String = error.chars().take(500).collect();
                    (
                        false,
                        Some(detail),
                        json!({
                            "content": [{ "type": "text", "text": error }],
                            "isError": true
                        }),
                    )
                }
            };
            // Auditoria é best-effort: nunca derruba a chamada por causa do log.
            let _ = crate::db::mcp::audit(
                &ctx.db,
                client.id,
                &client.name,
                name,
                ok,
                detail.as_deref(),
            );
            Some(result)
        }
        "resources/list" => Some(list_resources(ctx, client)),
        "resources/read" => {
            let uri = params
                .get("uri")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            Some(read_resource(ctx, client, uri))
        }
        "prompts/list" => Some(list_prompts()),
        "prompts/get" => {
            let name = params
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
            Some(get_prompt(name, &arguments))
        }
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

fn hex_encode(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn hex_decode(value: &str) -> Option<String> {
    if value.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for chunk in value.as_bytes().chunks(2) {
        let text = std::str::from_utf8(chunk).ok()?;
        bytes.push(u8::from_str_radix(text, 16).ok()?);
    }
    String::from_utf8(bytes).ok()
}

/// Protocolo de leitura entregue ao cliente no `initialize`.
///
/// É a maior alavanca de economia: sem isso o modelo não sabe que
/// `project.brief`/`memory.context` existem e encadeia leituras amplas.
const INSTRUCTIONS: &str = "\
Beta Start is the user's local memory: Markdown notes, tasks, window management, search and clipboard.

READ PROTOCOL (keeps token cost low):
1. For a project you have not seen in this session, call `project.brief` first. Do not chain project.list + memory.list + tasks.list.
2. To locate a topic, call `memory.search`. Read only the returned files.
3. Before reading a file, call `memory.read` with mode:\"outline\"; then read one section with mode:\"range\" or the whole file with mode:\"full\".
4. For a question that spans notes and tasks, call `memory.context` once instead of several calls.
5. For relationships use `graph.query`, `graph.neighbors`, `graph.path` or `graph.explain`.
6. `graph.get` returns the ENTIRE graph (usually tens of thousands of tokens). Use it only when the user explicitly asks for the whole graph.
7. `memory.list` is paginated (default 200). Prefer `project.brief` for an overview.

WRITE PROTOCOL:
- Create tasks in batch with `tasks.create_many`.
- Prefer `tasks.update` for task fields instead of rewriting whole files.
- Paths returned by `project.brief`/`memory.list` are relative to the project; pass `project` together with them.
";

fn has_scope(client: &McpClient, scope: &str) -> bool {
    client.scopes.iter().any(|item| item == scope)
}

fn list_resources(ctx: &McpContext, client: &McpClient) -> Value {
    if !has_scope(client, "memory") {
        return json!({ "resources": [] });
    }
    let files = crate::db::memory::list_files(&ctx.db, None).unwrap_or_default();
    let resources: Vec<Value> = files
        .into_iter()
        .map(|file| {
            json!({
                "uri": format!("betastart://memory/{}", hex_encode(&file.path)),
                "name": if file.title.trim().is_empty() { file.name } else { file.title },
                "description": format!("{} · {}", file.kind, file.project),
                "mimeType": "text/markdown"
            })
        })
        .collect();
    json!({ "resources": resources })
}

fn read_resource(ctx: &McpContext, client: &McpClient, uri: &str) -> Value {
    if !has_scope(client, "memory") {
        return json!({
            "contents": [{
                "uri": uri,
                "mimeType": "text/plain",
                "text": "permission denied: scope 'memory' not granted"
            }]
        });
    }
    let Some(encoded) = uri.strip_prefix("betastart://memory/") else {
        return json!({ "contents": [] });
    };
    let Some(path) = hex_decode(encoded) else {
        return json!({ "contents": [] });
    };
    let path_buf = PathBuf::from(&path);
    if crate::memory::ensure_allowed(&ctx.workspace, &ctx.linked, &path_buf).is_err() {
        return json!({ "contents": [] });
    }
    let text = match std::fs::read_to_string(&path_buf) {
        Ok(text) => text,
        Err(error) => {
            return json!({
                "contents": [{
                    "uri": uri,
                    "mimeType": "text/plain",
                    "text": format!("não foi possível ler o recurso: {error}")
                }]
            });
        }
    };
    json!({
        "contents": [{
            "uri": uri,
            "mimeType": "text/markdown",
            "text": text
        }]
    })
}

fn list_prompts() -> Value {
    json!({
        "prompts": [
            {
                "name": "project-brief",
                "description": "Resume um projeto usando o brief, as notas e as tasks abertas.",
                "arguments": [
                    { "name": "project", "description": "Slug do projeto", "required": true }
                ]
            },
            {
                "name": "plan-project",
                "description": "Transforma um objetivo em um plano de tasks no projeto.",
                "arguments": [
                    { "name": "project", "description": "Slug do projeto", "required": true },
                    { "name": "goal", "description": "Objetivo do plano", "required": true }
                ]
            },
            {
                "name": "review-tasks",
                "description": "Revisa as tasks abertas e sugere prioridades.",
                "arguments": [
                    { "name": "project", "description": "Slug do projeto", "required": false }
                ]
            },
            {
                "name": "capture-note",
                "description": "Salva uma decisão/nota curta na memória do projeto.",
                "arguments": [
                    { "name": "project", "description": "Slug do projeto", "required": false },
                    { "name": "content", "description": "Conteúdo da nota", "required": true }
                ]
            }
        ]
    })
}

fn get_prompt(name: &str, args: &Value) -> Value {
    let arg = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let project = {
        let value = arg("project");
        if value.is_empty() {
            "global".to_string()
        } else {
            value
        }
    };
    let (description, text) = match name {
        "project-brief" => (
            "Resumo do projeto",
            format!(
                "Use a tool project.brief com project=\"{project}\" e resuma em português: objetivo, estado atual, tasks abertas e próximos passos. Não invente informações que não estejam na memória."
            ),
        ),
        "plan-project" => {
            let goal = arg("goal");
            (
                "Plano de projeto",
                format!(
                    "Objetivo do projeto \"{project}\": {goal}. Leia project.brief e as tasks existentes. Depois crie um plano enxuto com tasks usando tasks.create_many, com prioridade e vencimento quando fizer sentido."
                ),
            )
        }
        "review-tasks" => (
            "Revisão de tasks",
            format!(
                "Liste as tasks abertas do projeto \"{project}\" com tasks.list (status todo/in_progress), identifique o que está bloqueado ou atrasado e sugira as 3 próximas ações prioritárias."
            ),
        ),
        "capture-note" => {
            let content = arg("content");
            (
                "Captura de nota",
                format!(
                    "Salve a nota abaixo na memória do projeto \"{project}\" usando memory.write. Escolha um caminho e título descritivos dentro de projects/{project}/notes. Conteúdo:\n\n{content}"
                ),
            )
        }
        _ => ("Prompt desconhecido", String::new()),
    };
    json!({
        "description": description,
        "messages": [{
            "role": "user",
            "content": { "type": "text", "text": text }
        }]
    })
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

/// Log em arquivo (`%APPDATA%/com.betastart.app/mcp-stdio.log`).
///
/// O stderr do processo é engolido por vários clientes MCP; sem isso, uma
/// conexão que cai não deixa rastro nenhum para investigar.
fn stdio_log(message: &str) {
    use std::io::Write as _;

    let path = default_app_data_dir().join("mcp-stdio.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(metadata) = std::fs::metadata(&path) {
        if metadata.len() > 512 * 1024 {
            let _ = std::fs::remove_file(&path);
        }
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(
            file,
            "{} {message}",
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        );
    }
}

fn stdio_fail(code: i32, message: &str) -> ! {
    eprintln!("[betastart mcp] {message}");
    stdio_log(&format!("erro (exit {code}): {message}"));
    std::process::exit(code);
}

pub fn run_stdio() {
    let started = std::time::Instant::now();
    let args: Vec<String> = std::env::args().collect();
    let token = parse_token_arg(&args).or_else(|| std::env::var("BETASTART_MCP_TOKEN").ok());
    let trace = std::env::var("BETASTART_MCP_TRACE").map(|value| value != "0").unwrap_or(false);

    stdio_log(&format!(
        "inicio (pid {}, exe: {:?}, app_data: {:?}, trace: {trace})",
        std::process::id(),
        std::env::current_exe().ok(),
        default_app_data_dir()
    ));

    // Com `panic = "abort"` no release, um panic some sem mensagem no cliente.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        stdio_log(&format!("panic: {info}"));
        default_hook(info);
    }));

    let Some(token) = token else {
        stdio_fail(
            2,
            "token ausente: use --token <token> ou BETASTART_MCP_TOKEN",
        );
    };

    let mut ctx = match McpContext::load() {
        Ok(ctx) => ctx,
        Err(error) => stdio_fail(3, &format!("não foi possível carregar o estado: {error}")),
    };
    stdio_log(&format!(
        "contexto carregado em {:?}",
        started.elapsed()
    ));

    // O processo stdio também pode executar extension tools; a API local
    // precisa existir neste processo, não só no app com janela.
    if let Err(error) = crate::extensions::api::start(default_app_data_dir()) {
        stdio_log(&format!("API de extensões indisponível: {error}"));
    }

    let client = match crate::db::mcp::verify_token(&ctx.db, &token) {
        Ok(Some(client)) => client,
        Ok(None) => stdio_fail(4, "token inválido ou revogado"),
        Err(error) => stdio_fail(5, &format!("erro ao verificar token: {error}")),
    };

    eprintln!(
        "[betastart mcp] stdio ativo para '{}' (escopos: {:?})",
        client.name, client.scopes
    );
    stdio_log(&format!(
        "pronto em {:?}: cliente '{}', escopos {:?}",
        started.elapsed(),
        client.name,
        client.scopes
    ));

    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                stdio_log(&format!("falha ao ler stdin: {error}; encerrando"));
                break;
            }
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let message: Value = match serde_json::from_str(line) {
            Ok(message) => message,
            Err(error) => {
                eprintln!("[betastart mcp] JSON inválido: {error}");
                stdio_log(&format!("JSON inválido: {error}"));
                continue;
            }
        };
        if trace {
            let method = message
                .get("method")
                .and_then(|value| value.as_str())
                .unwrap_or("(resposta)");
            stdio_log(&format!("-> {method}"));
        }
        if let Some(response) = handle(&mut ctx, &client, message) {
            if writeln!(stdout, "{response}").is_err() {
                stdio_log("falha ao escrever stdout; encerrando");
                break;
            }
            let _ = stdout.flush();
        }
    }

    stdio_log("stdin fechado (EOF); encerrando normalmente");
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
        // O protocolo de leitura precisa chegar ao cliente.
        assert!(init["result"]["instructions"]
            .as_str()
            .unwrap_or_default()
            .contains("project.brief"));

        let list = handle(
            &mut ctx,
            &client,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
        )
        .expect("tools/list response");
        let tools = list["result"]["tools"].as_array().expect("tools array");
        assert!(tools.len() >= 15);
        assert!(tools.iter().any(|tool| tool["name"] == "memory.list"));
        for expected in [
            "memory.context",
            "graph.query",
            "graph.neighbors",
            "graph.path",
            "graph.explain",
        ] {
            assert!(
                tools.iter().any(|tool| tool["name"] == expected),
                "tool ausente: {expected}"
            );
        }

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

        let resources = handle(
            &mut ctx,
            &client,
            json!({ "jsonrpc": "2.0", "id": 4, "method": "resources/list", "params": {} }),
        )
        .expect("resources/list response");
        assert!(resources["result"]["resources"].is_array());

        let prompts = handle(
            &mut ctx,
            &client,
            json!({ "jsonrpc": "2.0", "id": 5, "method": "prompts/list", "params": {} }),
        )
        .expect("prompts/list response");
        assert!(prompts["result"]["prompts"].as_array().unwrap().len() >= 4);

        let prompt = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 6,
                "method": "prompts/get",
                "params": {
                    "name": "project-brief",
                    "arguments": { "project": "global" }
                }
            }),
        )
        .expect("prompts/get response");
        assert!(
            prompt["result"]["messages"][0]["content"]["text"]
                .as_str()
                .unwrap_or_default()
                .contains("global")
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn memory_tools_work_on_linked_projects() {
        let root = std::env::temp_dir().join(format!(
            "betastart-mcp-linked-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&root);
        let workspace = root.join("workspace");
        let linked_root = root.join("projeto-externo");
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::create_dir_all(&linked_root).expect("linked");
        std::fs::write(linked_root.join("README.md"), "# Linked\n").expect("readme");

        let mut config = Config::default();
        config.workspace.path = workspace.to_string_lossy().to_string();
        config.workspace.linked_projects.push(
            crate::core::config::LinkedProjectConfig {
                slug: "externo".into(),
                root: linked_root.to_string_lossy().to_string(),
                title: Some("Externo".into()),
            },
        );
        config.save(&root.join("config.json")).expect("config");

        let mut ctx = McpContext::load_from(&root).expect("context");
        ctx.reindex().expect("reindex");
        let client = test_client(vec!["memory"]);

        let read = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "memory.read",
                    "arguments": { "path": linked_root.join("README.md").to_string_lossy() }
                }
            }),
        )
        .expect("read response");
        assert_eq!(read["result"]["isError"], false);
        assert!(read["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("Linked"));

        let write = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "memory.write",
                    "arguments": {
                        "path": linked_root.join("novo.md").to_string_lossy(),
                        "content": "# Novo\n"
                    }
                }
            }),
        )
        .expect("write response");
        assert_eq!(write["result"]["isError"], false);

        let list = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "memory.list",
                    "arguments": { "project": "externo" }
                }
            }),
        )
        .expect("list response");
        assert_eq!(list["result"]["isError"], false);
        let text = list["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        assert!(text.contains("README.md"));
        assert!(text.contains("novo.md"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_link_tool_links_folder_and_writes_ignore() {
        let (root, mut ctx) = test_context();
        let external = root.join("Prisma");
        std::fs::create_dir_all(external.join("SkillAndPlugin")).expect("external");
        std::fs::write(external.join("MEMORIA.md"), "# Memoria\n").expect("note");
        let client = test_client(vec!["memory"]);

        let link = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "project.link",
                    "arguments": {
                        "root": external.to_string_lossy(),
                        "title": "Prisma Historico",
                        "ignore": ["SkillAndPlugin/"]
                    }
                }
            }),
        )
        .expect("link response");
        assert_eq!(link["result"]["isError"], false);
        let text = link["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        assert!(text.contains("prisma-historico"));

        let ignore = std::fs::read_to_string(external.join(".betastartignore")).expect("ignore");
        assert!(ignore.contains("SkillAndPlugin/"));

        let disk = Config::load_or_default(&root.join("config.json")).expect("config reload");
        assert!(disk
            .workspace
            .linked_projects
            .iter()
            .any(|item| item.slug == "prisma-historico"));

        // `.betastartignore` não é Markdown, mas memory.write aceita.
        let write = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "memory.write",
                    "arguments": {
                        "path": external.join(".betastartignore").to_string_lossy(),
                        "content": "node_modules/\n"
                    }
                }
            }),
        )
        .expect("write response");
        assert_eq!(write["result"]["isError"], false);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn batch_tasks_brief_and_compact_list() {
        let (root, mut ctx) = test_context();
        let external = root.join("Projeto");
        std::fs::create_dir_all(&external).expect("dir");
        std::fs::write(external.join("MEMORIA.md"), "# Memoria\n").expect("note");
        let client = test_client(vec!["memory", "tasks"]);

        let link = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "project.link",
                    "arguments": { "root": external.to_string_lossy(), "title": "Projeto" }
                }
            }),
        )
        .expect("link response");
        assert_eq!(link["result"]["isError"], false);

        let batch = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "tasks.create_many",
                    "arguments": {
                        "project": "projeto",
                        "tasks": [
                            { "title": "Planejar roteiro", "priority": "high" },
                            { "title": "Gerar narração", "due": "2027-01-10" },
                            { "title": "Renderizar vídeo" }
                        ]
                    }
                }
            }),
        )
        .expect("batch response");
        assert_eq!(batch["result"]["isError"], false);
        let batch_text = batch["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        let batch_json: Value = serde_json::from_str(batch_text).expect("batch json");
        assert_eq!(batch_json["created"], 3);
        assert_eq!(batch_json["tasks"].as_array().expect("tasks").len(), 3);
        assert!(batch_json["tasks"][0]["id"].is_string());

        let list = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "tasks.list",
                    "arguments": { "project": "projeto" }
                }
            }),
        )
        .expect("list response");
        let list_text = list["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        let list_json: Value = serde_json::from_str(list_text).expect("list json");
        let list_tasks = list_json.as_array().expect("array");
        assert_eq!(list_tasks.len(), 3);
        assert!(list_tasks[0]["id"].is_string());
        assert!(list_tasks[0].get("filePath").is_none());
        assert!(list_tasks[0].get("project").is_none());

        let brief = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {
                    "name": "project.brief",
                    "arguments": { "project": "projeto" }
                }
            }),
        )
        .expect("brief response");
        assert_eq!(brief["result"]["isError"], false);
        let brief_text = brief["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        let brief_json: Value = serde_json::from_str(brief_text).expect("brief json");
        assert_eq!(brief_json["project"]["slug"], "projeto");
        assert_eq!(brief_json["files"]["notes"], 2);
        assert_eq!(brief_json["tasks"]["total"], 3);
        assert_eq!(brief_json["tasks"]["byStatus"]["todo"], 3);
        let notes = brief_json["notes"].as_array().expect("notes");
        let memoria = notes
            .iter()
            .find(|note| note["name"] == "MEMORIA.md")
            .expect("memoria note");
        assert_eq!(memoria["path"], "MEMORIA.md");

        // Caminho relativo do brief funciona no memory.read.
        let read = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "tools/call",
                "params": {
                    "name": "memory.read",
                    "arguments": { "project": "projeto", "path": "MEMORIA.md" }
                }
            }),
        )
        .expect("read response");
        assert_eq!(read["result"]["isError"], false);
        assert!(read["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("Memoria"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn memory_list_is_paginated_and_reads_have_modes() {
        let (root, mut ctx) = test_context();
        let folder = root.join("workspace/global/inbox");
        std::fs::create_dir_all(&folder).expect("inbox");
        std::fs::write(
            folder.join("nota.md"),
            "# Titulo\n\n## Secao\n\ncorpo da nota\n",
        )
        .expect("note");
        ctx.reindex().expect("reindex");

        let client = test_client(vec!["memory"]);

        let list = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "memory.list",
                    "arguments": { "project": "global", "limit": 1 }
                }
            }),
        )
        .expect("list response");
        let body = list["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        let parsed: Value = serde_json::from_str(body).expect("list json");
        assert_eq!(parsed["returned"], 1);
        assert!(parsed["total"].as_u64().unwrap_or(0) >= 1);
        let path = parsed["files"][0]["path"].as_str().unwrap_or_default();
        assert!(path.ends_with("nota.md"));
        assert!(!path.contains('\\'), "caminho deve ser relativo: {path}");

        let outline = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "memory.read",
                    "arguments": { "project": "global", "path": "inbox/nota.md", "mode": "outline" }
                }
            }),
        )
        .expect("outline response");
        let outline_text = outline["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        let outline_json: Value = serde_json::from_str(outline_text).expect("outline json");
        assert_eq!(outline_json["headings"][1]["level"], 2);

        let range = handle(
            &mut ctx,
            &client,
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "memory.read",
                    "arguments": {
                        "project": "global",
                        "path": "inbox/nota.md",
                        "mode": "range",
                        "start": 5,
                        "end": 5
                    }
                }
            }),
        )
        .expect("range response");
        assert!(range["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("corpo da nota"));

        // Sem escopo `memory`, resources ficam vazios (antes vazava tudo).
        let anonymous = test_client(vec![]);
        let resources = handle(
            &mut ctx,
            &anonymous,
            json!({ "jsonrpc": "2.0", "id": 4, "method": "resources/list", "params": {} }),
        )
        .expect("resources response");
        assert_eq!(resources["result"]["resources"].as_array().unwrap().len(), 0);

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
