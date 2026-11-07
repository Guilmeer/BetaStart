use std::path::{Path, PathBuf};

use crate::platform::AppEntry;

/// Lista aplicativos instalados procurando atalhos `.lnk` do Menu Iniciar.
pub fn list_installed_apps() -> Vec<AppEntry> {
    let mut roots: Vec<PathBuf> = Vec::new();

    if let Ok(program_data) = std::env::var("ProgramData") {
        roots.push(
            PathBuf::from(program_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    if let Ok(app_data) = std::env::var("APPDATA") {
        roots.push(
            PathBuf::from(app_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }

    let mut apps = Vec::new();
    for root in roots {
        collect_shortcuts(&root, &mut apps);
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name) && a.exe_path == b.exe_path);
    apps
}

fn collect_shortcuts(dir: &Path, out: &mut Vec<AppEntry>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_shortcuts(&path, out);
        } else if path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("lnk"))
            .unwrap_or(false)
        {
            if let Some(stem) = path.file_stem() {
                let name = stem.to_string_lossy().to_string();
                // Ignora atalhos claramente não-aplicativos.
                if name.is_empty() {
                    continue;
                }
                out.push(AppEntry {
                    name,
                    exe_path: path.to_string_lossy().to_string(),
                    icon_path: None,
                    keywords: Vec::new(),
                });
            }
        }
    }
}
