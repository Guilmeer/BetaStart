//! Comandos Tauri do ambiente de memória + tasks.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, State};

use crate::core::paths;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::memory::{self, Frontmatter, LinkedRoot};
use crate::models::{
    Graph, LinkInfo, MemoryFile, MemoryHit, NewTask, NoteDetail, ProjectInfo, Task, TaskPatch,
};
use crate::state::AppState;

fn config_scope(state: &AppState) -> AppResult<(PathBuf, Vec<LinkedRoot>)> {
    let config = state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))?;
    Ok((
        paths::expand(&config.workspace.path),
        LinkedRoot::from_config(&config.workspace),
    ))
}

fn reindex_with_state(state: &AppState) -> AppResult<usize> {
    let (workspace, linked) = config_scope(state)?;
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    memory::reindex_with_linked(&conn, &workspace, &linked)
}

/// Reindexação numa conexão própria, para rodar fora da thread da interface.
fn reindex_with_db(db_path: &Path, workspace: &Path, linked: &[LinkedRoot]) -> AppResult<usize> {
    let conn = db::open(db_path)?;
    let count = memory::reindex_with_linked(&conn, workspace, linked)?;
    db::checkpoint(&conn);
    Ok(count)
}

fn ensure_markdown_path(path: &Path) -> AppResult<()> {
    let is_markdown = path
        .extension()
        .map(|extension| extension.eq_ignore_ascii_case("md"))
        .unwrap_or(false);
    if is_markdown {
        Ok(())
    } else {
        Err(AppError::Platform(
            "apenas arquivos Markdown (.md) são acessíveis pela memória".into(),
        ))
    }
}

/// Resolve rótulos `[[...]]` para o arquivo correspondente (título ou stem).
/// Prioriza arquivos do mesmo projeto e cai para o índice global.
fn resolve_links(
    state: &AppState,
    project: Option<&str>,
    links: &[String],
) -> AppResult<Vec<LinkInfo>> {
    if links.is_empty() {
        return Ok(Vec::new());
    }

    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    let mut stmt = conn.prepare("SELECT file_path, title, project FROM memory_index")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    let mut scoped: HashMap<String, String> = HashMap::new();
    let mut global: HashMap<String, String> = HashMap::new();
    for row in rows {
        let (path, title, row_project) = row?;
        let title_key = title.trim().to_lowercase();
        let stem_key = Path::new(&path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_lowercase())
            .unwrap_or_else(|| title_key.clone());

        global.entry(title_key.clone()).or_insert(path.clone());
        global.entry(stem_key.clone()).or_insert(path.clone());
        if project.is_some_and(|value| value == row_project) {
            scoped.entry(title_key).or_insert(path.clone());
            scoped.entry(stem_key).or_insert(path);
        }
    }

    Ok(links
        .iter()
        .map(|label| {
            let key = label.trim().to_lowercase();
            LinkInfo {
                label: label.clone(),
                path: scoped.get(&key).or_else(|| global.get(&key)).cloned(),
            }
        })
        .collect())
}

#[tauri::command]
pub async fn reindex_memory(app: AppHandle, state: State<'_, AppState>) -> AppResult<usize> {
    let (workspace, linked) = config_scope(state.inner())?;
    let db_path = paths::db_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || reindex_with_db(&db_path, &workspace, &linked))
        .await
        .map_err(|e| AppError::Platform(format!("falha ao reindexar a memória: {e}")))?
}

/// Marcadores do bloco que o Beta Start mantém dentro do `AGENTS.md`.
const GUIDANCE_BEGIN: &str = "<!-- betastart:begin -->";
const GUIDANCE_END: &str = "<!-- betastart:end -->";

fn strip_guidance_block(content: &str) -> String {
    let Some(start) = content.find(GUIDANCE_BEGIN) else {
        return content.trim().to_string();
    };
    let Some(offset) = content[start..].find(GUIDANCE_END) else {
        return content.trim().to_string();
    };
    let end = start + offset + GUIDANCE_END.len();
    let mut out = String::new();
    out.push_str(content[..start].trim_end());
    let tail = content[end..].trim_start();
    if !tail.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(tail);
    }
    out.trim().to_string()
}

/// Bloco de instruções que o Beta Start mantém dentro do `AGENTS.md`.
fn guidance_block(slug: &str) -> String {
    format!(
        "{GUIDANCE_BEGIN}\n\
## Beta Start memory (MCP)\n\
\n\
This workspace is indexed by Beta Start on this machine and exposed through the\n\
`betastart` MCP server. Query it instead of re-reading files:\n\
\n\
- `project.brief {{ \"project\": \"{slug}\" }}` — start here: notes, task counts, open tasks.\n\
- `memory.search {{ \"project\": \"{slug}\", \"query\": \"...\" }}` — find notes by keyword.\n\
- `memory.read {{ \"project\": \"{slug}\", \"path\": \"...\", \"mode\": \"outline\" }}` — headings first, then `mode:\"range\"`.\n\
- `memory.context {{ \"project\": \"{slug}\", \"query\": \"...\" }}` — one ranked bundle (notes + tasks + related).\n\
- `graph.query` / `graph.neighbors` / `graph.path` — relationships between notes and tasks.\n\
- `tasks.list` / `tasks.create_many` — task state and batch creation.\n\
\n\
Avoid `graph.get` (the whole graph) and `memory.list` without `limit` unless you\n\
really need everything.\n\
{GUIDANCE_END}"
    )
}

/// Escreve o bloco no `AGENTS.md`, preservando o conteúdo que já existia.
fn write_guidance(root: &Path, slug: &str) -> AppResult<PathBuf> {
    let path = root.join("AGENTS.md");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let cleaned = strip_guidance_block(&existing);
    let block = guidance_block(slug);
    let output = if cleaned.is_empty() {
        format!("{block}\n")
    } else {
        format!("{cleaned}\n\n{block}\n")
    };
    std::fs::write(&path, output)?;
    Ok(path)
}

/// Escreve (ou atualiza) o bloco de instruções do Beta Start no `AGENTS.md`
/// da raiz do projeto, para que o assistente prefira o grafo à força bruta.
#[tauri::command]
pub fn install_ai_guidance(
    state: State<AppState>,
    project: Option<String>,
) -> AppResult<String> {
    let (workspace, linked) = config_scope(state.inner())?;
    let project = project.filter(|value| !value.is_empty() && value != "all").or_else(|| {
        state
            .config
            .lock()
            .ok()
            .and_then(|config| config.workspace.active_project.clone())
    });

    let (slug, root) = match project {
        Some(slug) => {
            let dir = memory::project_dir(&workspace, &linked, &slug)
                .ok_or_else(|| AppError::NotFound(format!("projeto '{slug}' não encontrado")))?;
            (slug, dir)
        }
        None => ("global".to_string(), workspace.clone()),
    };

    let path = write_guidance(&root, &slug)?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> AppResult<Vec<ProjectInfo>> {
    let (workspace, linked) = config_scope(state.inner())?;
    tauri::async_runtime::spawn_blocking(move || memory::list_projects(&workspace, &linked))
        .await
        .map_err(|e| AppError::Platform(format!("falha ao listar projetos: {e}")))?
}

#[tauri::command]
pub fn create_project(
    state: State<AppState>,
    slug: String,
    title: String,
) -> AppResult<ProjectInfo> {
    let (workspace, linked) = config_scope(state.inner())?;
    let slug = memory::sanitize_slug(if slug.trim().is_empty() {
        &title
    } else {
        &slug
    });
    if memory::list_projects(&workspace, &linked)?
        .iter()
        .any(|project| project.slug.eq_ignore_ascii_case(&slug))
    {
        return Err(AppError::Config(format!("projeto '{slug}' já existe")));
    }
    let dir = workspace.join("projects").join(&slug);
    let project_file = dir.join("project.md");
    if project_file.exists() {
        return Err(AppError::Config(format!("projeto '{slug}' já existe")));
    }

    for sub in ["notes", "tasks", "assets"] {
        std::fs::create_dir_all(dir.join(sub))?;
    }

    let title = if title.trim().is_empty() {
        slug.clone()
    } else {
        title.trim().to_string()
    };
    let now = memory::now_iso();
    let frontmatter = Frontmatter {
        kind: Some("project".into()),
        slug: Some(slug.clone()),
        title: Some(title.clone()),
        description: Some(String::new()),
        color: Some("#8b5cf6".into()),
        created: Some(now.clone()),
        updated: Some(now),
        ..Default::default()
    };
    memory::write_note_file(&project_file, &frontmatter, "")?;
    reindex_with_state(state.inner())?;

    Ok(ProjectInfo {
        slug,
        title,
        description: String::new(),
        color: Some("#8b5cf6".into()),
        root_path: Some(dir.to_string_lossy().to_string()),
        mode: "managed".into(),
        available: true,
    })
}

/// Vincula uma pasta existente como projeto do Beta Start.
///
/// Os arquivos Markdown da pasta são indexados no lugar e podem ser lidos/editados.
/// O app só cria/gerencia `<root>/.betastart/` para manifesto e tasks/notas próprias.
#[tauri::command]
pub fn link_project(
    app: AppHandle,
    state: State<AppState>,
    root: String,
    title: Option<String>,
) -> AppResult<ProjectInfo> {
    let config_path = paths::config_path(&app)?;
    let project = {
        let mut config = state
            .config
            .lock()
            .map_err(|_| AppError::Config("config lock envenenado".into()))?;
        let workspace = paths::expand(&config.workspace.path);
        let project = memory::link_project(
            &workspace,
            &mut config.workspace,
            &root,
            title,
            None,
            &[],
        )?;
        config.save(&config_path)?;
        project
    };

    reindex_with_state(state.inner())?;
    Ok(project)
}

/// Remove o vínculo com uma pasta. Nenhum arquivo do usuário é apagado.
#[tauri::command]
pub fn unlink_project(
    app: AppHandle,
    state: State<AppState>,
    slug: String,
) -> AppResult<()> {
    let config_path = paths::config_path(&app)?;
    {
        let mut config = state
            .config
            .lock()
            .map_err(|_| AppError::Config("config lock envenenado".into()))?;
        memory::unlink_project(&mut config.workspace, &slug)?;
        config.save(&config_path)?;
    }
    reindex_with_state(state.inner())?;
    Ok(())
}

#[tauri::command]
pub fn list_memory_files(
    state: State<AppState>,
    project: Option<String>,
) -> AppResult<Vec<MemoryFile>> {
    let project = project.filter(|value| value != "all");
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::memory::list_files(&conn, project.as_deref())
}

/// Notas/tasks que citam a nota atual via `[[link]]`.
#[tauri::command]
pub fn get_backlinks(state: State<AppState>, path: String) -> AppResult<Vec<MemoryFile>> {
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;

    let target_title: Option<String> = conn
        .query_row(
            "SELECT title FROM memory_index WHERE file_path = ?1",
            rusqlite::params![path],
            |row| row.get(0),
        )
        .ok();
    let target_title = target_title
        .map(|title| title.trim().to_lowercase())
        .filter(|title| !title.is_empty());
    let stem = Path::new(&path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().trim().to_lowercase())
        .filter(|stem| !stem.is_empty());

    let mut stmt = conn.prepare(
        "SELECT file_path, kind, title, project, tags, COALESCE(links, '[]')
           FROM memory_index
          WHERE file_path != ?1 AND links IS NOT NULL",
    )?;
    let rows = stmt.query_map(rusqlite::params![path], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;

    let mut out = Vec::new();
    for row in rows {
        let (file_path, kind, title, project, tags, links) = row?;
        let links: Vec<String> = serde_json::from_str(&links).unwrap_or_default();
        let matches = links.iter().any(|label| {
            let label = label.trim().to_lowercase();
            !label.is_empty()
                && (target_title.as_deref() == Some(label.as_str())
                    || stem.as_deref() == Some(label.as_str()))
        });
        if !matches {
            continue;
        }
        out.push(MemoryFile {
            name: Path::new(&file_path)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
            path: file_path,
            kind,
            title,
            project,
            tags: serde_json::from_str(&tags).unwrap_or_default(),
        });
    }
    out.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub fn read_note(state: State<AppState>, path: String) -> AppResult<NoteDetail> {
    let (workspace, linked) = config_scope(state.inner())?;
    let path_buf = PathBuf::from(&path);
    memory::ensure_allowed(&workspace, &linked, &path_buf)?;
    ensure_markdown_path(&path_buf)?;

    let parsed = memory::read_note_file_detailed(&path_buf)?;
    let frontmatter = parsed.frontmatter;
    let body = parsed.body;
    let kind = memory::note_kind(&frontmatter, &path_buf);
    let title = frontmatter.title.clone().unwrap_or_else(|| {
        path_buf
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default()
    });
    let path_project = memory::project_for_path(&workspace, &linked, &path_buf);
    let is_linked = path_project
        .as_ref()
        .is_some_and(|slug| linked.iter().any(|item| &item.slug == slug));
    let project = if is_linked {
        path_project
    } else {
        frontmatter.project.clone().or(path_project)
    };
    let id = frontmatter.id.clone().or_else(|| {
        if kind == "task" {
            Some(format!(
                "task-{}",
                path_buf
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
                    .unwrap_or_default()
            ))
        } else {
            None
        }
    });

    let mut link_labels = frontmatter.links.clone();
    link_labels.extend(memory::extract_wiki_links(&body));
    link_labels.sort();
    link_labels.dedup();
    let links = resolve_links(state.inner(), project.as_deref(), &link_labels)?;

    Ok(NoteDetail {
        path,
        kind,
        id,
        title,
        tags: frontmatter.tags,
        links,
        body,
        project,
        status: frontmatter.status,
        priority: frontmatter.priority,
        due: frontmatter.due,
        remind: frontmatter.remind,
        parent: frontmatter.parent,
    })
}

#[tauri::command]
pub fn save_note(
    state: State<AppState>,
    path: String,
    title: String,
    tags: Vec<String>,
    body: String,
) -> AppResult<NoteDetail> {
    let (workspace, linked) = config_scope(state.inner())?;
    let path_buf = PathBuf::from(&path);
    memory::ensure_allowed(&workspace, &linked, &path_buf)?;
    ensure_markdown_path(&path_buf)?;

    let parsed = memory::read_note_file_detailed(&path_buf)?;
    let mut frontmatter = parsed.frontmatter;
    let kind = memory::note_kind(&frontmatter, &path_buf);
    let is_structured = kind == "task" || kind == "project";
    let tags_empty = tags.is_empty();
    let stem = path_buf
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    let title_value = title.trim().to_string();
    let title_changed = !title_value.is_empty() && title_value != stem;

    if !title_value.is_empty() {
        frontmatter.title = Some(title_value);
    }
    frontmatter.tags = tags;
    frontmatter.updated = Some(memory::now_iso());

    let keep_frontmatter =
        parsed.has_frontmatter || is_structured || !tags_empty || title_changed;
    if keep_frontmatter {
        if frontmatter.kind.is_none() {
            frontmatter.kind = Some(
                match kind.as_str() {
                    "task" => "task",
                    "project" => "project",
                    _ => "note",
                }
                .into(),
            );
        }
        memory::write_note_file_preserving(&path_buf, &frontmatter, &body)?;
    } else {
        // Nota Markdown comum: não injeta frontmatter só por editar o corpo.
        memory::write_body_file(&path_buf, &body)?;
    }

    reindex_with_state(state.inner())?;
    read_note(state, path)
}

#[tauri::command]
pub fn create_note(
    state: State<AppState>,
    project: Option<String>,
    title: String,
) -> AppResult<NoteDetail> {
    let (workspace, linked) = config_scope(state.inner())?;
    let project = project.filter(|value| !value.is_empty() && value != "all");
    let dir = match project.as_deref() {
        Some(slug) => memory::notes_dir(&workspace, &linked, slug)
            .ok_or_else(|| AppError::NotFound(format!("projeto '{slug}' não encontrado")))?,
        None => workspace.join("global").join("inbox"),
    };
    std::fs::create_dir_all(&dir)?;

    let base = memory::sanitize_slug(if title.trim().is_empty() {
        "nota"
    } else {
        &title
    });
    let mut path = dir.join(format!("{base}.md"));
    if path.exists() {
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        path = dir.join(format!("{base}-{}.md", &suffix[..8]));
    }

    let now = memory::now_iso();
    let frontmatter = Frontmatter {
        kind: Some("note".into()),
        id: Some(uuid::Uuid::new_v4().to_string()),
        title: Some(if title.trim().is_empty() {
            "Nova nota".into()
        } else {
            title.trim().to_string()
        }),
        project: project.clone().filter(|value| value != "global"),
        created: Some(now.clone()),
        updated: Some(now),
        ..Default::default()
    };
    memory::write_note_file(&path, &frontmatter, "")?;
    reindex_with_state(state.inner())?;
    read_note(state, path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn delete_memory_file(state: State<AppState>, path: String) -> AppResult<()> {
    let (workspace, linked) = config_scope(state.inner())?;
    let path_buf = PathBuf::from(&path);
    memory::ensure_allowed(&workspace, &linked, &path_buf)?;
    ensure_markdown_path(&path_buf)?;
    if path_buf.exists() {
        std::fs::remove_file(&path_buf)?;
    }
    reindex_with_state(state.inner())?;
    Ok(())
}

#[tauri::command]
pub fn search_memory(
    state: State<AppState>,
    project: Option<String>,
    query: String,
    tags: Option<Vec<String>>,
    limit: Option<usize>,
) -> AppResult<Vec<MemoryHit>> {
    let project = project.filter(|value| value != "all");
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::memory::search(
        &conn,
        project.as_deref(),
        &query,
        tags.as_deref().unwrap_or(&[]),
        limit.unwrap_or(50),
    )
}

#[tauri::command]
pub async fn get_graph(app: AppHandle, project: Option<String>) -> AppResult<Graph> {
    let project = project.filter(|value| value != "all");
    let db_path = paths::db_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db::open(&db_path)?;
        crate::graph::build_graph_cached(&conn, project.as_deref())
    })
    .await
    .map_err(|e| AppError::Platform(format!("falha ao montar o grafo: {e}")))?
}

#[tauri::command]
pub fn list_tasks(
    state: State<AppState>,
    project: Option<String>,
    status: Option<String>,
    query: Option<String>,
) -> AppResult<Vec<Task>> {
    let project = project.filter(|value| value != "all");
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::tasks::list(
        &conn,
        project.as_deref(),
        status.as_deref(),
        query.as_deref(),
    )
}

#[tauri::command]
pub fn create_task(state: State<AppState>, input: NewTask) -> AppResult<Task> {
    let (workspace, linked) = config_scope(state.inner())?;
    let project = input
        .project
        .clone()
        .filter(|value| !value.is_empty() && value != "all");

    let dir = match project.as_deref() {
        Some(slug) => memory::tasks_dir(&workspace, &linked, slug)
            .ok_or_else(|| AppError::NotFound(format!("projeto '{slug}' não encontrado")))?,
        None => workspace.join("global").join("tasks"),
    };
    std::fs::create_dir_all(&dir)?;

    let id = uuid::Uuid::new_v4().to_string();
    let base = memory::sanitize_slug(&input.title);
    let path = dir.join(format!("{base}-{}.md", &id[..8]));

    let now = memory::now_iso();
    let due = input.due.clone().filter(|value| !value.trim().is_empty());
    let remind = input
        .remind
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| due.clone());

    let frontmatter = Frontmatter {
        kind: Some("task".into()),
        id: Some(id.clone()),
        title: Some(input.title.trim().to_string()),
        tags: input.tags.unwrap_or_default(),
        project: project.clone().filter(|value| value != "global"),
        status: Some("todo".into()),
        priority: Some(
            input
                .priority
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "medium".into()),
        ),
        due,
        remind,
        parent: input.parent.filter(|value| !value.trim().is_empty()),
        created: Some(now.clone()),
        updated: Some(now),
        ..Default::default()
    };

    memory::write_note_file(
        &path,
        &frontmatter,
        input.body.as_deref().unwrap_or_default(),
    )?;
    reindex_with_state(state.inner())?;

    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::tasks::get(&conn, &id)?
        .ok_or_else(|| AppError::NotFound("task não encontrada após criar".into()))
}

#[tauri::command]
pub fn update_task(state: State<AppState>, id: String, patch: TaskPatch) -> AppResult<Task> {
    let (workspace, linked) = config_scope(state.inner())?;
    let file_path = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::tasks::file_path_for_id(&conn, &id)?
            .ok_or_else(|| AppError::NotFound("task não encontrada".into()))?
    };

    let path = Path::new(&file_path).to_path_buf();
    memory::ensure_allowed(&workspace, &linked, &path)?;
    ensure_markdown_path(&path)?;
    let (mut frontmatter, mut body) = memory::read_note_file(&path)?;

    if let Some(title) = patch.title {
        if !title.trim().is_empty() {
            frontmatter.title = Some(title.trim().to_string());
        }
    }
    if let Some(status) = patch.status {
        frontmatter.status = Some(status);
    }
    if let Some(priority) = patch.priority {
        frontmatter.priority = Some(priority);
    }
    if let Some(due) = patch.due {
        frontmatter.due = if due.trim().is_empty() {
            None
        } else {
            Some(due)
        };
    }
    if let Some(remind) = patch.remind {
        frontmatter.remind = if remind.trim().is_empty() {
            None
        } else {
            Some(remind)
        };
    }
    if let Some(tags) = patch.tags {
        frontmatter.tags = tags;
    }
    if let Some(parent) = patch.parent {
        frontmatter.parent = if parent.trim().is_empty() {
            None
        } else {
            Some(parent)
        };
    }
    if let Some(new_body) = patch.body {
        body = new_body;
    }
    frontmatter.updated = Some(memory::now_iso());

    memory::write_note_file_preserving(&path, &frontmatter, &body)?;
    reindex_with_state(state.inner())?;

    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::tasks::get(&conn, &id)?.ok_or_else(|| AppError::NotFound("task não encontrada".into()))
}

#[tauri::command]
pub fn delete_task(state: State<AppState>, id: String) -> AppResult<()> {
    let (workspace, linked) = config_scope(state.inner())?;
    let file_path = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::tasks::file_path_for_id(&conn, &id)?
            .ok_or_else(|| AppError::NotFound("task não encontrada".into()))?
    };

    let path = Path::new(&file_path);
    memory::ensure_allowed(&workspace, &linked, path)?;
    ensure_markdown_path(path)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    reindex_with_state(state.inner())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guidance_block_preserves_existing_content_and_is_idempotent() {
        let dir = std::env::temp_dir().join(format!(
            "betastart-guidance-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("AGENTS.md"), "# Existing\n\nKeep me.\n").expect("agents");

        let path = write_guidance(&dir, "meu-projeto").expect("first");
        let raw = std::fs::read_to_string(&path).expect("read");
        assert!(raw.contains("Keep me."));
        assert_eq!(raw.matches(GUIDANCE_BEGIN).count(), 1);
        assert!(raw.contains("meu-projeto"));

        write_guidance(&dir, "meu-projeto").expect("second");
        let raw = std::fs::read_to_string(&path).expect("read again");
        assert_eq!(raw.matches(GUIDANCE_BEGIN).count(), 1);
        assert_eq!(raw.matches("# Existing").count(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
