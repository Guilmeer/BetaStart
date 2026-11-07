//! Watcher de janelas: aplica regras por app quando uma janela nova aparece.
//!
//! Polling simples (~1,2s). Só age quando existem regras habilitadas.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

pub fn start(db_path: PathBuf) {
    std::thread::spawn(move || {
        let conn = match crate::db::open(&db_path) {
            Ok(conn) => conn,
            Err(err) => {
                eprintln!("[betastart] window watcher: {err}");
                return;
            }
        };

        let mut seen: HashSet<u64> = HashSet::new();
        let mut had_rules = false;

        loop {
            if let Ok(rules) = crate::db::windows::list_rules(&conn) {
                if rules.is_empty() {
                    had_rules = false;
                } else {
                    // Primeira regra criada: reavalia janelas que já estavam abertas.
                    if !had_rules {
                        seen.clear();
                        had_rules = true;
                    }

                    let monitors = crate::window_manager::list_monitors();
                    let windows = crate::window_manager::snapshot_windows(&monitors);

                    for window in &windows {
                        if !seen.insert(window.handle) {
                            continue;
                        }
                        if window.process.to_lowercase().contains("betastart") {
                            continue;
                        }
                        if let Some(rule) = rules.iter().find(|rule| {
                            rule.enabled && crate::window_manager::matches_window(rule, window)
                        }) {
                            let _ = crate::window_manager::apply_rule(rule, window, &monitors);
                        }
                    }

                    let current: HashSet<u64> =
                        windows.iter().map(|window| window.handle).collect();
                    seen.retain(|handle| current.contains(handle));
                }
            }

            std::thread::sleep(Duration::from_millis(1200));
        }
    });
}
