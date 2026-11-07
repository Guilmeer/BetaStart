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

fn default_workspace_path() -> String {
    let base = dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join("BetaStart").to_string_lossy().to_string()
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
    pub tray: TrayConfig,
    #[serde(default)]
    pub mcp: McpConfig,
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
            tray: TrayConfig::default(),
            mcp: McpConfig::default(),
            appearance: AppearanceConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartMenuConfig {
    /// `ask` | `replace` | `win_x`
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
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            path: default_workspace_path(),
            active_project: None,
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
            index_dirs: Vec::new(),
            exclude_patterns: vec!["node_modules".into(), ".git".into()],
            max_results: default_max_results(),
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

impl Config {
    /// Carrega o `config.json` ou cria um com os padrões.
    pub fn load_or_default(path: &Path) -> AppResult<Self> {
        if path.exists() {
            let raw = std::fs::read_to_string(path)?;
            let cfg: Config = serde_json::from_str(&raw)?;
            Ok(cfg)
        } else {
            let cfg = Config::default();
            cfg.save(path)?;
            Ok(cfg)
        }
    }

    pub fn save(&self, path: &Path) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(path, raw)?;
        Ok(())
    }

    /// Aplica um patch parcial (JSON) sobre a configuração atual.
    pub fn merged(&self, patch: Value) -> AppResult<Config> {
        let mut base = serde_json::to_value(self)?;
        deep_merge(&mut base, patch);
        let cfg: Config = serde_json::from_value(base)?;
        Ok(cfg)
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
