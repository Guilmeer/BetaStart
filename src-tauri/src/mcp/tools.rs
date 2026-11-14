//! Definições e implementação das tools MCP.
//!
//! Regra de ouro: toda resposta é JSON compacto (sem indentação) e todo
//! retorno tem teto de tamanho. As tools de leitura ampla (`memory.list`,
//! `graph.get`) avisam o custo na descrição e apontam para a alternativa
//! escopada (`project.brief`, `graph.query`, `memory.context`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::db;
use crate::mcp::McpContext;
use crate::memory::{self, Frontmatter};
use crate::models::{
    GraphView, McpClient, MemoryFile, NoteDetail, ProjectInfo, RunResult, Task, TaskPatch,
    TaskSummary,
};

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

fn require_scope(client: &McpClient, scope: &str) -> Result<(), String> {
    if client.scopes.iter().any(|item| item == scope) {
        Ok(())
    } else {
        Err(format!("permission denied: scope '{scope}' not granted"))
    }
}

/// JSON compacto: indentação custava ~20% de tokens em respostas grandes.
fn text_result<T: serde::Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

fn arg_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn arg_vec(args: &Value, key: &str) -> Option<Vec<String>> {
    args.get(key)
        .and_then(|value| serde_json::from_value::<Vec<String>>(value.clone()).ok())
}

fn arg_bool(args: &Value, key: &str, default: bool) -> bool {
    args.get(key)
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

fn arg_present(args: &Value, key: &str) -> bool {
    args.get(key).is_some()
}

fn arg_f64(args: &Value, key: &str) -> Option<f64> {
    args.get(key).and_then(|value| value.as_f64())
}

fn arg_i64(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|value| value.as_i64())
}

fn arg_u64(args: &Value, key: &str) -> Option<u64> {
    args.get(key).and_then(|value| value.as_u64())
}

/// Projeto pedido, com `"all"`/vazio tratados como "todos".
fn scoped_project(args: &Value) -> Option<String> {
    arg_str(args, "project").filter(|value| !value.is_empty() && value != "all")
}

pub fn definitions() -> Vec<Value> {
    vec![
        tool(
            "memory.list",
            "List memory files (notes, tasks, project manifests) with relative paths. Paginated: default limit 200. Prefer project.brief for an overview; use this only when you need the file list.",
            json!({
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project slug or 'global'" },
                    "limit": { "type": "number", "description": "Default 200 (max 500)" },
                    "offset": { "type": "number", "description": "Default 0" },
                    "compact": { "type": "boolean", "description": "Default true (path/kind/title only)" }
                }
            }),
        ),
        tool(
            "memory.read",
            "Read a Markdown memory file from the workspace or a linked project. `mode:\"outline\"` returns headings + line numbers (cheap; do this before a full read), `mode:\"range\"` reads lines start..end, `mode:\"full\"` returns the raw file (capped at maxBytes).",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "project": { "type": "string", "description": "Required when `path` is relative" },
                    "mode": { "type": "string", "enum": ["full", "outline", "range"] },
                    "start": { "type": "number", "description": "range: first line (1-based)" },
                    "end": { "type": "number", "description": "range: last line" },
                    "maxBytes": { "type": "number", "description": "full: cap, default 24000" }
                },
                "required": ["path"]
            }),
        ),
        tool(
            "memory.write",
            "Write raw Markdown to a memory file in the workspace or a linked project (creates parent folders). Also accepts .betastartignore/.gitignore and files inside .betastart/. Relative paths are resolved against `project`.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "project": { "type": "string", "description": "Required when `path` is relative" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        ),
        tool(
            "memory.search",
            "Search memory by keyword and optional tags. Returns path, kind, title and a short snippet; read only the files you actually need.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "project": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "limit": { "type": "number", "description": "Default 20" }
                },
                "required": ["query"]
            }),
        ),
        tool(
            "memory.context",
            "One ranked bundle for a question: matching notes with snippets, open tasks, and related notes. Cheaper than memory.search + project.brief + graph.query in sequence.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "project": { "type": "string", "description": "Defaults to the active project" },
                    "limit": { "type": "number", "description": "Notes to return, default 8" },
                    "budget": { "type": "number", "description": "Approximate token cap, default 1500" }
                },
                "required": ["query"]
            }),
        ),
        tool(
            "project.list",
            "List managed projects and projects linked to existing user folders.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "project.create",
            "Create a managed project folder with project.md inside the workspace.",
            json!({
                "type": "object",
                "properties": {
                    "slug": { "type": "string" },
                    "title": { "type": "string" }
                },
                "required": ["title"]
            }),
        ),
        tool(
            "project.link",
            "Link an existing folder on disk as a Beta Start project, without copying or moving files. Indexes its Markdown in place and creates <root>/.betastart/ for the manifest. Optional `ignore` entries are appended to <root>/.betastartignore.",
            json!({
                "type": "object",
                "properties": {
                    "root": { "type": "string", "description": "Absolute path of an existing folder (supports ~/)" },
                    "title": { "type": "string", "description": "Display name; defaults to the folder name or the project.md title" },
                    "slug": { "type": "string", "description": "Project slug; defaults to the existing project.md slug or the title" },
                    "ignore": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Entries appended to .betastartignore (gitignore syntax), e.g. ['SkillAndPlugin/', 'examples/']"
                    }
                },
                "required": ["root"]
            }),
        ),
        tool(
            "project.unlink",
            "Unlink a linked project from Beta Start. No user files are deleted; only the link is removed.",
            json!({
                "type": "object",
                "properties": { "slug": { "type": "string" } },
                "required": ["slug"]
            }),
        ),
        tool(
            "project.brief",
            "START HERE for a project. Compact digest: metadata, notes ranked by importance (relative paths), task counts by status and the first open tasks. Costs ~300-600 tokens instead of project.list + memory.list + tasks.list.",
            json!({
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project slug; defaults to the active project" }
                }
            }),
        ),
        tool(
            "graph.get",
            "Returns the ENTIRE graph (every node and edge). Expensive: on a few hundred notes it costs tens of thousands of tokens. Use graph.query, graph.neighbors, graph.path or graph.explain for questions; use this only when the user explicitly asks for the whole graph.",
            json!({
                "type": "object",
                "properties": { "project": { "type": "string" } }
            }),
        ),
        tool(
            "graph.query",
            "Find nodes by term and return the subgraph around them, capped by a token budget. This is the default way to explore relationships between notes, tasks, tags and projects.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "project": { "type": "string" },
                    "depth": { "type": "number", "description": "Hops from the seeds, 1-3 (default 2)" },
                    "budget": { "type": "number", "description": "Approximate token cap, default 2000" }
                },
                "required": ["query"]
            }),
        ),
        tool(
            "graph.neighbors",
            "Subgraph around one node (by id, label, path or file name). Cheaper than graph.query when you already know the node.",
            json!({
                "type": "object",
                "properties": {
                    "node": { "type": "string" },
                    "project": { "type": "string" },
                    "hops": { "type": "number", "description": "Default 1 (max 3)" },
                    "budget": { "type": "number", "description": "Approximate token cap, default 1500" }
                },
                "required": ["node"]
            }),
        ),
        tool(
            "graph.path",
            "Shortest path between two nodes, hop by hop, with the edge kind of each hop (link, parent, tag, project).",
            json!({
                "type": "object",
                "properties": {
                    "from": { "type": "string" },
                    "to": { "type": "string" },
                    "project": { "type": "string" }
                },
                "required": ["from", "to"]
            }),
        ),
        tool(
            "graph.explain",
            "Explain one node: degree and every direct connection with direction and edge kind.",
            json!({
                "type": "object",
                "properties": {
                    "node": { "type": "string" },
                    "project": { "type": "string" },
                    "limit": { "type": "number", "description": "Max connections, default 40" }
                },
                "required": ["node"]
            }),
        ),
        tool(
            "tasks.list",
            "List tasks with optional filters. Compact by default (id, title, status, priority, due, tags); pass compact=false for full records with file paths.",
            json!({
                "type": "object",
                "properties": {
                    "project": { "type": "string" },
                    "status": { "type": "string" },
                    "query": { "type": "string" },
                    "compact": { "type": "boolean", "description": "Default true" }
                }
            }),
        ),
        tool(
            "tasks.create",
            "Create a task (Markdown file with frontmatter). Returns a compact task summary.",
            json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "project": { "type": "string" },
                    "parent": { "type": "string" },
                    "due": { "type": "string" },
                    "remind": { "type": "string" },
                    "priority": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "body": { "type": "string" }
                },
                "required": ["title"]
            }),
        ),
        tool(
            "tasks.create_many",
            "Create many tasks in one call (e.g. a development plan). Writes all files, reindexes once and returns compact summaries. Use `parent` with an existing task id for subtasks.",
            json!({
                "type": "object",
                "properties": {
                    "project": { "type": "string" },
                    "tasks": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "parent": { "type": "string" },
                                "due": { "type": "string" },
                                "remind": { "type": "string" },
                                "priority": { "type": "string" },
                                "tags": { "type": "array", "items": { "type": "string" } },
                                "body": { "type": "string" }
                            },
                            "required": ["title"]
                        }
                    }
                },
                "required": ["tasks"]
            }),
        ),
        tool(
            "tasks.update",
            "Update a task's frontmatter/body.",
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "title": { "type": "string" },
                    "status": { "type": "string" },
                    "priority": { "type": "string" },
                    "due": { "type": "string" },
                    "remind": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "parent": { "type": "string" },
                    "body": { "type": "string" }
                },
                "required": ["id"]
            }),
        ),
        tool(
            "tasks.delete",
            "Delete a task by id.",
            json!({
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"]
            }),
        ),
        tool(
            "windows.list",
            "List monitors and open windows.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "windows.move_resize",
            "Move/resize a window on a monitor using fractions (0..1) of the work area.",
            json!({
                "type": "object",
                "properties": {
                    "handle": { "type": "number" },
                    "monitor": { "type": "number" },
                    "x": { "type": "number" },
                    "y": { "type": "number" },
                    "w": { "type": "number" },
                    "h": { "type": "number" }
                },
                "required": ["handle", "monitor", "x", "y", "w", "h"]
            }),
        ),
        tool(
            "windows.apply_layout",
            "Apply a saved window layout.",
            json!({
                "type": "object",
                "properties": { "id": { "type": "number" } },
                "required": ["id"]
            }),
        ),
        tool(
            "windows.save_layout",
            "Save the current window arrangement as a layout.",
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "hotkey": { "type": "string" }
                },
                "required": ["name"]
            }),
        ),
        tool(
            "extensions.list",
            "List installed extensions.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "extensions.run",
            "Run an extension command in an isolated Deno process.",
            json!({
                "type": "object",
                "properties": {
                    "extId": { "type": "string" },
                    "commandId": { "type": "string" },
                    "args": { "type": "object" }
                },
                "required": ["extId", "commandId"]
            }),
        ),
        tool(
            "search.query",
            "Search apps, files, clipboard and extension commands.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "limit": { "type": "number" }
                },
                "required": ["query"]
            }),
        ),
        tool(
            "clipboard.list",
            "List clipboard history.",
            json!({
                "type": "object",
                "properties": { "limit": { "type": "number" } }
            }),
        ),
        tool(
            "settings.get",
            "Get the app configuration.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "settings.set",
            "Patch the app configuration.",
            json!({
                "type": "object",
                "properties": { "patch": { "type": "object" } },
                "required": ["patch"]
            }),
        ),
    ]
}

pub fn call(
    ctx: &mut McpContext,
    client: &McpClient,
    name: &str,
    args: Value,
) -> Result<String, String> {
    match name {
        "memory.list" => {
            require_scope(client, "memory")?;
            memory_list(ctx, &args)
        }
        "memory.read" => {
            require_scope(client, "memory")?;
            memory_read(ctx, &args)
        }
        "memory.write" => {
            require_scope(client, "memory")?;
            memory_write(ctx, &args)
        }
        "memory.search" => {
            require_scope(client, "memory")?;
            memory_search(ctx, &args)
        }
        "memory.context" => {
            require_scope(client, "memory")?;
            memory_context(ctx, &args)
        }
        "project.list" => {
            require_scope(client, "memory")?;
            project_list(ctx)
        }
        "project.create" => {
            require_scope(client, "memory")?;
            project_create(ctx, &args)
        }
        "project.link" => {
            require_scope(client, "memory")?;
            project_link(ctx, &args)
        }
        "project.unlink" => {
            require_scope(client, "memory")?;
            project_unlink(ctx, &args)
        }
        "project.brief" => {
            require_scope(client, "memory")?;
            project_brief(ctx, &args)
        }
        "graph.get" => {
            require_scope(client, "memory")?;
            graph_get(ctx, &args)
        }
        "graph.query" => {
            require_scope(client, "memory")?;
            graph_query(ctx, &args)
        }
        "graph.neighbors" => {
            require_scope(client, "memory")?;
            graph_neighbors(ctx, &args)
        }
        "graph.path" => {
            require_scope(client, "memory")?;
            graph_path(ctx, &args)
        }
        "graph.explain" => {
            require_scope(client, "memory")?;
            graph_explain(ctx, &args)
        }
        "tasks.list" => {
            require_scope(client, "tasks")?;
            tasks_list(ctx, &args)
        }
        "tasks.create" => {
            require_scope(client, "tasks")?;
            tasks_create(ctx, &args)
        }
        "tasks.create_many" => {
            require_scope(client, "tasks")?;
            tasks_create_many(ctx, &args)
        }
        "tasks.update" => {
            require_scope(client, "tasks")?;
            tasks_update(ctx, &args)
        }
        "tasks.delete" => {
            require_scope(client, "tasks")?;
            tasks_delete(ctx, &args)
        }
        "windows.list" => {
            require_scope(client, "windows")?;
            windows_list()
        }
        "windows.move_resize" => {
            require_scope(client, "windows")?;
            windows_move_resize(&args)
        }
        "windows.apply_layout" => {
            require_scope(client, "windows")?;
            windows_apply_layout(ctx, &args)
        }
        "windows.save_layout" => {
            require_scope(client, "windows")?;
            windows_save_layout(ctx, &args)
        }
        "extensions.list" => {
            require_scope(client, "extensions")?;
            extensions_list(ctx)
        }
        "extensions.run" => {
            require_scope(client, "extensions")?;
            extensions_run(ctx, &args)
        }
        "search.query" => {
            require_scope(client, "search")?;
            search_query(ctx, &args)
        }
        "clipboard.list" => {
            require_scope(client, "search")?;
            clipboard_list(ctx, &args)
        }
        "settings.get" => {
            require_scope(client, "settings")?;
            text_result(&ctx.config)
        }
        "settings.set" => {
            require_scope(client, "settings")?;
            settings_set(ctx, &args)
        }
        other => Err(format!("unknown tool: {other}")),
    }
}

fn memory_list(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let project = scoped_project(args);
    let limit = arg_i64(args, "limit").unwrap_or(200).clamp(1, 500) as usize;
    let offset = arg_i64(args, "offset").unwrap_or(0).max(0) as usize;
    let compact = arg_bool(args, "compact", true);

    let files = db::memory::list_files(&ctx.db, project.as_deref()).map_err(|e| e.to_string())?;
    let total = files.len();

    let items: Vec<Value> = files
        .iter()
        .skip(offset)
        .take(limit)
        .map(|file| {
            let path = relative_for(ctx, &file.project, &file.path);
            if compact {
                let mut value = json!({
                    "path": path,
                    "kind": file.kind,
                    "title": file.title,
                });
                if project.is_none() {
                    value["project"] = json!(file.project);
                }
                if !file.tags.is_empty() {
                    value["tags"] = json!(file.tags);
                }
                value
            } else {
                json!({
                    "path": path,
                    "name": file.name,
                    "kind": file.kind,
                    "title": file.title,
                    "project": file.project,
                    "tags": file.tags,
                })
            }
        })
        .collect();

    text_result(&json!({
        "total": total,
        "offset": offset,
        "returned": items.len(),
        "files": items,
    }))
}

/// Resolve `path` absoluto ou relativo ao projeto informado (ou projeto ativo).
fn resolve_memory_path(ctx: &McpContext, args: &Value) -> Result<PathBuf, String> {
    let raw = arg_str(args, "path").ok_or_else(|| "path is required".to_string())?;
    let path = PathBuf::from(&raw);
    if path.is_absolute() {
        return Ok(path);
    }

    let slug = scoped_project(args)
        .or_else(|| ctx.config.workspace.active_project.clone())
        .ok_or_else(|| {
            "relative path requires 'project' (or an active project); use project.list".to_string()
        })?;
    let dir = memory::project_dir(&ctx.workspace, &ctx.linked, &slug)
        .ok_or_else(|| format!("project '{slug}' not found"))?;
    Ok(dir.join(path))
}

/// Caminho relativo à raiz do projeto (economiza o caminho absoluto inteiro).
fn relative_for(ctx: &McpContext, project: &str, path: &str) -> String {
    match memory::project_dir(&ctx.workspace, &ctx.linked, project) {
        Some(root) => relative_path(Some(&root.to_string_lossy()), path),
        None => relative_path(None, path),
    }
}

fn memory_read(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let path_buf = resolve_memory_path(ctx, args)?;
    ensure_text_path(&path_buf)?;
    memory::ensure_allowed(&ctx.workspace, &ctx.linked, &path_buf).map_err(|e| e.to_string())?;

    let raw = std::fs::read_to_string(&path_buf).map_err(|e| e.to_string())?;
    let mode = arg_str(args, "mode")
        .unwrap_or_else(|| "full".into())
        .to_lowercase();
    let label = arg_str(args, "path").unwrap_or_default();

    match mode.as_str() {
        "outline" => {
            let lines: Vec<&str> = raw.lines().collect();
            let mut headings = Vec::new();
            for (index, line) in lines.iter().enumerate() {
                let trimmed = line.trim_start();
                if !trimmed.starts_with('#') {
                    continue;
                }
                let level = trimmed.chars().take_while(|value| *value == '#').count();
                if level > 6 {
                    continue;
                }
                let text = trimmed[level..].trim();
                if text.is_empty() {
                    continue;
                }
                headings.push(json!({ "line": index + 1, "level": level, "text": text }));
            }
            let preview = raw
                .split("\n\n")
                .find(|block| !block.trim().is_empty())
                .map(|block| block.chars().take(240).collect::<String>())
                .unwrap_or_default();
            Ok(text_result(&json!({
                "path": label,
                "mode": "outline",
                "lines": lines.len(),
                "chars": raw.chars().count(),
                "headings": headings,
                "preview": if headings.is_empty() { preview } else { String::new() },
                "hint": "use mode:\"range\" with a heading line to read one section",
            }))?)
        }
        "range" => {
            let lines: Vec<&str> = raw.lines().collect();
            let total = lines.len();
            let start = arg_i64(args, "start").unwrap_or(1).max(1) as usize;
            let end = arg_i64(args, "end")
                .unwrap_or((start + 200) as i64)
                .max(start as i64) as usize;
            let end = end.min(total.max(1));
            if start > total {
                return Err(format!("start ({start}) is past the end ({total} lines)"));
            }
            let text = lines[start - 1..end].join("\n");
            Ok(text_result(&json!({
                "path": label,
                "mode": "range",
                "start": start,
                "end": end,
                "totalLines": total,
                "text": text,
                "hasMore": end < total,
            }))?)
        }
        _ => {
            let max_bytes = arg_i64(args, "maxBytes")
                .unwrap_or(24_000)
                .clamp(500, 200_000) as usize;
            if raw.len() <= max_bytes {
                return Ok(raw);
            }
            let mut end = max_bytes;
            while end > 0 && !raw.is_char_boundary(end) {
                end -= 1;
            }
            Ok(format!(
                "{}\n\n[… truncated at {} of {} bytes; use mode:\"outline\" or mode:\"range\" for the rest]",
                &raw[..end],
                end,
                raw.len()
            ))
        }
    }
}

fn memory_write(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let content = arg_str(args, "content").ok_or_else(|| "content is required".to_string())?;
    let path_buf = resolve_memory_path(ctx, args)?;
    ensure_text_path(&path_buf)?;
    memory::ensure_allowed(&ctx.workspace, &ctx.linked, &path_buf).map_err(|e| e.to_string())?;
    if let Some(parent) = path_buf.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path_buf, content).map_err(|e| e.to_string())?;
    ctx.reindex().map_err(|e| e.to_string())?;
    text_result(&json!({ "ok": true, "path": path_buf.to_string_lossy() }))
}

/// Aceita Markdown, arquivos de ignore e qualquer arquivo dentro de `.betastart/`.
fn ensure_text_path(path: &Path) -> Result<(), String> {
    let is_markdown = path
        .extension()
        .map(|extension| extension.eq_ignore_ascii_case("md"))
        .unwrap_or(false);
    let is_ignore_file = path
        .file_name()
        .map(|name| {
            let name = name.to_string_lossy();
            name.eq_ignore_ascii_case(".betastartignore") || name.eq_ignore_ascii_case(".gitignore")
        })
        .unwrap_or(false);
    let in_betastart_dir = path
        .components()
        .any(|component| component.as_os_str().eq_ignore_ascii_case(".betastart"));

    if is_markdown || is_ignore_file || in_betastart_dir {
        Ok(())
    } else {
        Err(
            "memory tools accept Markdown (.md), .betastartignore/.gitignore and files inside .betastart/"
                .into(),
        )
    }
}

fn memory_search(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let query = arg_str(args, "query").unwrap_or_default();
    let project = scoped_project(args);
    let tags = arg_vec(args, "tags").unwrap_or_default();
    let limit = arg_i64(args, "limit").unwrap_or(20).max(1) as usize;
    let hits = db::memory::search(&ctx.db, project.as_deref(), &query, &tags, limit)
        .map_err(|e| e.to_string())?;

    let items: Vec<Value> = hits
        .iter()
        .map(|hit| {
            json!({
                "path": relative_for(ctx, &hit.project, &hit.file_path),
                "kind": hit.kind,
                "title": hit.title,
                "project": hit.project,
                "tags": hit.tags,
                "snippet": hit.snippet,
            })
        })
        .collect();
    text_result(&json!({ "total": items.len(), "hits": items }))
}

/// Pacote ranqueado único para uma pergunta (notas + tasks + vizinhança).
fn memory_context(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let query = arg_str(args, "query").unwrap_or_default();
    let project = scoped_project(args).or_else(|| ctx.config.workspace.active_project.clone());
    let limit = arg_i64(args, "limit").unwrap_or(8).clamp(1, 30) as usize;
    let budget = arg_i64(args, "budget").unwrap_or(1500).clamp(400, 8000) as usize;
    let limit_chars = budget * 4;

    let project_meta = match project.as_deref() {
        Some(slug) => memory::list_projects(&ctx.workspace, &ctx.linked)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|item| item.slug == slug)
            .map(|item| json!({ "slug": item.slug, "title": item.title, "description": item.description })),
        None => None,
    };

    let hits = if query.trim().is_empty() {
        Vec::new()
    } else {
        db::memory::search(&ctx.db, project.as_deref(), &query, &[], limit)
            .map_err(|e| e.to_string())?
    };

    let mut used = 0usize;
    let mut truncated = false;
    let mut notes: Vec<Value> = Vec::new();
    for hit in &hits {
        let snippet: String = hit.snippet.chars().take(160).collect();
        let value = json!({
            "path": relative_for(ctx, &hit.project, &hit.file_path),
            "kind": hit.kind,
            "title": hit.title,
            "snippet": snippet,
        });
        used += value.to_string().len();
        if used > limit_chars / 2 {
            truncated = true;
            break;
        }
        notes.push(value);
    }

    let mut tasks: Vec<Value> = Vec::new();
    if let Ok(list) = db::tasks::list(&ctx.db, project.as_deref(), None, Some(&query)) {
        for task in list.iter().filter(|task| task.status != "done").take(8) {
            let summary = TaskSummary::from_task(task);
            used += 60;
            if used > limit_chars {
                truncated = true;
                break;
            }
            tasks.push(serde_json::to_value(summary).unwrap_or(Value::Null));
        }
    }

    // Vizinhança da primeira nota: dá o "onde isso se conecta" sem despejar o grafo.
    let mut related: Vec<Value> = Vec::new();
    if let Some(first) = hits.first() {
        if let Ok(explain) =
            crate::graph::explain(&ctx.db, project.as_deref(), &first.file_path, 12)
        {
            for connection in explain.connections {
                used += 50;
                if used > limit_chars {
                    truncated = true;
                    break;
                }
                related.push(json!({
                    "label": connection.label,
                    "kind": connection.node_kind,
                    "relation": connection.kind,
                    "direction": connection.direction,
                    "path": connection.path.map(|path| {
                        relative_for(ctx, project.as_deref().unwrap_or("global"), &path)
                    }),
                }));
            }
        }
    }

    text_result(&json!({
        "project": project_meta,
        "query": query,
        "notes": notes,
        "tasks": tasks,
        "related": related,
        "truncated": truncated,
    }))
}

fn project_list(ctx: &McpContext) -> Result<String, String> {
    let projects =
        memory::list_projects(&ctx.workspace, &ctx.linked).map_err(|e| e.to_string())?;
    let items: Vec<Value> = projects
        .iter()
        .map(|project| {
            json!({
                "slug": project.slug,
                "title": project.title,
                "mode": project.mode,
                "available": project.available,
            })
        })
        .collect();
    text_result(&json!({ "projects": items }))
}

fn project_create(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let title = arg_str(args, "title").ok_or_else(|| "title is required".to_string())?;
    let slug = memory::sanitize_slug(&arg_str(args, "slug").unwrap_or_else(|| title.clone()));
    let existing =
        memory::list_projects(&ctx.workspace, &ctx.linked).map_err(|e| e.to_string())?;
    if existing
        .iter()
        .any(|project| project.slug.eq_ignore_ascii_case(&slug))
    {
        return Err("project already exists".into());
    }
    let dir = ctx.workspace.join("projects").join(&slug);
    let project_file = dir.join("project.md");
    if project_file.exists() {
        return Err("project already exists".into());
    }

    for sub in ["notes", "tasks", "assets"] {
        std::fs::create_dir_all(dir.join(sub)).map_err(|e| e.to_string())?;
    }

    let now = memory::now_iso();
    let frontmatter = Frontmatter {
        kind: Some("project".into()),
        slug: Some(slug.clone()),
        title: Some(title),
        description: Some(String::new()),
        color: Some("#8b5cf6".into()),
        created: Some(now.clone()),
        updated: Some(now),
        ..Default::default()
    };
    memory::write_note_file(&project_file, &frontmatter, "").map_err(|e| e.to_string())?;
    ctx.reindex().map_err(|e| e.to_string())?;

    text_result(&ProjectInfo {
        slug,
        title: frontmatter.title.unwrap_or_default(),
        description: String::new(),
        color: frontmatter.color,
        root_path: Some(dir.to_string_lossy().to_string()),
        mode: "managed".into(),
        available: true,
    })
}

fn project_link(ctx: &mut McpContext, args: &Value) -> Result<String, String> {
    let root = arg_str(args, "root").ok_or_else(|| "root is required".to_string())?;
    let title = arg_str(args, "title");
    let slug = arg_str(args, "slug");
    let ignore = arg_vec(args, "ignore").unwrap_or_default();

    let project = memory::link_project(
        &ctx.workspace,
        &mut ctx.config.workspace,
        &root,
        title,
        slug,
        &ignore,
    )
    .map_err(|e| e.to_string())?;

    ctx.save_config().map_err(|e| e.to_string())?;
    ctx.linked = memory::LinkedRoot::from_config(&ctx.config.workspace);
    ctx.reindex().map_err(|e| e.to_string())?;
    let indexed_files = db::memory::list_files(&ctx.db, Some(&project.slug))
        .map(|files| files.len())
        .unwrap_or(0);

    text_result(&json!({
        "project": project,
        "indexedFiles": indexed_files,
        "hint": "The Beta Start window refreshes automatically within ~2 seconds.",
    }))
}

fn project_unlink(ctx: &mut McpContext, args: &Value) -> Result<String, String> {
    let slug = arg_str(args, "slug").ok_or_else(|| "slug is required".to_string())?;

    memory::unlink_project(&mut ctx.config.workspace, &slug).map_err(|e| e.to_string())?;
    ctx.save_config().map_err(|e| e.to_string())?;
    ctx.linked = memory::LinkedRoot::from_config(&ctx.config.workspace);
    ctx.reindex().map_err(|e| e.to_string())?;

    text_result(&json!({ "ok": true, "slug": slug }))
}

fn relative_path(root: Option<&str>, path: &str) -> String {
    if let Some(root) = root {
        if let Ok(relative) = Path::new(path).strip_prefix(root) {
            return relative.to_string_lossy().replace('\\', "/");
        }
    }
    // Sem raiz conhecida, devolve o caminho inteiro: um nome de arquivo solto
    // aqui quebraria o `memory.read` (o caminho relativo precisa de projeto).
    path.replace('\\', "/")
}

/// Digest compacto de um projeto (~300-600 tokens): metadados, arquivos de
/// memória com caminho relativo e resumo de tasks. Evita encadear
/// `project.list` + `memory.list` + `tasks.list`.
///
/// As notas são rankeadas por grau aproximado (links + tags) e recência, não
/// pela ordem do índice: o que está truncado é o menos conectado.
fn project_brief(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let requested = scoped_project(args)
        .or_else(|| ctx.config.workspace.active_project.clone())
        .ok_or_else(|| {
            "project is required (no active project set); use project.list".to_string()
        })?;

    let projects = memory::list_projects(&ctx.workspace, &ctx.linked).map_err(|e| e.to_string())?;
    let project = projects
        .into_iter()
        .find(|item| item.slug == requested)
        .ok_or_else(|| format!("project '{requested}' not found"))?;

    let rows =
        db::memory::brief_rows(&ctx.db, Some(&project.slug)).map_err(|e| e.to_string())?;
    let root = project.root_path.as_deref();

    let mut notes: Vec<Value> = rows
        .iter()
        .filter(|row| row.kind != "task")
        .map(|row| {
            json!({
                "name": Path::new(&row.file_path)
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
                "title": row.title,
                "kind": row.kind,
                "path": relative_path(root, &row.file_path),
            })
        })
        .collect();
    // O manifesto do projeto vem primeiro; o resto já está por importância.
    notes.sort_by_key(|note| note["kind"].as_str() != Some("project"));
    let notes_total = notes.len();
    notes.truncate(25);

    let tasks =
        db::tasks::list(&ctx.db, Some(&project.slug), None, None).map_err(|e| e.to_string())?;
    let mut by_status: BTreeMap<String, usize> = BTreeMap::new();
    for task in &tasks {
        *by_status.entry(task.status.clone()).or_default() += 1;
    }
    let open: Vec<TaskSummary> = tasks
        .iter()
        .filter(|task| task.status != "done")
        .take(8)
        .map(|task| {
            let mut summary = TaskSummary::from_task(task);
            summary.project.clear();
            summary
        })
        .collect();
    let open_shown = open.len();

    text_result(&json!({
        "project": {
            "slug": project.slug,
            "title": project.title,
            "description": project.description,
            "mode": project.mode,
            "available": project.available,
        },
        "files": {
            "total": rows.len(),
            "notes": notes_total,
        },
        "notes": notes,
        "notesTruncated": notes_total > 25,
        "tasks": {
            "total": tasks.len(),
            "byStatus": by_status,
            "open": open,
            "openShown": open_shown,
        },
    }))
}

/// Converte um subgrafo em JSON com caminhos relativos (menos tokens).
fn graph_view_json(ctx: &McpContext, view: &GraphView, scoped: bool) -> Value {
    let nodes: Vec<Value> = view
        .nodes
        .iter()
        .map(|node| {
            let path = node.path.as_deref().map(|path| match node.project.as_deref() {
                Some(slug) => relative_for(ctx, slug, path),
                None => path.to_string(),
            });
            let mut value = json!({
                "id": node.id,
                "kind": node.kind,
                "label": node.label,
                "path": path,
            });
            if !scoped {
                value["project"] = json!(node.project);
            }
            value
        })
        .collect();

    json!({
        "nodes": nodes,
        "edges": view.edges,
        "seeds": view.seeds,
        "truncated": view.truncated,
        "totalNodes": view.total_nodes,
        "totalEdges": view.total_edges,
    })
}

fn graph_get(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let project = scoped_project(args);
    let graph = crate::graph::build_graph_cached(&ctx.db, project.as_deref())
        .map_err(|e| e.to_string())?;

    let nodes: Vec<Value> = graph
        .nodes
        .iter()
        .map(|node| {
            let path = node.path.as_deref().map(|path| match node.project.as_deref() {
                Some(slug) => relative_for(ctx, slug, path),
                None => path.to_string(),
            });
            let mut value = json!({
                "id": node.id,
                "kind": node.kind,
                "label": node.label,
                "path": path,
            });
            if project.is_none() {
                value["project"] = json!(node.project);
            }
            value
        })
        .collect();

    text_result(&json!({
        "nodes": nodes,
        "edges": graph.edges,
        "totalNodes": graph.nodes.len(),
        "totalEdges": graph.edges.len(),
        "hint": "graph.get is the full graph; prefer graph.query/neighbors/path for questions",
    }))
}

fn graph_query(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let query = arg_str(args, "query").ok_or_else(|| "query is required".to_string())?;
    let project = scoped_project(args);
    let depth = arg_i64(args, "depth").unwrap_or(2).clamp(1, 3) as usize;
    let budget = arg_i64(args, "budget").unwrap_or(2000).max(200) as usize;

    let view = crate::graph::query(&ctx.db, project.as_deref(), &query, depth, budget)
        .map_err(|e| e.to_string())?;

    if view.nodes.is_empty() {
        return text_result(&json!({
            "nodes": [],
            "edges": [],
            "seeds": [],
            "truncated": false,
            "hint": "no node matched; try a broader term or memory.search",
        }));
    }

    text_result(&graph_view_json(ctx, &view, project.is_some()))
}

fn graph_neighbors(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let node = arg_str(args, "node").ok_or_else(|| "node is required".to_string())?;
    let project = scoped_project(args);
    let hops = arg_i64(args, "hops").unwrap_or(1).clamp(1, 3) as usize;
    let budget = arg_i64(args, "budget").unwrap_or(1500).max(200) as usize;

    let view = crate::graph::neighbors(&ctx.db, project.as_deref(), &node, hops, budget)
        .map_err(|e| e.to_string())?;
    text_result(&graph_view_json(ctx, &view, project.is_some()))
}

fn graph_path(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let from = arg_str(args, "from").ok_or_else(|| "from is required".to_string())?;
    let to = arg_str(args, "to").ok_or_else(|| "to is required".to_string())?;
    let project = scoped_project(args);

    let path = crate::graph::find_path(&ctx.db, project.as_deref(), &from, &to)
        .map_err(|e| e.to_string())?;

    let nodes: Vec<Value> = path
        .nodes
        .iter()
        .map(|node| {
            let path = node.path.as_deref().map(|path| match node.project.as_deref() {
                Some(slug) => relative_for(ctx, slug, path),
                None => path.to_string(),
            });
            json!({ "id": node.id, "kind": node.kind, "label": node.label, "path": path })
        })
        .collect();

    text_result(&json!({
        "from": path.from,
        "to": path.to,
        "hops": path.hops,
        "nodes": nodes,
        "edges": path.edges,
    }))
}

fn graph_explain(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let node = arg_str(args, "node").ok_or_else(|| "node is required".to_string())?;
    let project = scoped_project(args);
    let limit = arg_i64(args, "limit").unwrap_or(40).clamp(1, 200) as usize;

    let explain = crate::graph::explain(&ctx.db, project.as_deref(), &node, limit)
        .map_err(|e| e.to_string())?;

    let connections: Vec<Value> = explain
        .connections
        .iter()
        .map(|connection| {
            let path = connection.path.as_deref().map(|path| {
                relative_for(
                    ctx,
                    project.as_deref().unwrap_or("global"),
                    path,
                )
            });
            json!({
                "direction": connection.direction,
                "kind": connection.kind,
                "id": connection.id,
                "label": connection.label,
                "nodeKind": connection.node_kind,
                "path": path,
            })
        })
        .collect();

    text_result(&json!({
        "node": {
            "id": explain.node.id,
            "kind": explain.node.kind,
            "label": explain.node.label,
            "path": explain.node.path.as_deref().map(|path| {
                relative_for(ctx, project.as_deref().unwrap_or("global"), path)
            }),
        },
        "degree": explain.degree,
        "connections": connections,
        "truncated": explain.degree > connections.len(),
    }))
}

fn tasks_list(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let project = scoped_project(args);
    let status = arg_str(args, "status").filter(|value| !value.is_empty());
    let query = arg_str(args, "query").filter(|value| !value.is_empty());
    let compact = arg_bool(args, "compact", true);

    let tasks =
        db::tasks::list(&ctx.db, project.as_deref(), status.as_deref(), query.as_deref())
            .map_err(|e| e.to_string())?;

    if !compact {
        return text_result(&tasks);
    }

    let scoped = project.is_some();
    let summaries: Vec<TaskSummary> = tasks
        .iter()
        .map(|task| {
            let mut summary = TaskSummary::from_task(task);
            if scoped {
                summary.project.clear();
            }
            summary
        })
        .collect();
    text_result(&summaries)
}

struct NewTaskSpec {
    title: String,
    parent: Option<String>,
    due: Option<String>,
    remind: Option<String>,
    priority: Option<String>,
    tags: Vec<String>,
    body: String,
}

fn task_spec(value: &Value) -> Result<NewTaskSpec, String> {
    let title = arg_str(value, "title")
        .unwrap_or_default()
        .trim()
        .to_string();
    if title.is_empty() {
        return Err("title is required".to_string());
    }
    let due = arg_str(value, "due").filter(|item| !item.trim().is_empty());
    let remind = arg_str(value, "remind")
        .filter(|item| !item.trim().is_empty())
        .or_else(|| due.clone());

    Ok(NewTaskSpec {
        title,
        parent: arg_str(value, "parent").filter(|item| !item.trim().is_empty()),
        due,
        remind,
        priority: arg_str(value, "priority").filter(|item| !item.trim().is_empty()),
        tags: arg_vec(value, "tags").unwrap_or_default(),
        body: arg_str(value, "body").unwrap_or_default(),
    })
}

fn write_task_file(
    ctx: &McpContext,
    project: Option<&str>,
    spec: &NewTaskSpec,
) -> Result<String, String> {
    let dir = match project {
        Some(slug) => memory::tasks_dir(&ctx.workspace, &ctx.linked, slug)
            .ok_or_else(|| format!("project '{slug}' not found"))?,
        None => ctx.workspace.join("global").join("tasks"),
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let base = memory::sanitize_slug(&spec.title);
    let path = dir.join(format!("{base}-{}.md", &id[..8]));

    let now = memory::now_iso();
    let frontmatter = Frontmatter {
        kind: Some("task".into()),
        id: Some(id.clone()),
        title: Some(spec.title.clone()),
        tags: spec.tags.clone(),
        project: project.filter(|value| *value != "global").map(str::to_string),
        status: Some("todo".into()),
        priority: Some(
            spec.priority
                .clone()
                .unwrap_or_else(|| "medium".into()),
        ),
        due: spec.due.clone(),
        remind: spec.remind.clone(),
        parent: spec.parent.clone(),
        created: Some(now.clone()),
        updated: Some(now),
        ..Default::default()
    };

    memory::write_note_file(&path, &frontmatter, &spec.body).map_err(|e| e.to_string())?;
    Ok(id)
}

fn tasks_create(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let project = scoped_project(args);
    let spec = task_spec(args)?;
    let id = write_task_file(ctx, project.as_deref(), &spec)?;
    ctx.reindex().map_err(|e| e.to_string())?;

    let task = db::tasks::get(&ctx.db, &id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "task not found after create".to_string())?;
    text_result(&TaskSummary::from_task(&task))
}

fn tasks_create_many(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let project = scoped_project(args);
    let items = args
        .get("tasks")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "tasks is required".to_string())?;
    if items.len() > 100 {
        return Err("at most 100 tasks per call".to_string());
    }
    if items.is_empty() {
        return text_result(&json!({ "created": 0, "tasks": [] }));
    }

    let mut ids = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let spec = task_spec(item).map_err(|error| format!("tasks[{index}]: {error}"))?;
        ids.push(write_task_file(ctx, project.as_deref(), &spec)?);
    }
    ctx.reindex().map_err(|e| e.to_string())?;

    let scoped = project.is_some();
    let mut summaries = Vec::with_capacity(ids.len());
    for id in &ids {
        let task = db::tasks::get(&ctx.db, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "task not found after create".to_string())?;
        let mut summary = TaskSummary::from_task(&task);
        if scoped {
            summary.project.clear();
        }
        summaries.push(summary);
    }

    text_result(&json!({ "created": summaries.len(), "tasks": summaries }))
}

fn tasks_update(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let id = arg_str(args, "id").ok_or_else(|| "id is required".to_string())?;
    let file_path = db::tasks::file_path_for_id(&ctx.db, &id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "task not found".to_string())?;
    let path = Path::new(&file_path).to_path_buf();
    memory::ensure_allowed(&ctx.workspace, &ctx.linked, &path).map_err(|e| e.to_string())?;

    let (mut frontmatter, mut body) = memory::read_note_file(&path).map_err(|e| e.to_string())?;

    if let Some(title) = arg_str(args, "title") {
        if !title.trim().is_empty() {
            frontmatter.title = Some(title.trim().to_string());
        }
    }
    if let Some(status) = arg_str(args, "status") {
        frontmatter.status = Some(status);
    }
    if let Some(priority) = arg_str(args, "priority") {
        frontmatter.priority = Some(priority);
    }
    if arg_present(args, "due") {
        frontmatter.due = arg_str(args, "due").filter(|value| !value.trim().is_empty());
    }
    if arg_present(args, "remind") {
        frontmatter.remind = arg_str(args, "remind").filter(|value| !value.trim().is_empty());
    }
    if let Some(tags) = arg_vec(args, "tags") {
        frontmatter.tags = tags;
    }
    if arg_present(args, "parent") {
        frontmatter.parent = arg_str(args, "parent").filter(|value| !value.trim().is_empty());
    }
    if let Some(new_body) = arg_str(args, "body") {
        body = new_body;
    }
    frontmatter.updated = Some(memory::now_iso());

    memory::write_note_file_preserving(&path, &frontmatter, &body).map_err(|e| e.to_string())?;
    ctx.reindex().map_err(|e| e.to_string())?;

    let task = db::tasks::get(&ctx.db, &id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "task not found".to_string())?;
    text_result(&task)
}

fn tasks_delete(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let id = arg_str(args, "id").ok_or_else(|| "id is required".to_string())?;
    let file_path = db::tasks::file_path_for_id(&ctx.db, &id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "task not found".to_string())?;
    let path = Path::new(&file_path);
    memory::ensure_allowed(&ctx.workspace, &ctx.linked, path).map_err(|e| e.to_string())?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    ctx.reindex().map_err(|e| e.to_string())?;
    text_result(&json!({ "ok": true, "id": id }))
}

fn windows_list() -> Result<String, String> {
    let monitors = crate::window_manager::list_monitors();
    let windows = crate::window_manager::snapshot_windows(&monitors);
    text_result(&json!({ "monitors": monitors, "windows": windows }))
}

fn windows_move_resize(args: &Value) -> Result<String, String> {
    let handle = arg_u64(args, "handle").ok_or_else(|| "handle is required".to_string())?;
    let monitor = arg_i64(args, "monitor").unwrap_or(0) as i32;
    let x = arg_f64(args, "x").ok_or_else(|| "x is required".to_string())?;
    let y = arg_f64(args, "y").ok_or_else(|| "y is required".to_string())?;
    let w = arg_f64(args, "w").ok_or_else(|| "w is required".to_string())?;
    let h = arg_f64(args, "h").ok_or_else(|| "h is required".to_string())?;
    crate::window_manager::move_window(handle, monitor, x, y, w, h)?;
    text_result(&json!({ "ok": true }))
}

fn windows_apply_layout(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let id = arg_i64(args, "id").ok_or_else(|| "id is required".to_string())?;
    let result = crate::window_manager::apply_layout_by_id(&ctx.db, id)?;
    text_result(&result)
}

fn windows_save_layout(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let name = arg_str(args, "name").ok_or_else(|| "name is required".to_string())?;
    let hotkey = arg_str(args, "hotkey").filter(|value| !value.trim().is_empty());
    let monitors = crate::window_manager::list_monitors();
    let slots = crate::window_manager::capture_layout(&monitors);
    let id = db::windows::save_layout(&ctx.db, &name, hotkey.as_deref(), &monitors, &slots)
        .map_err(|e| e.to_string())?;
    let layout = db::windows::get_layout(&ctx.db, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "layout not found".to_string())?;
    text_result(&layout)
}

fn extensions_list(ctx: &McpContext) -> Result<String, String> {
    let extensions = db::extensions::list(&ctx.db).map_err(|e| e.to_string())?;
    text_result(&extensions)
}

fn extensions_run(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let ext_id = arg_str(args, "extId").ok_or_else(|| "extId is required".to_string())?;
    let command_id =
        arg_str(args, "commandId").ok_or_else(|| "commandId is required".to_string())?;
    let run_args = args.get("args").cloned().unwrap_or(Value::Null);

    let row = db::extensions::get_row(&ctx.db, &ext_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "extension not installed".to_string())?;
    if !row.enabled {
        return Err("extension disabled".into());
    }
    if row.pending {
        return Err("extension permissions not granted".into());
    }
    if !row.manifest.commands.iter().any(|c| c.id == command_id) {
        return Err("command not found".into());
    }

    let deno = crate::extensions::runtime::find_deno(ctx.config.extensions.deno_path.as_deref())
        .ok_or_else(|| "Deno runtime not found".to_string())?;
    let outcome = crate::extensions::runtime::run_command(
        &deno,
        &crate::core::paths::runtime_dir(),
        Path::new(&row.install_path),
        &row.manifest,
        &command_id,
        run_args,
        &row.granted,
        Some(&ctx.workspace),
    )
    .map_err(|e| e.to_string())?;

    if !outcome.ok {
        let hint = outcome
            .log_path
            .as_deref()
            .map(|path| format!("\nLog: {path}"))
            .unwrap_or_default();
        return Err(format!(
            "{}{hint}",
            outcome.error.unwrap_or_else(|| "extension failed".into())
        ));
    }

    // `ui.toast` acompanha a ação; não substitui `copy`/`open`.
    let toast = outcome.toast;
    let result = outcome.result;
    if let Some(object) = result.as_object() {
        match object.get("type").and_then(|value| value.as_str()) {
            Some("copy") => {
                if !row.granted.iter().any(|p| p == "clipboard:write") {
                    return Err("permission 'clipboard:write' not granted".into());
                }
                let text = object
                    .get("text")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();
                let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
                clipboard
                    .set_text(text.clone())
                    .map_err(|e| e.to_string())?;
                return text_result(&RunResult {
                    kind: "copy".into(),
                    text: Some(text),
                    toast,
                });
            }
            Some("open") => {
                if !row.granted.iter().any(|p| p == "shell") {
                    return Err("permission 'shell' not granted".into());
                }
                let path = object
                    .get("path")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                crate::platform::launch(path).map_err(|e| e.to_string())?;
                return text_result(&RunResult {
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
                return text_result(&RunResult {
                    kind: "message".into(),
                    text: Some(text),
                    toast: None,
                });
            }
            _ => {}
        }
    }

    if let Some(text) = result.as_str() {
        return text_result(&RunResult {
            kind: "message".into(),
            text: Some(text.to_string()),
            toast: None,
        });
    }

    if let Some(toast) = toast {
        return text_result(&RunResult {
            kind: "message".into(),
            text: Some(toast),
            toast: None,
        });
    }

    text_result(&RunResult {
        kind: "done".into(),
        text: None,
        toast: None,
    })
}

fn search_query(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let query = arg_str(args, "query").unwrap_or_default();
    let limit = arg_i64(args, "limit").unwrap_or(10).clamp(1, 50) as usize;

    let apps = db::apps::search(&ctx.db, &query, limit).map_err(|e| e.to_string())?;
    let files = db::files::search(&ctx.db, &query, limit).map_err(|e| e.to_string())?;
    let clipboard =
        db::clipboard::search(&ctx.db, &query, limit.min(10)).map_err(|e| e.to_string())?;
    let commands = db::extensions::commands(&ctx.db, &query, limit).map_err(|e| e.to_string())?;

    text_result(&json!({
        "apps": apps,
        "files": files,
        "clipboard": clipboard,
        "commands": commands
    }))
}

fn clipboard_list(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let limit = arg_i64(args, "limit").unwrap_or(50).clamp(1, 200) as usize;
    let items = db::clipboard::recent(&ctx.db, limit).map_err(|e| e.to_string())?;
    text_result(&items)
}

fn settings_set(ctx: &mut McpContext, args: &Value) -> Result<String, String> {
    let patch = args
        .get("patch")
        .cloned()
        .ok_or_else(|| "patch is required".to_string())?;
    let old_workspace = ctx.workspace.clone();

    ctx.config = ctx.config.merged(patch).map_err(|e| e.to_string())?;
    ctx.save_config().map_err(|e| e.to_string())?;
    ctx.workspace = crate::core::paths::expand(&ctx.config.workspace.path);
    ctx.linked = crate::memory::LinkedRoot::from_config(&ctx.config.workspace);
    crate::core::paths::ensure_workspace(&ctx.config.workspace.path).map_err(|e| e.to_string())?;

    if ctx.workspace != old_workspace {
        ctx.reindex().map_err(|e| e.to_string())?;
    }

    text_result(&ctx.config)
}

// Mantém os imports usados mesmo que algumas tools não usem todos os tipos.
#[allow(dead_code)]
fn _type_hints(_: MemoryFile, _: NoteDetail, _: Task, _: TaskPatch) {}
