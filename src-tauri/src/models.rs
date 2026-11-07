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
