use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

/// Cria o ícone da bandeja e seu menu (Abrir / Reindexar / Sair).
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Abrir Beta Start", true, None::<&str>)?;
    let rebuild = MenuItem::with_id(app, "rebuild", "Reindexar aplicativos", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &rebuild, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Beta Start")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => crate::core::window::show(app),
            "rebuild" => {
                if let Some(state) = app.try_state::<crate::state::AppState>() {
                    if let Ok(conn) = state.db.lock() {
                        let _ = crate::db::apps::refresh(&conn);
                    }
                }
            }
            "quit" => {
                if let Some(state) = app.try_state::<crate::state::AppState>() {
                    if let Ok(conn) = state.db.lock() {
                        crate::db::checkpoint(&conn);
                    }
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::core::window::show(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    let _tray = builder.build(app)?;
    Ok(())
}
