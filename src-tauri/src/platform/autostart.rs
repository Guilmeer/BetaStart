//! Registro do Beta Start na inicialização do Windows.
//!
//! Usa `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, que não exige
//! privilégios de administrador e funciona no instalador por usuário.

use crate::error::AppResult;

#[cfg(windows)]
pub fn set(enabled: bool) -> AppResult<()> {
    use crate::error::AppError;

    let exe = std::env::current_exe().map_err(|e| AppError::Platform(e.to_string()))?;
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    let mut command = crate::platform::hidden_command("reg");
    if enabled {
        command.args([
            "add",
            key,
            "/v",
            "BetaStart",
            "/t",
            "REG_SZ",
            "/d",
            &format!("\"{}\"", exe.to_string_lossy()),
            "/f",
        ]);
    } else {
        command.args(["delete", key, "/v", "BetaStart", "/f"]);
    }

    let output = command
        .output()
        .map_err(|e| AppError::Platform(e.to_string()))?;
    if enabled && !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        return Err(AppError::Platform(if detail.is_empty() {
            "não foi possível registrar a inicialização automática".into()
        } else {
            format!("não foi possível registrar a inicialização automática: {detail}")
        }));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set(_enabled: bool) -> AppResult<()> {
    Ok(())
}
