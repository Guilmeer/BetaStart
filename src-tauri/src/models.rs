//! DTOs compartilhados entre backend e frontend.

use serde::{Deserialize, Serialize};

use crate::extensions::ManifestCommand;
use crate::platform::AppEntry;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub ext: Option<String>,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipEntry {
    pub id: i64,
    pub kind: String,
    pub text: Option<String>,
    pub copied_at: i64,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEntry {
    pub ext_id: String,
    pub command_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub apps: Vec<AppEntry>,
    pub files: Vec<FileEntry>,
    pub clipboard: Vec<ClipEntry>,
    pub commands: Vec<CommandEntry>,
    pub calculation: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledExtension {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub install_source: String,
    pub install_path: String,
    pub enabled: bool,
    pub permissions: Vec<String>,
    pub granted: Vec<String>,
    pub pending: bool,
    pub commands: Vec<ManifestCommand>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    pub kind: String,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub available: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreItem {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub url: String,
    /// `git` ou `local`.
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String {
    "git".into()
}

// ---------------------------------------------------------------------------
// Memória & tasks
// ---------------------------------------------------------------------------

/// Linha interna do índice de memória.
pub struct MemoryRow {
    pub file_path: String,
    pub kind: String,
    pub title: String,
    pub project: String,
    pub tags: Vec<String>,
    pub content: String,
    pub updated_at: i64,
}

/// Linha interna do índice de tasks.
pub struct TaskRow {
    pub id: String,
    pub file_path: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub due: Option<i64>,
    pub remind: Option<i64>,
    pub tags: Vec<String>,
    pub project: String,
    pub parent_id: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryFile {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub title: String,
    pub project: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryHit {
    pub file_path: String,
    pub kind: String,
    pub title: String,
    pub project: String,
    pub tags: Vec<String>,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDetail {
    pub path: String,
    pub kind: String,
    pub id: Option<String>,
    pub title: String,
    pub tags: Vec<String>,
    pub body: String,
    pub project: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub due: Option<String>,
    pub remind: Option<String>,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub file_path: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub due: Option<i64>,
    pub due_iso: Option<String>,
    pub remind: Option<i64>,
    pub remind_iso: Option<String>,
    pub tags: Vec<String>,
    pub project: String,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTask {
    pub title: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub remind: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub remind: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
}
