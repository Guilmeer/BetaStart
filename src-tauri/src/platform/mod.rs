//! Abstração de plataforma. Hoje só Windows; Linux/macOS entram depois
//! alterando apenas este módulo.

pub mod apps;
pub mod windows;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub name: String,
    /// Para atalhos do Menu Iniciar, este é o caminho do `.lnk`.
    pub exe_path: String,
    #[serde(default)]
    pub icon_path: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub handle: u64,
    pub process: String,
    pub title: String,
    pub class: Option<String>,
    pub monitor: i32,
    pub rect: Rect,
}

/// Abre um aplicativo/arquivo com o handler padrão do sistema.
#[cfg(target_os = "windows")]
pub fn launch(path: &str) -> Result<(), crate::error::AppError> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;

    use winapi::um::shellapi::ShellExecuteW;
    use winapi::um::winuser::SW_SHOWNORMAL;

    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let operation = wide("open");
    let file = wide(path);

    unsafe {
        ShellExecuteW(
            null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn launch(path: &str) -> Result<(), crate::error::AppError> {
    std::process::Command::new("xdg-open").arg(path).spawn()?;
    Ok(())
}
