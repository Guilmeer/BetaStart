use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

/// Normaliza o nome dos modificadores (o parser do Deno/global-hotkey usa "Super"
/// em vez de "Win") e converte para `Shortcut`.
pub fn parse_combo(combo: &str) -> Result<Shortcut, String> {
    let normalized = combo
        .split('+')
        .map(|part| {
            let trimmed = part.trim();
            match trimmed.to_ascii_lowercase().as_str() {
                "win" | "windows" | "meta" | "super" => "Super".to_string(),
                "ctrl" | "control" => "Control".to_string(),
                "alt" => "Alt".to_string(),
                "shift" => "Shift".to_string(),
                _ => trimmed.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("+");

    normalized
        .parse::<Shortcut>()
        .map_err(|e| format!("hotkey inválida '{combo}': {e}"))
}

/// Registra um atalho global.
pub fn register(app: &AppHandle, combo: &str) -> Result<(), String> {
    let shortcut = parse_combo(combo)?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| e.to_string())
}

/// Re-registra tudo: launcher + hotkeys de layouts.
pub fn sync(app: &AppHandle, launcher: &str, layout_hotkeys: &[String]) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())?;

    register(app, launcher)?;

    for combo in layout_hotkeys {
        // Atalhos duplicados/conflitantes não devem derrubar o app.
        let _ = register(app, combo);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_combo;

    #[test]
    fn parses_win_modifier() {
        assert!(parse_combo("Win+Shift+F7").is_ok());
        assert!(parse_combo("win+shift+f7").is_ok());
    }

    #[test]
    fn parses_alt_space() {
        assert!(parse_combo("Alt+Space").is_ok());
    }
}
