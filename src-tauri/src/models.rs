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
    pub links: Vec<String>,
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
pub struct LinkInfo {
    pub label: String,
    /// Caminho do alvo, ou `None` se o destino não foi encontrado.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDetail {
    pub path: String,
    pub kind: String,
    pub id: Option<String>,
    pub title: String,
    pub tags: Vec<String>,
    pub links: Vec<LinkInfo>,
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
    /// Pasta raiz do projeto (workspace gerenciado ou pasta vinculada).
    pub root_path: Option<String>,
    /// `managed` (workspace do Beta Start) ou `linked` (pasta existente).
    pub mode: String,
    /// `false` quando uma pasta vinculada não está mais disponível.
    pub available: bool,
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

/// Visão compacta de uma task nas respostas MCP (menos tokens).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_iso: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remind_iso: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub project: String,
}

impl TaskSummary {
    pub fn from_task(task: &Task) -> Self {
        Self {
            id: task.id.clone(),
            title: task.title.clone(),
            status: task.status.clone(),
            priority: task.priority.clone(),
            due_iso: task.due_iso.clone(),
            // Lembrete igual ao vencimento é redundante na visão compacta.
            remind_iso: task
                .remind_iso
                .clone()
                .filter(|remind| task.due_iso.as_deref() != Some(remind.as_str())),
            parent_id: task.parent_id.clone(),
            tags: task.tags.clone(),
            project: task.project.clone(),
        }
    }
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub project: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClient {
    pub id: i64,
    pub name: String,
    pub scopes: Vec<String>,
    pub created_at: i64,
    pub last_seen: Option<i64>,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClientCreated {
    pub token: String,
    pub client: McpClient,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
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
