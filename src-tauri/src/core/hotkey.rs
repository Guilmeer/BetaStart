use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

/// Registra a hotkey global do launcher. Retorna uma mensagem de erro legível
/// (sem abortar o app) caso o atalho seja inválido ou já esteja em uso.
pub fn register(app: &AppHandle, combo: &str) -> Result<(), String> {
    let shortcut = combo
        .parse::<Shortcut>()
        .map_err(|e| format!("hotkey inválida '{combo}': {e}"))?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| e.to_string())
}
