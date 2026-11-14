use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppResult;

fn default_version() -> u32 {
    1
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_max_results() -> usize {
    8
}

fn default_http_port() -> u16 {
    19_000
}

fn default_clipboard_max() -> i64 {
    500
}

fn default_launcher_hotkey() -> String {
    "Alt+Space".into()
}

fn default_start_menu_mode() -> String {
    "ask".into()
}

fn default_start_menu_fallback() -> String {
    "Win+Shift".into()
}

fn default_host() -> String {
    "127.0.0.1".into()
}

fn default_theme() -> String {
    "auto".into()
}

fn default_store_url() -> String {
    "https://raw.githubusercontent.com/betastart-org/extensions/main/index.json".into()
}

fn default_workspace_path() -> String {
    let base = dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join("BetaStart").to_string_lossy().to_string()
}

/// Pastas indexadas por padrão quando o usuário não define nenhuma.
pub fn default_index_dirs() -> Vec<String> {
    let mut dirs = Vec::new();
    if let Some(dir) = dirs::document_dir() {
        dirs.push(dir.to_string_lossy().to_string());
    }
    if let Some(dir) = dirs::desktop_dir() {
        dirs.push(dir.to_string_lossy().to_string());
    }
    if let Some(dir) = dirs::download_dir() {
        dirs.push(dir.to_string_lossy().to_string());
    }
    dirs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_false")]
    pub onboarding_done: bool,
    #[serde(default)]
    pub start_menu: StartMenuConfig,
    #[serde(default)]
    pub hotkeys: HotkeysConfig,
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub clipboard: ClipboardConfig,
    #[serde(default)]
    pub windows: WindowsConfig,
    #[serde(default)]
    pub tray: TrayConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub extensions: ExtensionsConfig,
    #[serde(default)]
    pub appearance: AppearanceConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: default_version(),
            onboarding_done: false,
            start_menu: StartMenuConfig::default(),
            hotkeys: HotkeysConfig::default(),
            workspace: WorkspaceConfig::default(),
            search: SearchConfig::default(),
            clipboard: ClipboardConfig::default(),
            windows: WindowsConfig::default(),
            tray: TrayConfig::default(),
            mcp: McpConfig::default(),
            extensions: ExtensionsConfig::default(),
            appearance: AppearanceConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartMenuConfig {
    /// `ask` | `replace` | `off`
    #[serde(default = "default_start_menu_mode")]
    pub mode: String,
    #[serde(default = "default_start_menu_fallback")]
    pub fallback: String,
}

impl Default for StartMenuConfig {
    fn default() -> Self {
        Self {
            mode: default_start_menu_mode(),
            fallback: default_start_menu_fallback(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeysConfig {
    #[serde(default = "default_launcher_hotkey")]
    pub launcher: String,
    #[serde(default)]
    pub window_layouts: BTreeMap<String, String>,
}

impl Default for HotkeysConfig {
    fn default() -> Self {
        Self {
            launcher: default_launcher_hotkey(),
            window_layouts: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    #[serde(default = "default_workspace_path")]
    pub path: String,
    #[serde(default)]
    pub active_project: Option<String>,
    /// Projetos vinculados a pastas existentes do usuário.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked_projects: Vec<LinkedProjectConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedProjectConfig {
    pub slug: String,
    pub root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            path: default_workspace_path(),
            active_project: None,
            linked_projects: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchConfig {
    #[serde(default = "default_true")]
    pub include_apps: bool,
    #[serde(default = "default_true")]
    pub include_files: bool,
    #[serde(default)]
    pub index_dirs: Vec<String>,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            include_apps: true,
            include_files: true,
            index_dirs: default_index_dirs(),
            exclude_patterns: vec!["node_modules".into(), ".git".into()],
            max_results: default_max_results(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_clipboard_max")]
    pub max_entries: i64,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: default_clipboard_max(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsConfig {
    /// Aplica regras por app automaticamente quando uma janela nova aparece.
    #[serde(default = "default_true")]
    pub auto_apply_rules: bool,
    /// Reaplica o último layout usado quando o app inicia.
    #[serde(default)]
    pub restore_last_layout: bool,
    /// Último layout aplicado (usado por `restore_last_layout`).
    #[serde(default)]
    pub last_layout_id: Option<i64>,
}

impl Default for WindowsConfig {
    fn default() -> Self {
        Self {
            auto_apply_rules: true,
            restore_last_layout: false,
            last_layout_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayConfig {
    #[serde(default = "default_true")]
    pub show_on_close: bool,
    #[serde(default = "default_false")]
    pub start_on_boot: bool,
    #[serde(default = "default_true")]
    pub hide_on_blur: bool,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            show_on_close: true,
            start_on_boot: false,
            hide_on_blur: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for ToggleConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpHttpConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_http_port")]
    pub port: u16,
}

impl Default for McpHttpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            host: default_host(),
            port: default_http_port(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub stdio: ToggleConfig,
    #[serde(default)]
    pub http: McpHttpConfig,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            stdio: ToggleConfig::default(),
            http: McpHttpConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionsConfig {
    /// URL (http/https) ou caminho local do `index.json` da loja.
    #[serde(default = "default_store_url")]
    pub store_url: String,
    /// Caminho explícito para o executável do Deno (opcional).
    #[serde(default)]
    pub deno_path: Option<String>,
}

impl Default for ExtensionsConfig {
    fn default() -> Self {
        Self {
            store_url: default_store_url(),
            deno_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
        }
    }
}

/// Grava o config via arquivo temporário + rename, para que outro processo
/// (ex.: o servidor MCP) nunca leia um `config.json` pela metade.
fn write_atomic(path: &Path, content: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "config.json".into());
    let tmp = path.with_file_name(format!("{name}.{}.tmp", std::process::id()));

    if std::fs::write(&tmp, content).is_ok() && std::fs::rename(&tmp, path).is_ok() {
        return Ok(());
    }
    let _ = std::fs::remove_file(&tmp);
    // Fallback (ex.: antivírus/OneDrive segurando o arquivo): grava direto.
    std::fs::write(path, content)?;
    Ok(())
}

impl Config {
    /// Carrega o `config.json` ou cria um com os padrões.
    /// Também aplica migrações de valores antigos (ex.: modo `win_x`).
    pub fn load_or_default(path: &Path) -> AppResult<Self> {
        if path.exists() {
            let raw = std::fs::read_to_string(path)?;
            let mut cfg: Config = serde_json::from_str(&raw)?;
            cfg.normalize();
            // Persiste defaults novos (ex.: seção `extensions`) e migrações.
            let serialized = serde_json::to_string_pretty(&cfg)?;
            if serialized != raw {
                write_atomic(path, &serialized)?;
            }
            Ok(cfg)
        } else {
            let mut cfg = Config::default();
            cfg.normalize();
            cfg.save(path)?;
            Ok(cfg)
        }
    }

    pub fn save(&self, path: &Path) -> AppResult<()> {
        let raw = serde_json::to_string_pretty(self)?;
        write_atomic(path, &raw)
    }

    /// Aplica um patch parcial (JSON) sobre a configuração atual.
    pub fn merged(&self, patch: Value) -> AppResult<Config> {
        let mut base = serde_json::to_value(self)?;
        deep_merge(&mut base, patch);
        let cfg: Config = serde_json::from_value(base)?;
        Ok(cfg)
    }

    /// Corrige valores obsoletos/ausentes. Retorna `true` se algo mudou.
    pub fn normalize(&mut self) -> bool {
        let mut changed = false;

        // `win_x` foi descontinuado: Win+X é reservado pelo Windows.
        if self.start_menu.mode == "win_x" {
            self.start_menu.mode = "off".into();
            changed = true;
        }
        // Se o onboarding já foi concluído e o modo ainda é "ask", assume "off".
        if self.start_menu.mode == "ask" && self.onboarding_done {
            self.start_menu.mode = "off".into();
            changed = true;
        }
        // Config antiga sem pastas de índice: preenche com os padrões.
        if self.search.include_files && self.search.index_dirs.is_empty() {
            self.search.index_dirs = default_index_dirs();
            changed = true;
        }

        changed
    }
}

/// Merge recursivo de objetos JSON.
pub fn deep_merge(base: &mut Value, patch: Value) {
    match patch {
        Value::Object(patch_map) => {
            if !base.is_object() {
                *base = Value::Object(serde_json::Map::new());
            }
            let base_map = base.as_object_mut().expect("base é objeto");
            for (key, value) in patch_map {
                match base_map.get_mut(&key) {
                    Some(existing) => deep_merge(existing, value),
                    None => {
                        base_map.insert(key, value);
                    }
                }
            }
        }
        other => *base = other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_config_atomically_and_reloads() {
        let base = std::env::temp_dir().join(format!(
            "betastart-config-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("dir");
        let path = base.join("config.json");

        let mut config = Config::default();
        config.workspace.active_project = Some("prisma-historico".into());
        config.save(&path).expect("save");

        let raw = std::fs::read_to_string(&path).expect("raw");
        assert!(raw.contains("prisma-historico"));

        let loaded = Config::load_or_default(&path).expect("load");
        assert_eq!(
            loaded.workspace.active_project.as_deref(),
            Some("prisma-historico")
        );

        let leftovers: Vec<_> = std::fs::read_dir(&base)
            .expect("dir")
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());

        let _ = std::fs::remove_dir_all(&base);
    }
}
