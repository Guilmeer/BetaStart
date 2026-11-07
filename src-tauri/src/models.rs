//! DTOs compartilhados entre backend e frontend.

use serde::Serialize;

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
