//! Watcher de memória: reindexa quando arquivos Markdown mudam por fora do app.
//!
//! Cobre edições externas (VS Code, Git, IA via MCP) no workspace e nos projetos
//! vinculados, emitindo `memory-changed` para a UI recarregar.
//!
//! O conjunto de pastas é reconciliado periodicamente com o `config.json`, então
//! vincular/desvincular um projeto não exige reiniciar o app.

use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use notify::{EventKind, RecursiveMode, Watcher};
use tauri::{Emitter, Manager};

use crate::core::config::Config;
use crate::memory::LinkedRoot;

pub fn start(
    app: tauri::AppHandle,
    db_path: PathBuf,
    initial_workspace: PathBuf,
    config_path: PathBuf,
) {
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(move |result| {
            let _ = tx.send(result);
        }) {
            Ok(watcher) => watcher,
            Err(error) => {
                eprintln!("[betastart] watcher de memória indisponível: {error}");
                return;
            }
        };

        let mut watched: Vec<PathBuf> = Vec::new();
        let (mut workspace, mut linked) = scope(&config_path)
            .unwrap_or_else(|| (initial_workspace.clone(), Vec::new()));
        sync_watches(&mut watcher, &mut watched, &workspace, &linked);

        loop {
            match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(Ok(event)) => {
                    if !is_relevant(&event) {
                        continue;
                    }
                    // Debounce: absorve a rajada de eventos de uma edição/rename.
                    while rx.recv_timeout(Duration::from_millis(700)).is_ok() {}

                    let config_changed = reload_config(&app, &config_path);
                    if let Some((next_workspace, next_linked)) = scope(&config_path) {
                        sync_watches(
                            &mut watcher,
                            &mut watched,
                            &next_workspace,
                            &next_linked,
                        );
                        workspace = next_workspace;
                        linked = next_linked;
                    }
                    reindex(&db_path, &workspace, &linked);
                    let _ = app.emit("memory-changed", ());
                    if config_changed {
                        let _ = app.emit("config-changed", ());
                    }
                }
                Ok(Err(_)) => {}
                Err(RecvTimeoutError::Timeout) => {
                    // O config.json pode ter sido alterado por fora (ex.: MCP
                    // vinculando um projeto). Recarrega no estado do app para a
                    // UI não continuar vendo a configuração antiga.
                    let config_changed = reload_config(&app, &config_path);
                    if let Some((next_workspace, next_linked)) = scope(&config_path) {
                        let changed = sync_watches(
                            &mut watcher,
                            &mut watched,
                            &next_workspace,
                            &next_linked,
                        );
                        workspace = next_workspace;
                        linked = next_linked;
                        if changed {
                            reindex(&db_path, &workspace, &linked);
                        }
                        if changed || config_changed {
                            let _ = app.emit("memory-changed", ());
                        }
                    }
                    if config_changed {
                        let _ = app.emit("config-changed", ());
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

/// Recarrega o `config.json` no estado compartilhado do app.
/// Retorna `true` quando a configuração em memória estava desatualizada.
///
/// Segura o lock antes de ler o disco para não sobrescrever uma alteração que
/// um comando Tauri acabou de gravar.
fn reload_config(app: &tauri::AppHandle, config_path: &Path) -> bool {
    let state = app.state::<crate::state::AppState>();
    let Ok(mut current) = state.config.lock() else {
        return false;
    };
    let Ok(disk) = Config::load_or_default(config_path) else {
        return false;
    };
    let changed = serde_json::to_value(&*current).ok() != serde_json::to_value(&disk).ok();
    if changed {
        *current = disk;
    }
    changed
}

fn scope(config_path: &Path) -> Option<(PathBuf, Vec<LinkedRoot>)> {
    let config = Config::load_or_default(config_path).ok()?;
    Some((
        crate::core::paths::expand(&config.workspace.path),
        LinkedRoot::from_config(&config.workspace),
    ))
}

fn reindex(db_path: &Path, workspace: &Path, linked: &[LinkedRoot]) {
    if let Ok(conn) = crate::db::open(db_path) {
        let _ = crate::memory::reindex_with_linked(&conn, workspace, linked);
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    normalize(left) == normalize(right)
}

fn normalize(path: &Path) -> String {
    let canonical = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    canonical
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase()
}

/// Retorna `true` se o conjunto observado mudou.
fn sync_watches(
    watcher: &mut notify::RecommendedWatcher,
    watched: &mut Vec<PathBuf>,
    workspace: &Path,
    linked: &[LinkedRoot],
) -> bool {
    let mut desired = vec![workspace.to_path_buf()];
    desired.extend(
        linked
            .iter()
            .filter(|item| item.root.exists())
            .map(|item| item.root.clone()),
    );

    let mut changed = false;
    watched.retain(|current| {
        let keep = desired.iter().any(|path| same_path(path, current));
        if !keep {
            let _ = watcher.unwatch(current);
            changed = true;
        }
        keep
    });

    for path in desired {
        if watched.iter().any(|current| same_path(current, &path)) {
            continue;
        }
        if watcher.watch(&path, RecursiveMode::Recursive).is_ok() {
            watched.push(path);
            changed = true;
        }
    }
    changed
}

fn is_relevant(event: &notify::Event) -> bool {
    let paths: Vec<&Path> = event
        .paths
        .iter()
        .map(PathBuf::as_path)
        .filter(|path| !crate::memory::is_ignored_path(path))
        .collect();
    if paths.is_empty() {
        return false;
    }

    let touches_ignore_files = paths.iter().any(|path| {
        path.file_name()
            .map(|name| {
                let name = name.to_string_lossy();
                name == ".gitignore" || name == ".betastartignore"
            })
            .unwrap_or(false)
    });
    if touches_ignore_files {
        return true;
    }

    let touches_markdown = paths.iter().any(|path| {
        path.extension()
            .map(|extension| extension.eq_ignore_ascii_case("md"))
            .unwrap_or(false)
            || path
                .components()
                .any(|component| component.as_os_str() == ".betastart")
    });
    if touches_markdown {
        return true;
    }

    let structural = matches!(
        event.kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(notify::event::ModifyKind::Name(_))
    );
    structural
        && paths.iter().any(|path| {
            path.extension().is_none()
                || path
                    .components()
                    .any(|component| component.as_os_str() == ".betastart")
        })
}
