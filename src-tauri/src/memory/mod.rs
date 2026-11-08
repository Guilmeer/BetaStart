//! Memória: arquivos Markdown reais no workspace, com frontmatter YAML.
//!
//! Layout:
//! ```text
//! workspace/
//!   global/inbox/*.md       (memória principal)
//!   global/tasks/*.md
//!   projects/<slug>/project.md
//!   projects/<slug>/notes/*.md
//!   projects/<slug>/tasks/*.md
//! ```

use std::path::Path;

use chrono::TimeZone;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::models::{MemoryRow, TaskRow};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,

    // task
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remind: Option<String>,

    // project
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
}

pub struct ParsedNote {
    pub frontmatter: Frontmatter,
    pub body: String,
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn parse_iso(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(datetime) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(datetime.timestamp());
    }
    for format in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"] {
        if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(value, format) {
            return chrono::Local
                .from_local_datetime(&datetime)
                .single()
                .map(|local| local.timestamp());
        }
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return date.and_hms_opt(0, 0, 0).and_then(|datetime| {
            chrono::Local
                .from_local_datetime(&datetime)
                .single()
                .map(|local| local.timestamp())
        });
    }
    None
}

pub fn parse_note(raw: &str) -> AppResult<ParsedNote> {
    let normalized = raw.trim_start_matches('\u{feff}').replace("\r\n", "\n");

    if let Some(rest) = normalized.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---") {
            let yaml = &rest[..end];
            let mut body = &rest[end + 4..];
            body = body.trim_start_matches('\n');

            let frontmatter: Frontmatter = if yaml.trim().is_empty() {
                Frontmatter::default()
            } else {
                serde_yaml::from_str(yaml)
                    .map_err(|e| AppError::Config(format!("frontmatter inválido: {e}")))?
            };
            return Ok(ParsedNote {
                frontmatter,
                body: body.to_string(),
            });
        }
    }

    Ok(ParsedNote {
        frontmatter: Frontmatter::default(),
        body: normalized,
    })
}

pub fn render_note(frontmatter: &Frontmatter, body: &str) -> String {
    let mut yaml = serde_yaml::to_string(frontmatter).unwrap_or_default();
    if let Some(stripped) = yaml.strip_prefix("---\n") {
        yaml = stripped.to_string();
    }
    let yaml = yaml.trim_end();
    let body = body.trim_start_matches('\n');

    if yaml.is_empty() || yaml == "{}" {
        format!("---\n---\n\n{body}")
    } else {
        format!("---\n{yaml}\n---\n\n{body}")
    }
}

pub fn read_note_file(path: &Path) -> AppResult<(Frontmatter, String)> {
    let raw = std::fs::read_to_string(path)?;
    let parsed = parse_note(&raw)?;
    Ok((parsed.frontmatter, parsed.body))
}

pub fn write_note_file(path: &Path, frontmatter: &Frontmatter, body: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, render_note(frontmatter, body))?;
    Ok(())
}

pub fn sanitize_slug(input: &str) -> String {
    let mut output = String::new();
    for character in input.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
        } else if character == '-' || character == '_' || character == ' ' || character == '.' {
            output.push('-');
        }
    }
    let output = output.trim_matches('-').to_string();
    if output.is_empty() {
        "item".into()
    } else {
        output
    }
}

pub fn project_from_path(workspace: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(workspace).ok()?;
    let parts: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect();

    match parts.first().map(String::as_str) {
        Some("projects") => parts.get(1).cloned(),
        Some("global") => Some("global".into()),
        _ => None,
    }
}

pub fn note_kind(frontmatter: &Frontmatter, path: &Path) -> String {
    if let Some(kind) = &frontmatter.kind {
        return kind.clone();
    }
    if path
        .file_name()
        .map(|name| name == "project.md")
        .unwrap_or(false)
    {
        return "project".into();
    }
    if path
        .extension()
        .map(|extension| extension.eq_ignore_ascii_case("md"))
        .unwrap_or(false)
    {
        if path
            .components()
            .any(|component| component.as_os_str() == "tasks")
        {
            return "task".into();
        }
        return "note".into();
    }
    "file".into()
}

pub fn ensure_inside(workspace: &Path, path: &Path) -> AppResult<()> {
    let root = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let target = if path.exists() {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    } else {
        let parent = path.parent().unwrap_or(path);
        let parent = parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf());
        parent.join(path.file_name().unwrap_or_default())
    };

    if !target.starts_with(&root) {
        return Err(AppError::Platform("caminho fora do workspace".into()));
    }
    Ok(())
}

fn mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// Reindexa o workspace inteiro no SQLite (memória + tasks + lembretes).
pub fn reindex(conn: &rusqlite::Connection, workspace: &Path) -> AppResult<usize> {
    let (memory_rows, task_rows) = scan_workspace(workspace)?;
    crate::db::memory::replace_all(conn, &memory_rows)?;
    crate::db::tasks::replace_all(conn, &task_rows)?;
    Ok(memory_rows.len())
}

pub fn scan_workspace(workspace: &Path) -> AppResult<(Vec<MemoryRow>, Vec<TaskRow>)> {
    let mut memory = Vec::new();
    let mut tasks = Vec::new();
    if workspace.exists() {
        scan_dir(workspace, workspace, &mut memory, &mut tasks);
    }
    Ok((memory, tasks))
}

fn scan_dir(workspace: &Path, dir: &Path, memory: &mut Vec<MemoryRow>, tasks: &mut Vec<TaskRow>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            scan_dir(workspace, &path, memory, tasks);
            continue;
        }
        if !path
            .extension()
            .map(|extension| extension.eq_ignore_ascii_case("md"))
            .unwrap_or(false)
        {
            continue;
        }

        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(parsed) = parse_note(&raw) else {
            continue;
        };

        let frontmatter = &parsed.frontmatter;
        let project = frontmatter
            .project
            .clone()
            .or_else(|| project_from_path(workspace, &path))
            .unwrap_or_else(|| "global".into());
        let kind = note_kind(frontmatter, &path);
        let title = frontmatter.title.clone().unwrap_or_else(|| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_else(|| name.clone())
        });
        let tags = frontmatter.tags.clone();
        let updated_at = mtime(&path);
        let file_path = path.to_string_lossy().to_string();

        memory.push(MemoryRow {
            file_path: file_path.clone(),
            kind: kind.clone(),
            title: title.clone(),
            project: project.clone(),
            tags: tags.clone(),
            content: parsed.body.chars().take(20_000).collect(),
            updated_at,
        });

        if kind == "task" {
            let id = frontmatter.id.clone().unwrap_or_else(|| {
                let stem = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
                    .unwrap_or_else(|| "task".into());
                format!("task-{stem}")
            });
            tasks.push(TaskRow {
                id,
                file_path,
                title,
                status: frontmatter.status.clone().unwrap_or_else(|| "todo".into()),
                priority: frontmatter
                    .priority
                    .clone()
                    .unwrap_or_else(|| "medium".into()),
                due: frontmatter.due.as_deref().and_then(parse_iso),
                remind: frontmatter.remind.as_deref().and_then(parse_iso),
                tags,
                project,
                parent_id: frontmatter.parent.clone(),
                updated_at,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_renders_frontmatter() {
        let raw = "---\ntype: task\ntitle: Minha tarefa\ntags:\n  - a\n  - b\ndue: 2026-11-07T18:00\n---\n\nCorpo da tarefa\n";
        let parsed = parse_note(raw).expect("parse");
        assert_eq!(parsed.frontmatter.kind.as_deref(), Some("task"));
        assert_eq!(parsed.frontmatter.title.as_deref(), Some("Minha tarefa"));
        assert_eq!(parsed.frontmatter.tags, vec!["a", "b"]);
        assert!(parsed.body.contains("Corpo da tarefa"));

        let rendered = render_note(&parsed.frontmatter, &parsed.body);
        let reparsed = parse_note(&rendered).expect("reparse");
        assert_eq!(reparsed.frontmatter.title.as_deref(), Some("Minha tarefa"));
        assert!(reparsed.body.contains("Corpo da tarefa"));
    }

    #[test]
    fn parses_dates() {
        assert!(parse_iso("2026-11-07").is_some());
        assert!(parse_iso("2026-11-07T18:00").is_some());
        assert!(parse_iso("").is_none());
    }

    #[test]
    fn sanitizes_slugs() {
        assert_eq!(sanitize_slug("Meu Projeto!"), "meu-projeto");
        assert_eq!(sanitize_slug("  "), "item");
    }

    #[test]
    fn indexes_workspace_into_db() {
        let root = std::env::temp_dir().join(format!("betastart-mem-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("projects/teste/notes")).expect("dirs");
        std::fs::create_dir_all(root.join("projects/teste/tasks")).expect("dirs");

        std::fs::write(
            root.join("projects/teste/project.md"),
            "---\ntype: project\nslug: teste\ntitle: Teste\n---\n",
        )
        .expect("project");
        std::fs::write(
            root.join("projects/teste/notes/nota.md"),
            "---\ntype: note\ntitle: Minha nota\ntags: [x]\n---\nconteudo\n",
        )
        .expect("note");
        std::fs::write(
            root.join("projects/teste/tasks/tarefa.md"),
            "---\ntype: task\nid: t1\ntitle: Tarefa\ndue: 2020-01-01T10:00\nremind: 2020-01-01T10:00\n---\ncorpo\n",
        )
        .expect("task");

        let (memory, tasks) = scan_workspace(&root).expect("scan");
        assert_eq!(memory.len(), 3);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].project, "teste");

        let db_path =
            std::env::temp_dir().join(format!("betastart-mem-test-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&db_path);
        let conn = crate::db::open(&db_path).expect("db");
        reindex(&conn, &root).expect("reindex");

        let files = crate::db::memory::list_files(&conn, Some("teste")).expect("files");
        assert_eq!(files.len(), 3);

        let hits =
            crate::db::memory::search(&conn, Some("teste"), "Minha", &[], 10).expect("search");
        assert_eq!(hits.len(), 1);

        let listed = crate::db::tasks::list(&conn, Some("teste"), None, None).expect("tasks");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title, "Tarefa");

        let due = crate::db::tasks::due_reminders(&conn, crate::db::now_secs()).expect("reminders");
        assert_eq!(due.len(), 1);

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_file(&db_path);
    }
}
