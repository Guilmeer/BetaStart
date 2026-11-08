//! Comandos Tauri do ambiente de memória + tasks.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tauri::State;

use crate::core::paths;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::memory::{self, Frontmatter};
use crate::models::{
    Graph, LinkInfo, MemoryFile, MemoryHit, NewTask, NoteDetail, ProjectInfo, Task, TaskPatch,
};
use crate::state::AppState;

fn workspace(state: &AppState) -> AppResult<PathBuf> {
    let path = state
        .config
        .lock()
        .map_err(|_| AppError::Config("config lock envenenado".into()))?
        .workspace
        .path
        .clone();
    Ok(paths::expand(&path))
}

fn reindex_with_state(state: &AppState) -> AppResult<usize> {
    let workspace = workspace(state)?;
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    memory::reindex(&conn, &workspace)
}

/// Resolve rótulos `[[...]]` para o arquivo correspondente (título ou stem).
fn resolve_links(state: &AppState, links: &[String]) -> AppResult<Vec<LinkInfo>> {
    if links.is_empty() {
        return Ok(Vec::new());
    }

    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    let mut stmt = conn.prepare("SELECT file_path, title FROM memory_index")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut index: HashMap<String, String> = HashMap::new();
    for row in rows {
        let (path, title) = row?;
        index
            .entry(title.trim().to_lowercase())
            .or_insert(path.clone());
        if let Some(stem) = Path::new(&path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_lowercase())
        {
            index.entry(stem).or_insert(path);
        }
    }

    Ok(links
        .iter()
        .map(|label| LinkInfo {
            label: label.clone(),
            path: index.get(&label.trim().to_lowercase()).cloned(),
        })
        .collect())
}

#[tauri::command]
pub fn reindex_memory(state: State<AppState>) -> AppResult<usize> {
    reindex_with_state(state.inner())
}

#[tauri::command]
pub fn list_projects(state: State<AppState>) -> AppResult<Vec<ProjectInfo>> {
    let workspace = workspace(state.inner())?;
    let projects_dir = workspace.join("projects");
    let mut out = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let project_file = path.join("project.md");
            let Ok((frontmatter, _)) = memory::read_note_file(&project_file) else {
                continue;
            };
            let slug = frontmatter
                .slug
                .clone()
                .or_else(|| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .unwrap_or_default();
            let title = frontmatter.title.clone().unwrap_or_else(|| slug.clone());
            out.push(ProjectInfo {
                slug,
                title,
                description: frontmatter.description.unwrap_or_default(),
                color: frontmatter.color,
            });
        }
    }

    out.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()));
    Ok(out)
}

#[tauri::command]
pub fn create_project(
    state: State<AppState>,
    slug: String,
    title: String,
) -> AppResult<ProjectInfo> {
    let workspace = workspace(state.inner())?;
    let slug = memory::sanitize_slug(if slug.trim().is_empty() {
        &title
    } else {
        &slug
    });
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
    })
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

#[tauri::command]
pub fn read_note(state: State<AppState>, path: String) -> AppResult<NoteDetail> {
    let workspace = workspace(state.inner())?;
    let path_buf = PathBuf::from(&path);
    memory::ensure_inside(&workspace, &path_buf)?;

    let (frontmatter, body) = memory::read_note_file(&path_buf)?;
    let kind = memory::note_kind(&frontmatter, &path_buf);
    let title = frontmatter.title.clone().unwrap_or_else(|| {
        path_buf
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default()
    });
    let project = frontmatter
        .project
        .clone()
        .or_else(|| memory::project_from_path(&workspace, &path_buf));
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
    let links = resolve_links(state.inner(), &link_labels)?;

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
    let workspace = workspace(state.inner())?;
    let path_buf = PathBuf::from(&path);
    memory::ensure_inside(&workspace, &path_buf)?;

    let (mut frontmatter, _) = memory::read_note_file(&path_buf)?;
    if !title.trim().is_empty() {
        frontmatter.title = Some(title.trim().to_string());
    }
    frontmatter.tags = tags;
    frontmatter.updated = Some(memory::now_iso());
    if frontmatter.kind.is_none() {
        frontmatter.kind = Some("note".into());
    }

    memory::write_note_file(&path_buf, &frontmatter, &body)?;
    reindex_with_state(state.inner())?;
    read_note(state, path)
}

#[tauri::command]
pub fn create_note(
    state: State<AppState>,
    project: Option<String>,
    title: String,
) -> AppResult<NoteDetail> {
    let workspace = workspace(state.inner())?;
    let project = project.filter(|value| !value.is_empty() && value != "all");
    let dir = match project.as_deref() {
        Some(slug) if slug != "global" => workspace.join("projects").join(slug).join("notes"),
        _ => workspace.join("global").join("inbox"),
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
    let workspace = workspace(state.inner())?;
    let path_buf = PathBuf::from(&path);
    memory::ensure_inside(&workspace, &path_buf)?;
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
pub fn get_graph(state: State<AppState>, project: Option<String>) -> AppResult<Graph> {
    let project = project.filter(|value| value != "all");
    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    crate::graph::build_graph(&conn, project.as_deref())
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
    let workspace = workspace(state.inner())?;
    let project = input
        .project
        .clone()
        .filter(|value| !value.is_empty() && value != "all");

    let dir = match project.as_deref() {
        Some(slug) if slug != "global" && workspace.join("projects").join(slug).exists() => {
            workspace.join("projects").join(slug).join("tasks")
        }
        _ => workspace.join("global").join("tasks"),
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
    let workspace = workspace(state.inner())?;
    let file_path = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::tasks::file_path_for_id(&conn, &id)?
            .ok_or_else(|| AppError::NotFound("task não encontrada".into()))?
    };

    let path = Path::new(&file_path).to_path_buf();
    memory::ensure_inside(&workspace, &path)?;
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

    memory::write_note_file(&path, &frontmatter, &body)?;
    reindex_with_state(state.inner())?;

    let conn = state
        .db
        .lock()
        .map_err(|_| AppError::Database("db lock envenenado".into()))?;
    db::tasks::get(&conn, &id)?.ok_or_else(|| AppError::NotFound("task não encontrada".into()))
}

#[tauri::command]
pub fn delete_task(state: State<AppState>, id: String) -> AppResult<()> {
    let workspace = workspace(state.inner())?;
    let file_path = {
        let conn = state
            .db
            .lock()
            .map_err(|_| AppError::Database("db lock envenenado".into()))?;
        db::tasks::file_path_for_id(&conn, &id)?
            .ok_or_else(|| AppError::NotFound("task não encontrada".into()))?
    };

    let path = Path::new(&file_path);
    memory::ensure_inside(&workspace, path)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    reindex_with_state(state.inner())?;
    Ok(())
}
