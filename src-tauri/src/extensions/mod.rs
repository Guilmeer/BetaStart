use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub mod api;
pub mod runtime;

pub const ALLOWED_PERMISSIONS: &[&str] = &[
    "fs:read",
    "fs:write",
    "network",
    "shell",
    "clipboard:read",
    "clipboard:write",
    "memory",
    "tasks",
    "windows",
    "search",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionManifest {
    #[serde(default = "default_schema")]
    pub schema: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub entry: String,
    #[serde(default)]
    pub commands: Vec<ManifestCommand>,
    #[serde(default)]
    pub search_providers: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

fn default_schema() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestCommand {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default = "default_command_mode")]
    pub mode: String,
    #[serde(default)]
    pub icon: Option<String>,
}

fn default_command_mode() -> String {
    "no-view".into()
}

/// Lê e valida o `extension.json` de uma pasta.
pub fn read_manifest(dir: &Path) -> Result<ExtensionManifest, String> {
    let path = dir.join("extension.json");
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("não foi possível ler {}: {e}", path.display()))?;
    let manifest: ExtensionManifest =
        serde_json::from_str(&raw).map_err(|e| format!("extension.json inválido: {e}"))?;
    validate(&manifest)?;
    Ok(manifest)
}

fn validate(manifest: &ExtensionManifest) -> Result<(), String> {
    if manifest.id.trim().is_empty() {
        return Err("id vazio".into());
    }
    if !manifest.id.contains('/') {
        return Err("id deve seguir o formato 'namespace/nome'".into());
    }
    if !manifest
        .id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c))
    {
        return Err("id contém caracteres inválidos".into());
    }
    if manifest.name.trim().is_empty() {
        return Err("name vazio".into());
    }
    if manifest.entry.trim().is_empty() {
        return Err("entry vazio".into());
    }
    if manifest.entry.starts_with('/') || manifest.entry.contains("..") {
        return Err("entry inválido".into());
    }
    if manifest.commands.is_empty() {
        return Err("nenhum comando declarado".into());
    }
    for command in &manifest.commands {
        if command.id.trim().is_empty() || command.title.trim().is_empty() {
            return Err("comando com id/title vazio".into());
        }
    }
    for permission in &manifest.permissions {
        if !ALLOWED_PERMISSIONS.contains(&permission.as_str()) {
            return Err(format!("permissão desconhecida: {permission}"));
        }
    }
    Ok(())
}

/// Nome de pasta seguro para o id (`namespace/nome` -> `namespace_nome`).
pub fn folder_name(id: &str) -> String {
    id.chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect()
}

/// Instala copiando uma pasta local para a pasta de extensões.
pub fn install_from_dir(
    extensions_root: &Path,
    source: &Path,
) -> Result<ExtensionManifest, String> {
    if !source.is_dir() {
        return Err(format!("pasta inválida: {}", source.display()));
    }
    let manifest = read_manifest(source)?;
    let destination = extensions_root.join(folder_name(&manifest.id));

    // Evita copiar a pasta de instalação para dentro dela mesma (recursão infinita).
    if let Ok(source_canonical) = dunce::canonicalize(source) {
        let destination_canonical =
            dunce::canonicalize(&destination).unwrap_or_else(|_| destination.clone());
        if source_canonical == destination_canonical
            || source_canonical.starts_with(&destination_canonical)
            || destination_canonical.starts_with(&source_canonical)
        {
            return Err("a pasta de origem e o destino da instalação se sobrepõem".into());
        }
    }

    if destination.exists() {
        std::fs::remove_dir_all(&destination).map_err(|e| e.to_string())?;
    }
    copy_dir(source, &destination)?;
    Ok(manifest)
}

/// Instala clonando um repositório Git (clone raso) e copiando o conteúdo.
pub fn install_from_git(extensions_root: &Path, url: &str) -> Result<ExtensionManifest, String> {
    let temp = std::env::temp_dir().join(format!(
        "betastart-git-{}-{}",
        std::process::id(),
        crate::db::now_secs()
    ));
    let _ = std::fs::remove_dir_all(&temp);

    let output = crate::platform::hidden_command("git")
        .args(["clone", "--depth", "1", "--"])
        .arg(url)
        .arg(&temp)
        // Nunca deixa o Git abrir um prompt invisível esperando credenciais.
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("git indisponível: {e}"))?;

    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&temp);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        return Err(if detail.is_empty() {
            "git clone falhou".into()
        } else {
            format!("git clone falhou: {detail}")
        });
    }

    let result = install_from_dir(extensions_root, &temp);
    let _ = std::fs::remove_dir_all(&temp);
    result
}

pub fn uninstall(extensions_root: &Path, id: &str) -> Result<(), String> {
    let destination = extensions_root.join(folder_name(id));
    if destination.exists() {
        std::fs::remove_dir_all(&destination).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(from)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let name = entry.file_name();
        if name.to_string_lossy() == ".git" {
            continue;
        }
        let source = entry.path();
        let destination = to.join(&name);
        if source.is_dir() {
            copy_dir(&source, &destination)?;
        } else {
            std::fs::copy(&source, &destination).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Resolve o caminho do entrypoint, garantindo que fique dentro da extensão.
pub fn entry_path(install_path: &Path, manifest: &ExtensionManifest) -> Result<PathBuf, String> {
    let path = install_path.join(&manifest.entry);
    if !path.exists() {
        return Err(format!("entry não encontrado: {}", path.display()));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_manifest() -> ExtensionManifest {
        ExtensionManifest {
            schema: 1,
            id: "autor/ext".into(),
            name: "Ext".into(),
            version: "1.0.0".into(),
            description: String::new(),
            author: String::new(),
            icon: None,
            entry: "main.ts".into(),
            commands: vec![ManifestCommand {
                id: "cmd".into(),
                title: "Cmd".into(),
                subtitle: None,
                keywords: vec![],
                mode: "no-view".into(),
                icon: None,
            }],
            search_providers: vec![],
            permissions: vec![],
        }
    }

    #[test]
    fn validates_good_manifest() {
        assert!(validate(&base_manifest()).is_ok());
    }

    #[test]
    fn rejects_unknown_permission() {
        let mut manifest = base_manifest();
        manifest.permissions = vec!["root".into()];
        assert!(validate(&manifest).is_err());
    }

    #[test]
    fn rejects_bad_id() {
        let mut manifest = base_manifest();
        manifest.id = "semnamespace".into();
        assert!(validate(&manifest).is_err());
    }

    #[test]
    fn folder_name_is_flat() {
        assert_eq!(folder_name("autor/ext"), "autor_ext");
        assert!(!folder_name("a/../../b").contains('/'));
    }
}
