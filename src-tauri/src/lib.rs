mod clipboard_watcher;
mod commands;
mod core;
mod db;
mod error;
mod extensions;
mod indexer;
mod models;
mod platform;
mod state;
mod window_manager;
mod window_watcher;

use tauri::Manager;
use tauri_plugin_global_shortcut::ShortcutState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // O single-instance precisa ser o primeiro plugin registrado.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            core::window::show(app);
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    let state = app.state::<state::AppState>();

                    let launcher = state
                        .config
                        .lock()
                        .map(|config| config.hotkeys.launcher.clone())
                        .unwrap_or_default();
                    if let Ok(parsed) = core::hotkey::parse_combo(&launcher) {
                        if parsed == *shortcut {
                            core::window::toggle(app);
                            return;
                        }
                    }

                    if let Ok(conn) = state.db.lock() {
                        if let Ok(layouts) = db::windows::list_layouts(&conn) {
                            for layout in layouts {
                                if let Some(combo) = &layout.hotkey {
                                    if let Ok(parsed) = core::hotkey::parse_combo(combo) {
                                        if parsed == *shortcut {
                                            let _ = window_manager::apply_layout_by_id(
                                                &conn, layout.id,
                                            );
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();

            let config_path = core::paths::config_path(&handle)?;
            let db_path = core::paths::db_path(&handle)?;
            let config = core::config::Config::load_or_default(&config_path)?;
            core::paths::ensure_workspace(&config.workspace.path)?;

            let conn = db::open(&db_path)?;
            // Indexação de apps é rápida e roda de forma síncrona.
            let _ = db::apps::refresh(&conn);
            let layout_hotkeys = db::windows::layout_hotkeys(&conn).unwrap_or_default();

            app.manage(state::AppState::new(config.clone(), conn));

            core::tray::setup(&handle)?;

            if let Err(err) = core::hotkey::sync(&handle, &config.hotkeys.launcher, &layout_hotkeys)
            {
                eprintln!("[betastart] falha ao registrar atalhos: {err}");
            }

            if config.start_menu.mode == "replace" {
                if let Err(err) = core::win_hook::install(handle.clone()) {
                    eprintln!("[betastart] falha ao instalar hook da tecla Win: {err}");
                }
            }

            // Indexação de arquivos em segundo plano.
            if config.search.include_files {
                let dirs = config.search.index_dirs.clone();
                let excludes = config.search.exclude_patterns.clone();
                let index_db = db_path.clone();
                std::thread::spawn(move || {
                    indexer::run_file_index(index_db, dirs, excludes);
                });
            }

            // Histórico de clipboard em segundo plano.
            if config.clipboard.enabled {
                clipboard_watcher::start(db_path.clone(), config.clipboard.max_entries);
            }

            // Regras por app aplicadas a janelas novas.
            if config.windows.auto_apply_rules {
                window_watcher::start(db_path.clone());
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::get_config,
            commands::get_paths,
            commands::set_config,
            commands::set_onboarding_done,
            commands::apply_launcher_settings,
            commands::validate_hotkey,
            commands::suspend_hotkeys,
            commands::resume_hotkeys,
            commands::search,
            commands::search_apps,
            commands::list_installed_apps,
            commands::launch_app,
            commands::open_path,
            commands::copy_text,
            commands::list_clipboard,
            commands::delete_clipboard,
            commands::pin_clipboard,
            commands::list_open_windows,
            commands::rebuild_apps_index,
            commands::rebuild_files_index,
            commands::list_extensions,
            commands::install_extension,
            commands::uninstall_extension,
            commands::set_extension_enabled,
            commands::set_extension_permissions,
            commands::run_extension_command,
            commands::runtime_status,
            commands::ensure_runtime,
            commands::list_store,
            commands::list_monitors,
            commands::list_windows_with_monitor,
            commands::list_window_layouts,
            commands::save_current_layout,
            commands::delete_window_layout,
            commands::apply_window_layout,
            commands::set_window_layout_hotkey,
            commands::list_window_rules,
            commands::upsert_window_rule,
            commands::delete_window_rule,
            commands::apply_window_rules,
            commands::get_status,
            commands::show_launcher,
            commands::hide_launcher,
            commands::toggle_launcher,
            commands::quit_app,
        ])
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                // Fechar a janela não encerra o app: vai para a bandeja.
                api.prevent_close();
                let _ = window.hide();
            }
            tauri::WindowEvent::Focused(false) => {
                let app = window.app_handle();
                let hide_on_blur = app
                    .state::<state::AppState>()
                    .config
                    .lock()
                    .map(|cfg| cfg.tray.hide_on_blur)
                    .unwrap_or(true);
                if hide_on_blur {
                    let _ = window.hide();
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
