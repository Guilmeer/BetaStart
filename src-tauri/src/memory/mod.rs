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

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::TimeZone;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

use crate::core::config::{LinkedProjectConfig, WorkspaceConfig};
use crate::error::{AppError, AppResult};
use crate::models::{MemoryRow, ProjectInfo, TaskRow};

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
    pub has_frontmatter: bool,
}

/// Projeto vinculado a uma pasta existente do usuário.
#[derive(Debug, Clone)]
pub struct LinkedRoot {
    pub slug: String,
    pub title: String,
    pub root: PathBuf,
}

impl LinkedRoot {
    pub fn from_config(config: &WorkspaceConfig) -> Vec<Self> {
        config
            .linked_projects
            .iter()
            .filter(|item| !item.slug.trim().is_empty() && !item.root.trim().is_empty())
            .map(Self::from_item)
            .collect()
    }

    pub fn from_item(item: &LinkedProjectConfig) -> Self {
        let root = crate::core::paths::expand(&item.root);
        let title = item
            .title
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                root.file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| item.slug.clone())
            });
        Self {
            slug: item.slug.clone(),
            title,
            root,
        }
    }
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
                has_frontmatter: true,
            });
        }
    }

    Ok(ParsedNote {
        frontmatter: Frontmatter::default(),
        body: normalized,
        has_frontmatter: false,
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

const KNOWN_FRONTMATTER_KEYS: &[&str] = &[
    "type",
    "id",
    "title",
    "tags",
    "project",
    "links",
    "created",
    "updated",
    "status",
    "priority",
    "due",
    "parent",
    "remind",
    "slug",
    "description",
    "color",
    "archived",
];

/// Atualiza o frontmatter preservando chaves desconhecidas do usuário.
pub fn write_note_file_preserving(
    path: &Path,
    frontmatter: &Frontmatter,
    body: &str,
) -> AppResult<()> {
    if !path.exists() {
        return write_note_file(path, frontmatter, body);
    }

    let raw = std::fs::read_to_string(path)?;
    let normalized = raw.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut mapping = serde_yaml::Mapping::new();

    if let Some(rest) = normalized.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---") {
            let yaml = &rest[..end];
            if !yaml.trim().is_empty() {
                if let Ok(serde_yaml::Value::Mapping(existing)) =
                    serde_yaml::from_str::<serde_yaml::Value>(yaml)
                {
                    mapping = existing;
                }
            }
        }
    }

    if let Ok(serde_yaml::Value::Mapping(known)) = serde_yaml::to_value(frontmatter) {
        for key in KNOWN_FRONTMATTER_KEYS {
            mapping.remove(serde_yaml::Value::String((*key).into()));
        }
        for (key, value) in known {
            mapping.insert(key, value);
        }
    }

    let yaml = serde_yaml::to_string(&serde_yaml::Value::Mapping(mapping)).unwrap_or_default();
    let yaml = yaml.strip_prefix("---\n").unwrap_or(&yaml).trim_end();
    let body = body.trim_start_matches('\n');
    let output = if yaml.is_empty() || yaml == "{}" {
        format!("---\n---\n\n{body}")
    } else {
        format!("---\n{yaml}\n---\n\n{body}")
    };
    std::fs::write(path, output)?;
    Ok(())
}

pub fn read_note_file(path: &Path) -> AppResult<(Frontmatter, String)> {
    let raw = std::fs::read_to_string(path)?;
    let parsed = parse_note(&raw)?;
    Ok((parsed.frontmatter, parsed.body))
}

pub fn read_note_file_detailed(path: &Path) -> AppResult<ParsedNote> {
    let raw = std::fs::read_to_string(path)?;
    parse_note(&raw)
}

pub fn write_note_file(path: &Path, frontmatter: &Frontmatter, body: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, render_note(frontmatter, body))?;
    Ok(())
}

/// Grava apenas o corpo, preservando arquivos Markdown que nunca tiveram frontmatter.
pub fn write_body_file(path: &Path, body: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, body)?;
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
        if matches!(kind.as_str(), "note" | "task" | "project") {
            return kind.clone();
        }
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

fn canonical_or_parent(path: &Path) -> PathBuf {
    let target = if path.exists() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    dunce::canonicalize(target).unwrap_or_else(|_| target.to_path_buf())
}

pub fn ensure_allowed(
    workspace: &Path,
    linked: &[LinkedRoot],
    path: &Path,
) -> AppResult<()> {
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(AppError::Platform("caminho com '..' não permitido".into()));
    }

    let target = canonical_or_parent(path);
    let root = dunce::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    if target.starts_with(&root) {
        return Ok(());
    }

    for item in linked {
        if !item.root.exists() {
            continue;
        }
        let linked_root =
            dunce::canonicalize(&item.root).unwrap_or_else(|_| item.root.to_path_buf());
        if target.starts_with(&linked_root) {
            return Ok(());
        }
    }

    Err(AppError::Platform("caminho fora do workspace/projetos vinculados".into()))
}

fn mtime(path: &Path) -> i64 {
    // Millisegundos: segundos faziam duas edições no mesmo segundo passarem
    // como "sem mudança" no reindex incremental.
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// Acumula o que foi lido do disco durante um scan.
#[derive(Default)]
struct ScanState {
    memory: Vec<MemoryRow>,
    tasks: Vec<TaskRow>,
    /// Todo caminho visitado, mesmo os que foram reaproveitados do índice.
    seen: HashSet<String>,
}

/// Reindexa o workspace inteiro no SQLite (memória + tasks + lembretes).
#[allow(dead_code)]
pub fn reindex(conn: &rusqlite::Connection, workspace: &Path) -> AppResult<usize> {
    reindex_with_linked(conn, workspace, &[])
}

/// Reindexa o workspace + projetos vinculados, relendo só o que mudou.
///
/// O `mtime` de cada arquivo é comparado com o índice: arquivos inalterados
/// não são lidos nem reescritos. Alterações são aplicadas linha a linha e os
/// triggers do FTS mantêm a busca em sincronia.
pub fn reindex_with_linked(
    conn: &rusqlite::Connection,
    workspace: &Path,
    linked: &[LinkedRoot],
) -> AppResult<usize> {
    let known = crate::db::memory::snapshot(conn)?;
    let mut state = ScanState::default();

    if workspace.exists() {
        scan_tree(workspace, workspace, None, &known, &mut state);
    }
    for item in linked {
        if item.root.exists() {
            scan_tree(
                workspace,
                &item.root,
                Some(&item.slug),
                &known,
                &mut state,
            );
        }
    }

    dedupe_rows(&mut state.memory, &mut state.tasks);
    sync_index(conn, &known, state)
}

fn sync_index(
    conn: &rusqlite::Connection,
    known: &std::collections::HashMap<String, (i64, String)>,
    state: ScanState,
) -> AppResult<usize> {
    let total = state.seen.len();

    // Memória: remove sumidos, grava só os que mudaram.
    let removed: Vec<String> = known
        .keys()
        .filter(|path| !state.seen.contains(*path))
        .cloned()
        .collect();
    crate::db::memory::delete_paths(conn, &removed)?;

    let changed: Vec<MemoryRow> = state
        .memory
        .into_iter()
        .filter(|row| known.get(&row.file_path) != Some(&(row.updated_at, row.project.clone())))
        .collect();
    crate::db::memory::upsert_rows(conn, &changed)?;
    let memory_changed = !changed.is_empty() || !removed.is_empty();

    // Tasks derivam dos mesmos arquivos: apaga as removidas/alteradas e
    // reinsere só elas.
    let mut touched: Vec<String> = removed;
    touched.extend(changed.iter().map(|row| row.file_path.clone()));
    crate::db::tasks::delete_paths(conn, &touched)?;

    let touched: HashSet<&str> = touched.iter().map(String::as_str).collect();
    let tasks: Vec<TaskRow> = state
        .tasks
        .into_iter()
        .filter(|row| touched.contains(row.file_path.as_str()))
        .collect();
    let tasks_changed = !tasks.is_empty();
    crate::db::tasks::insert_rows(conn, &tasks)?;
    crate::db::tasks::prune_reminders(conn)?;

    // Só invalida o cache do grafo quando algo realmente mudou.
    if memory_changed || tasks_changed {
        crate::db::meta_set(
            conn,
            "memory_version",
            &std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or(0)
                .to_string(),
        )?;
    }

    Ok(total)
}

fn dedupe_rows(memory: &mut Vec<MemoryRow>, tasks: &mut Vec<TaskRow>) {
    let mut seen = HashSet::new();
    memory.retain(|row| seen.insert(row.file_path.clone()));
    let mut seen = HashSet::new();
    tasks.retain(|row| seen.insert(row.file_path.clone()));
}

/// Extrai links no formato `[[destino]]` do corpo de uma nota.
pub fn extract_wiki_links(body: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = body;

    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        if let Some(end) = after.find("]]") {
            let link = after[..end].trim();
            if !link.is_empty() {
                links.push(link.to_string());
            }
            rest = &after[end + 2..];
        } else {
            break;
        }
    }

    links
}

const BUILTIN_IGNORED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    "coverage",
    ".next",
    ".nuxt",
    ".cache",
    "__pycache__",
    ".venv",
    "venv",
    "vendor",
    "Pods",
    ".idea",
    ".vscode",
];

fn is_ignored_dir(name: &str) -> bool {
    BUILTIN_IGNORED_DIRS
        .iter()
        .any(|item| item.eq_ignore_ascii_case(name))
}

/// Caminho que o indexador/watcher deve ignorar.
pub fn is_ignored_path(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        if name.is_empty() || name == "." || name == ".." {
            return false;
        }
        if name == ".betastart" || name == ".gitignore" || name == ".betastartignore" {
            return false;
        }
        name.starts_with('.') || is_ignored_dir(&name)
    })
}

fn should_visit(entry: &ignore::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
    if is_dir {
        if name == ".betastart" {
            return true;
        }
        if name.starts_with('.') {
            return false;
        }
        if is_ignored_dir(&name) {
            return false;
        }
    } else if name.starts_with('.') {
        return false;
    }
    true
}

fn walk_markdown<F>(base: &Path, force_include_ignored: bool, mut visit: F)
where
    F: FnMut(&Path),
{
    let mut builder = WalkBuilder::new(base);
    builder
        .hidden(false)
        .git_ignore(!force_include_ignored)
        .git_global(false)
        .git_exclude(!force_include_ignored)
        .require_git(false)
        .follow_links(false)
        .max_filesize(Some(5 * 1024 * 1024))
        .add_custom_ignore_filename(".betastartignore")
        .filter_entry(should_visit);

    for entry in builder.build().flatten() {
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        let path = entry.path();
        if path
            .extension()
            .map(|extension| extension.eq_ignore_ascii_case("md"))
            .unwrap_or(false)
        {
            visit(path);
        }
    }
}

#[allow(dead_code)] // usado nos testes
pub fn scan_workspace(workspace: &Path) -> AppResult<(Vec<MemoryRow>, Vec<TaskRow>)> {
    let mut state = ScanState::default();
    if workspace.exists() {
        scan_tree(
            workspace,
            workspace,
            None,
            &std::collections::HashMap::new(),
            &mut state,
        );
    }
    Ok((state.memory, state.tasks))
}

fn scan_tree(
    workspace: &Path,
    base: &Path,
    project_override: Option<&str>,
    known: &std::collections::HashMap<String, (i64, String)>,
    state: &mut ScanState,
) {
    if !base.exists() {
        return;
    }

    let mut seen = HashSet::new();
    let mut paths = Vec::new();
    walk_markdown(base, false, |path| {
        if seen.insert(path.to_path_buf()) {
            paths.push(path.to_path_buf());
        }
    });

    // `.betastart` pode estar no .gitignore, mas ainda é estado do projeto.
    let betastart = base.join(".betastart");
    if betastart.exists() {
        walk_markdown(&betastart, true, |path| {
            if seen.insert(path.to_path_buf()) {
                paths.push(path.to_path_buf());
            }
        });
    }

    for path in paths {
        scan_file(workspace, project_override, &path, known, state);
    }
}

fn scan_file(
    workspace: &Path,
    project_override: Option<&str>,
    path: &Path,
    known: &std::collections::HashMap<String, (i64, String)>,
    state: &mut ScanState,
) {
    let file_path = path.to_string_lossy().to_string();
    let updated_at = mtime(path);

    // Inalterado desde a última indexação: reaproveita o que já está no banco.
    if let Some((known_mtime, known_project)) = known.get(&file_path) {
        let same_project =
            project_override.is_none() || project_override == Some(known_project.as_str());
        if *known_mtime == updated_at && same_project {
            state.seen.insert(file_path);
            return;
        }
    }

    let Ok(parsed) = read_note_file_detailed(path) else {
        return;
    };
    state.seen.insert(file_path.clone());

    let frontmatter = &parsed.frontmatter;
    let project = project_override
        .map(ToString::to_string)
        .or_else(|| frontmatter.project.clone())
        .or_else(|| project_from_path(workspace, path))
        .unwrap_or_else(|| "global".into());
    let kind = note_kind(frontmatter, path);
    let title = frontmatter.title.clone().unwrap_or_else(|| {
        path.file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "sem-titulo".into())
    });
    let tags = frontmatter.tags.clone();
    let mut links = frontmatter.links.clone();
    links.extend(extract_wiki_links(&parsed.body));
    links.sort();
    links.dedup();

    state.memory.push(MemoryRow {
        file_path: file_path.clone(),
        kind: kind.clone(),
        title: title.clone(),
        project: project.clone(),
        tags: tags.clone(),
        links,
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
        state.tasks.push(TaskRow {
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

pub fn unique_slug(base: &str, used: &HashSet<String>) -> String {
    if !used.contains(&base.to_lowercase()) {
        return base.to_string();
    }
    for index in 2..1000 {
        let candidate = format!("{base}-{index}");
        if !used.contains(&candidate.to_lowercase()) {
            return candidate;
        }
    }
    format!("{base}-{}", uuid::Uuid::new_v4().simple())
}

/// Acrescenta entradas ao `.betastartignore` da pasta, sem duplicar linhas.
pub fn append_ignore_entries(root: &Path, entries: &[String]) -> AppResult<usize> {
    if entries.is_empty() {
        return Ok(0);
    }

    let path = root.join(".betastartignore");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = existing
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    let mut added = 0;
    for entry in entries {
        let entry = entry.trim().replace('\\', "/");
        if entry.is_empty() {
            continue;
        }
        let normalized = entry.trim_end_matches('/').to_lowercase();
        let duplicate = lines
            .iter()
            .any(|line| line.trim_end_matches('/').to_lowercase() == normalized);
        if duplicate {
            continue;
        }
        lines.push(entry);
        added += 1;
    }

    if added > 0 {
        let mut output = lines.join("\n");
        output.push('\n');
        std::fs::write(&path, output)?;
    }
    Ok(added)
}

/// Vincula uma pasta existente como projeto, sem duplicar arquivos.
///
/// Atualiza `workspace_cfg` em memória; quem chama deve persistir o config.
/// Também cria `<root>/.betastart/` (manifesto, notas e tasks próprias) e
/// acrescenta as entradas pedidas em `<root>/.betastartignore`.
pub fn link_project(
    workspace: &Path,
    workspace_cfg: &mut WorkspaceConfig,
    root: &str,
    title: Option<String>,
    slug: Option<String>,
    ignore: &[String],
) -> AppResult<ProjectInfo> {
    let workspace_canonical =
        dunce::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());

    let expanded = crate::core::paths::expand(root.trim());
    if !expanded.exists() {
        return Err(AppError::Config("pasta não encontrada".into()));
    }
    if !expanded.is_dir() {
        return Err(AppError::Config(
            "o caminho informado não é uma pasta".into(),
        ));
    }
    let root_path = dunce::canonicalize(&expanded)
        .map_err(|error| AppError::Platform(format!("não foi possível abrir a pasta: {error}")))?;

    if root_path.parent().is_none() {
        return Err(AppError::Config(
            "não é possível vincular a raiz de um disco".into(),
        ));
    }
    if root_path == workspace_canonical || root_path.starts_with(&workspace_canonical) {
        return Err(AppError::Config(
            "essa pasta já fica dentro do workspace do Beta Start".into(),
        ));
    }
    if workspace_canonical.starts_with(&root_path) {
        return Err(AppError::Config(
            "essa pasta contém o workspace do Beta Start; escolha a pasta do projeto".into(),
        ));
    }

    let linked = LinkedRoot::from_config(workspace_cfg);

    // Já vinculada: idempotente, apenas aplica ignores/título novos.
    if let Some(existing) = linked.iter().find(|item| {
        dunce::canonicalize(&item.root).unwrap_or_else(|_| item.root.clone()) == root_path
    }) {
        append_ignore_entries(&root_path, ignore)?;
        let projects = list_projects(workspace, &linked)?;
        return projects
            .into_iter()
            .find(|item| item.slug == existing.slug)
            .ok_or_else(|| AppError::NotFound("projeto vinculado não encontrado".into()));
    }

    for item in &linked {
        let item_root = dunce::canonicalize(&item.root).unwrap_or_else(|_| item.root.clone());
        if root_path.starts_with(&item_root) || item_root.starts_with(&root_path) {
            return Err(AppError::Config(
                "essa pasta se sobrepõe a outro projeto vinculado".into(),
            ));
        }
    }

    let projects = list_projects(workspace, &linked)?;
    let used: HashSet<String> = projects
        .iter()
        .map(|item| item.slug.to_lowercase())
        .collect();

    let project_file = root_path.join(".betastart").join("project.md");
    let (existing_frontmatter, _) = read_note_file(&project_file)
        .unwrap_or((Frontmatter::default(), String::new()));
    let fallback_title = title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| existing_frontmatter.title.clone())
        .or_else(|| {
            root_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "Projeto".into());
    let base_slug = slug
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(sanitize_slug)
        .or_else(|| {
            existing_frontmatter
                .slug
                .clone()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| sanitize_slug(&fallback_title));
    let slug = unique_slug(&base_slug, &used);

    for sub in ["notes", "tasks", "assets"] {
        std::fs::create_dir_all(root_path.join(".betastart").join(sub))?;
    }

    if !project_file.exists() {
        let now = now_iso();
        let frontmatter = Frontmatter {
            kind: Some("project".into()),
            slug: Some(slug.clone()),
            title: Some(fallback_title.clone()),
            description: Some(String::new()),
            color: Some("#8b5cf6".into()),
            created: Some(now.clone()),
            updated: Some(now),
            ..Default::default()
        };
        write_note_file(&project_file, &frontmatter, "")?;
    } else if let Ok((mut frontmatter, body)) = read_note_file(&project_file) {
        let mut dirty = false;
        if frontmatter.slug.as_deref() != Some(slug.as_str()) {
            frontmatter.slug = Some(slug.clone());
            dirty = true;
        }
        if title.is_some()
            && frontmatter.title.as_deref() != Some(fallback_title.as_str())
        {
            frontmatter.title = Some(fallback_title.clone());
            dirty = true;
        }
        if dirty {
            write_note_file_preserving(&project_file, &frontmatter, &body)?;
        }
    }

    workspace_cfg.linked_projects.push(LinkedProjectConfig {
        slug: slug.clone(),
        root: root_path.to_string_lossy().to_string(),
        title: Some(fallback_title.clone()),
    });

    append_ignore_entries(&root_path, ignore)?;

    let (final_frontmatter, _) = read_note_file(&project_file)
        .unwrap_or((Frontmatter::default(), String::new()));
    Ok(ProjectInfo {
        slug,
        title: fallback_title,
        description: final_frontmatter.description.unwrap_or_default(),
        color: final_frontmatter.color.or_else(|| Some("#8b5cf6".into())),
        root_path: Some(root_path.to_string_lossy().to_string()),
        mode: "linked".into(),
        available: true,
    })
}

/// Desvincula um projeto (não apaga nenhum arquivo do usuário).
pub fn unlink_project(workspace_cfg: &mut WorkspaceConfig, slug: &str) -> AppResult<()> {
    let before = workspace_cfg.linked_projects.len();
    workspace_cfg
        .linked_projects
        .retain(|item| item.slug != slug);
    if workspace_cfg.linked_projects.len() == before {
        return Err(AppError::NotFound(
            "projeto vinculado não encontrado".into(),
        ));
    }
    Ok(())
}

/// Lista projetos gerenciados + projetos vinculados a pastas do usuário.
pub fn list_projects(workspace: &Path, linked: &[LinkedRoot]) -> AppResult<Vec<ProjectInfo>> {
    let projects_dir = workspace.join("projects");
    let mut out = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let project_file = path.join("project.md");
            let Ok((frontmatter, _)) = read_note_file(&project_file) else {
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
                root_path: Some(path.to_string_lossy().to_string()),
                mode: "managed".into(),
                available: true,
            });
        }
    }

    for item in linked {
        let project_file = item.root.join(".betastart").join("project.md");
        let (frontmatter, _) = read_note_file(&project_file)
            .unwrap_or((Frontmatter::default(), String::new()));
        let slug = item.slug.clone();
        let title = frontmatter
            .title
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| item.title.clone());
        out.push(ProjectInfo {
            slug,
            title,
            description: frontmatter.description.unwrap_or_default(),
            color: frontmatter.color,
            root_path: Some(item.root.to_string_lossy().to_string()),
            mode: "linked".into(),
            available: item.root.exists(),
        });
    }

    out.sort_by(|left, right| left.title.to_lowercase().cmp(&right.title.to_lowercase()));
    Ok(out)
}

/// Resolve o projeto de um caminho, considerando roots vinculados.
pub fn project_for_path(workspace: &Path, linked: &[LinkedRoot], path: &Path) -> Option<String> {
    let target = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    for item in linked {
        let root = dunce::canonicalize(&item.root).unwrap_or_else(|_| item.root.clone());
        if target.starts_with(&root) {
            return Some(item.slug.clone());
        }
    }
    project_from_path(workspace, path)
}

/// Diretório base de um projeto (workspace gerenciado ou root vinculado).
pub fn project_dir(
    workspace: &Path,
    linked: &[LinkedRoot],
    slug: &str,
) -> Option<PathBuf> {
    if slug == "global" {
        return Some(workspace.join("global"));
    }
    if let Some(item) = linked.iter().find(|item| item.slug == slug) {
        return Some(item.root.clone());
    }
    let dir = workspace.join("projects").join(slug);
    dir.exists().then_some(dir)
}

/// Diretório onde novas notas do projeto são criadas.
pub fn notes_dir(
    workspace: &Path,
    linked: &[LinkedRoot],
    slug: &str,
) -> Option<PathBuf> {
    if slug == "global" {
        return Some(workspace.join("global").join("inbox"));
    }
    if let Some(item) = linked.iter().find(|item| item.slug == slug) {
        if !item.root.exists() {
            return None;
        }
        return Some(item.root.join(".betastart").join("notes"));
    }
    let root = project_dir(workspace, linked, slug)?;
    Some(root.join("notes"))
}

/// Diretório onde novas tasks do projeto são criadas.
pub fn tasks_dir(
    workspace: &Path,
    linked: &[LinkedRoot],
    slug: &str,
) -> Option<PathBuf> {
    if slug == "global" {
        return Some(workspace.join("global").join("tasks"));
    }
    if let Some(item) = linked.iter().find(|item| item.slug == slug) {
        if !item.root.exists() {
            return None;
        }
        return Some(item.root.join(".betastart").join("tasks"));
    }
    let root = project_dir(workspace, linked, slug)?;
    Some(root.join("tasks"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_renders_frontmatter() {
        let raw = "---\ntype: task\ntitle: Minha tarefa\ntags:\n  - a\n  - b\ndue: 2026-11-07T18:00\n---\n\nCorpo da tarefa\n";
        let parsed = parse_note(raw).expect("parse");
        assert!(parsed.has_frontmatter);
        assert_eq!(parsed.frontmatter.kind.as_deref(), Some("task"));
        assert_eq!(parsed.frontmatter.title.as_deref(), Some("Minha tarefa"));
        assert_eq!(parsed.frontmatter.tags, vec!["a", "b"]);
        assert!(parsed.body.contains("Corpo da tarefa"));

        let plain = parse_note("Só corpo, sem frontmatter\n").expect("plain");
        assert!(!plain.has_frontmatter);

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

        // Termos múltiplos exigem todas as palavras, em qualquer ordem.
        let multi = crate::db::memory::search(&conn, Some("teste"), "nota minha", &[], 10)
            .expect("search multi");
        assert_eq!(multi.len(), 1);
        let missing =
            crate::db::memory::search(&conn, Some("teste"), "nota inexistente", &[], 10)
                .expect("search missing");
        assert!(missing.is_empty());

        let listed = crate::db::tasks::list(&conn, Some("teste"), None, None).expect("tasks");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title, "Tarefa");

        let due = crate::db::tasks::due_reminders(&conn, crate::db::now_secs()).expect("reminders");
        assert_eq!(due.len(), 1);

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn indexes_linked_project_in_place() {
        let base =
            std::env::temp_dir().join(format!("betastart-linked-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let workspace = base.join("workspace");
        let root = base.join("meu-projeto");

        std::fs::create_dir_all(workspace.join("global/inbox")).expect("workspace");
        std::fs::create_dir_all(root.join("docs")).expect("docs");
        std::fs::create_dir_all(root.join("tasks")).expect("tasks");
        std::fs::create_dir_all(root.join(".betastart")).expect("betastart");

        std::fs::write(root.join("README.md"), "# Readme\n\nconteudo\n").expect("readme");
        std::fs::write(
            root.join("docs/arquitetura.md"),
            "# Arquitetura\n\nconteudo\n",
        )
        .expect("docs");
        std::fs::write(
            root.join("tasks/tarefa.md"),
            "---\ntype: task\nid: linked-1\ntitle: Tarefa vinculada\n---\ncorpo\n",
        )
        .expect("task");
        std::fs::write(
            root.join(".betastart/project.md"),
            "---\ntype: project\nslug: meu-projeto\ntitle: Meu Projeto\n---\n",
        )
        .expect("project");
        std::fs::write(root.join("ignored.md"), "# Ignorado\n").expect("ignored");
        std::fs::write(root.join(".gitignore"), "ignored.md\n.betastart/\n")
            .expect("gitignore");

        let linked = vec![LinkedRoot {
            slug: "meu-projeto".into(),
            title: "Meu Projeto".into(),
            root: root.clone(),
        }];
        let db_path = base.join("data.db");
        let conn = crate::db::open(&db_path).expect("db");
        reindex_with_linked(&conn, &workspace, &linked).expect("reindex");

        let files = crate::db::memory::list_files(&conn, Some("meu-projeto")).expect("files");
        assert!(files.iter().any(|file| file.name == "README.md"));
        assert!(files.iter().any(|file| file.name == "arquitetura.md"));
        assert!(files.iter().any(|file| file.kind == "project"));
        assert!(!files.iter().any(|file| file.name == "ignored.md"));

        let tasks = crate::db::tasks::list(&conn, Some("meu-projeto"), None, None).expect("tasks");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Tarefa vinculada");

        let projects = list_projects(&workspace, &linked).expect("projects");
        let project = projects
            .iter()
            .find(|item| item.slug == "meu-projeto")
            .expect("linked project");
        assert_eq!(project.mode, "linked");
        assert!(project.available);

        ensure_allowed(&workspace, &linked, &root.join("README.md")).expect("allowed");
        ensure_allowed(&workspace, &linked, &base.join("fora.md")).expect_err("blocked");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn links_existing_folder_without_duplication() {
        let base = std::env::temp_dir().join(format!(
            "betastart-link-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let workspace = base.join("workspace");
        let root = base.join("Meu Projeto");
        std::fs::create_dir_all(workspace.join("projects")).expect("workspace");
        std::fs::create_dir_all(root.join("SkillAndPlugin")).expect("root");
        std::fs::write(root.join("MEMORIA.md"), "# Memoria\n").expect("note");

        let mut config = WorkspaceConfig::default();
        config.path = workspace.to_string_lossy().to_string();

        let project = link_project(
            &workspace,
            &mut config,
            &root.to_string_lossy(),
            Some("Prisma Historico".into()),
            None,
            &["SkillAndPlugin/".into(), "examples/".into()],
        )
        .expect("link");

        assert_eq!(project.slug, "prisma-historico");
        assert_eq!(project.mode, "linked");
        assert_eq!(config.linked_projects.len(), 1);
        assert!(root.join(".betastart/project.md").exists());
        assert!(root.join(".betastart/notes").exists());

        let ignore = std::fs::read_to_string(root.join(".betastartignore")).expect("ignore");
        assert!(ignore.contains("SkillAndPlugin/"));
        assert!(ignore.contains("examples/"));

        // Chamar de novo é idempotente (não duplica o vínculo nem o arquivo).
        let again = link_project(
            &workspace,
            &mut config,
            &root.to_string_lossy(),
            None,
            None,
            &["examples/".into()],
        )
        .expect("relink");
        assert_eq!(again.slug, "prisma-historico");
        assert_eq!(config.linked_projects.len(), 1);
        let ignore = std::fs::read_to_string(root.join(".betastartignore")).expect("ignore");
        assert_eq!(ignore.matches("examples/").count(), 1);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn incremental_reindex_updates_and_removes() {
        let base = std::env::temp_dir().join(format!(
            "betastart-incremental-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let workspace = base.join("workspace");
        std::fs::create_dir_all(workspace.join("global/inbox")).expect("workspace");

        std::fs::write(workspace.join("global/inbox/a.md"), "# Alpha\n\nconteudo antigo\n")
            .expect("a");
        std::fs::write(workspace.join("global/inbox/b.md"), "# Beta\n\nsegunda nota\n")
            .expect("b");

        let db_path = base.join("data.db");
        let conn = crate::db::open(&db_path).expect("db");
        assert_eq!(reindex_with_linked(&conn, &workspace, &[]).expect("first"), 2);

        // Edita uma nota e apaga a outra.
        std::thread::sleep(std::time::Duration::from_millis(25));
        std::fs::write(workspace.join("global/inbox/a.md"), "# Alpha\n\nconteudo novo\n")
            .expect("rewrite");
        std::fs::remove_file(workspace.join("global/inbox/b.md")).expect("remove");

        assert_eq!(reindex_with_linked(&conn, &workspace, &[]).expect("second"), 1);

        let files = crate::db::memory::list_files(&conn, Some("global")).expect("files");
        assert_eq!(files.len(), 1);
        assert!(files[0].name.starts_with("a.md"));

        // O FTS acompanha a escrita incremental via triggers.
        let hits = crate::db::memory::search(&conn, None, "novo", &[], 10).expect("search");
        assert_eq!(hits.len(), 1);
        let old = crate::db::memory::search(&conn, None, "antigo", &[], 10).expect("search old");
        assert!(old.is_empty(), "conteúdo antigo não pode continuar indexado");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn preserves_unknown_frontmatter_keys() {
        let base = std::env::temp_dir().join(format!(
            "betastart-frontmatter-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("dir");
        let path = base.join("nota.md");
        std::fs::write(
            &path,
            "---\ntype: note\ntitle: Antiga\ncustom: manter\ntags:\n  - a\n---\n\ncorpo\n",
        )
        .expect("write");

        let (mut frontmatter, body) = read_note_file(&path).expect("read");
        frontmatter.title = Some("Nova".into());
        frontmatter.tags.clear();
        write_note_file_preserving(&path, &frontmatter, &body).expect("write");

        let raw = std::fs::read_to_string(&path).expect("raw");
        assert!(raw.contains("custom: manter"));
        assert!(raw.contains("title: Nova"));
        assert!(!raw.contains("tags:"));
        assert!(raw.contains("corpo"));

        let _ = std::fs::remove_dir_all(&base);
    }
}
