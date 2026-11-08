//! Definições e implementação das tools MCP.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::db;
use crate::mcp::McpContext;
use crate::memory::{self, Frontmatter};
use crate::models::{McpClient, MemoryFile, NoteDetail, ProjectInfo, RunResult, Task, TaskPatch};

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

fn text_result<T: serde::Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|error| error.to_string())
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

pub fn definitions() -> Vec<Value> {
    vec![
        tool(
            "memory.list",
            "List notes, tasks and projects in the workspace.",
            json!({
                "type": "object",
                "properties": { "project": { "type": "string", "description": "Project slug or 'global'" } }
            }),
        ),
        tool(
            "memory.read",
            "Read a Markdown memory file (raw content).",
            json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        ),
        tool(
            "memory.write",
            "Write raw Markdown to a memory file (creates parent folders).",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
        ),
        tool(
            "memory.search",
            "Search memory by keyword and optional tags.",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "project": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "limit": { "type": "number" }
                },
                "required": ["query"]
            }),
        ),
        tool(
            "project.list",
            "List projects.",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "project.create",
            "Create a project folder with project.md.",
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
            "graph.get",
            "Get the knowledge graph (nodes and edges).",
            json!({
                "type": "object",
                "properties": { "project": { "type": "string" } }
            }),
        ),
        tool(
            "tasks.list",
            "List tasks with optional filters.",
            json!({
                "type": "object",
                "properties": {
                    "project": { "type": "string" },
                    "status": { "type": "string" },
                    "query": { "type": "string" }
                }
            }),
        ),
        tool(
            "tasks.create",
            "Create a task (Markdown file with frontmatter).",
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
        "project.list" => {
            require_scope(client, "memory")?;
            project_list(ctx)
        }
        "project.create" => {
            require_scope(client, "memory")?;
            project_create(ctx, &args)
        }
        "graph.get" => {
            require_scope(client, "memory")?;
            graph_get(ctx, &args)
        }
        "tasks.list" => {
            require_scope(client, "tasks")?;
            tasks_list(ctx, &args)
        }
        "tasks.create" => {
            require_scope(client, "tasks")?;
            tasks_create(ctx, &args)
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
    let project = arg_str(args, "project");
    let files = db::memory::list_files(&ctx.db, project.as_deref()).map_err(|e| e.to_string())?;
    text_result(&files)
}

fn memory_read(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let path = arg_str(args, "path").ok_or_else(|| "path is required".to_string())?;
    let path_buf = PathBuf::from(&path);
    memory::ensure_inside(&ctx.workspace, &path_buf).map_err(|e| e.to_string())?;
    std::fs::read_to_string(&path_buf).map_err(|e| e.to_string())
}

fn memory_write(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let path = arg_str(args, "path").ok_or_else(|| "path is required".to_string())?;
    let content = arg_str(args, "content").ok_or_else(|| "content is required".to_string())?;
    let path_buf = PathBuf::from(&path);
    memory::ensure_inside(&ctx.workspace, &path_buf).map_err(|e| e.to_string())?;
    if let Some(parent) = path_buf.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path_buf, content).map_err(|e| e.to_string())?;
    ctx.reindex().map_err(|e| e.to_string())?;
    text_result(&json!({ "ok": true, "path": path }))
}

fn memory_search(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let query = arg_str(args, "query").unwrap_or_default();
    let project = arg_str(args, "project");
    let tags = arg_vec(args, "tags").unwrap_or_default();
    let limit = arg_i64(args, "limit").unwrap_or(20).max(1) as usize;
    let hits = db::memory::search(&ctx.db, project.as_deref(), &query, &tags, limit)
        .map_err(|e| e.to_string())?;
    text_result(&hits)
}

fn project_list(ctx: &McpContext) -> Result<String, String> {
    let projects_dir = ctx.workspace.join("projects");
    let mut out: Vec<ProjectInfo> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Ok((frontmatter, _)) = memory::read_note_file(&path.join("project.md")) else {
                continue;
            };
            let slug = frontmatter.slug.clone().or_else(|| {
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
            });
            out.push(ProjectInfo {
                slug: slug.unwrap_or_default(),
                title: frontmatter.title.unwrap_or_default(),
                description: frontmatter.description.unwrap_or_default(),
                color: frontmatter.color,
            });
        }
    }

    out.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()));
    text_result(&out)
}

fn project_create(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let title = arg_str(args, "title").ok_or_else(|| "title is required".to_string())?;
    let slug = memory::sanitize_slug(&arg_str(args, "slug").unwrap_or_else(|| title.clone()));
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
    })
}

fn graph_get(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let project = arg_str(args, "project");
    let graph =
        crate::graph::build_graph(&ctx.db, project.as_deref()).map_err(|e| e.to_string())?;
    text_result(&graph)
}

fn tasks_list(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let project = arg_str(args, "project");
    let status = arg_str(args, "status");
    let query = arg_str(args, "query");
    let tasks = db::tasks::list(
        &ctx.db,
        project.as_deref(),
        status.as_deref(),
        query.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    text_result(&tasks)
}

fn tasks_create(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let title = arg_str(args, "title").ok_or_else(|| "title is required".to_string())?;
    let project = arg_str(args, "project").filter(|value| !value.is_empty() && value != "all");

    let dir = match project.as_deref() {
        Some(slug) if slug != "global" && ctx.workspace.join("projects").join(slug).exists() => {
            ctx.workspace.join("projects").join(slug).join("tasks")
        }
        _ => ctx.workspace.join("global").join("tasks"),
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let base = memory::sanitize_slug(&title);
    let path = dir.join(format!("{base}-{}.md", &id[..8]));

    let now = memory::now_iso();
    let due = arg_str(args, "due").filter(|value| !value.trim().is_empty());
    let remind = arg_str(args, "remind")
        .filter(|value| !value.trim().is_empty())
        .or_else(|| due.clone());
    let body = arg_str(args, "body").unwrap_or_default();

    let frontmatter = Frontmatter {
        kind: Some("task".into()),
        id: Some(id.clone()),
        title: Some(title.trim().to_string()),
        tags: arg_vec(args, "tags").unwrap_or_default(),
        project: project.filter(|value| value != "global"),
        status: Some("todo".into()),
        priority: Some(
            arg_str(args, "priority")
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "medium".into()),
        ),
        due,
        remind,
        parent: arg_str(args, "parent").filter(|value| !value.is_empty()),
        created: Some(now.clone()),
        updated: Some(now),
        ..Default::default()
    };

    memory::write_note_file(&path, &frontmatter, &body).map_err(|e| e.to_string())?;
    ctx.reindex().map_err(|e| e.to_string())?;

    let task = db::tasks::get(&ctx.db, &id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "task not found after create".to_string())?;
    text_result(&task)
}

fn tasks_update(ctx: &McpContext, args: &Value) -> Result<String, String> {
    let id = arg_str(args, "id").ok_or_else(|| "id is required".to_string())?;
    let file_path = db::tasks::file_path_for_id(&ctx.db, &id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "task not found".to_string())?;
    let path = Path::new(&file_path).to_path_buf();
    memory::ensure_inside(&ctx.workspace, &path).map_err(|e| e.to_string())?;

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

    memory::write_note_file(&path, &frontmatter, &body).map_err(|e| e.to_string())?;
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
    memory::ensure_inside(&ctx.workspace, path).map_err(|e| e.to_string())?;
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
        return Err(outcome.error.unwrap_or_else(|| "extension failed".into()));
    }

    if let Some(toast) = outcome.toast {
        return text_result(&RunResult {
            kind: "message".into(),
            text: Some(toast),
        });
    }

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
                });
            }
            _ => {}
        }
    }

    if let Some(text) = result.as_str() {
        return text_result(&RunResult {
            kind: "message".into(),
            text: Some(text.to_string()),
        });
    }

    text_result(&RunResult {
        kind: "done".into(),
        text: None,
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
    crate::core::paths::ensure_workspace(&ctx.config.workspace.path).map_err(|e| e.to_string())?;

    if ctx.workspace != old_workspace {
        ctx.reindex().map_err(|e| e.to_string())?;
    }

    text_result(&ctx.config)
}

// Mantém os imports usados mesmo que algumas tools não usem todos os tipos.
#[allow(dead_code)]
fn _type_hints(_: MemoryFile, _: NoteDetail, _: Task, _: TaskPatch) {}
