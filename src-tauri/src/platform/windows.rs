use crate::platform::{MonitorInfo, Rect, WindowInfo};

/// Enumera as janelas visíveis de nível superior (Windows).
#[cfg(target_os = "windows")]
pub fn list_open_windows() -> Vec<WindowInfo> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    use winapi::shared::minwindef::{BOOL, LPARAM, TRUE};
    use winapi::shared::windef::{HWND, RECT};
    use winapi::um::winuser::{
        EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
        IsWindowVisible,
    };

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let out = &mut *(lparam as *mut Vec<WindowInfo>);

            if IsWindowVisible(hwnd) == 0 {
                return TRUE;
            }

            let len = GetWindowTextLengthW(hwnd);
            if len <= 0 {
                return TRUE;
            }

            let mut buf: Vec<u16> = vec![0; (len + 1) as usize];
            let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            if n <= 0 {
                return TRUE;
            }
            let title = OsString::from_wide(&buf[..n as usize])
                .to_string_lossy()
                .to_string();

            let mut class_buf: [u16; 256] = [0; 256];
            let cn = GetClassNameW(hwnd, class_buf.as_mut_ptr(), class_buf.len() as i32);
            let class = if cn > 0 {
                Some(
                    OsString::from_wide(&class_buf[..cn as usize])
                        .to_string_lossy()
                        .to_string(),
                )
            } else {
                None
            };

            let mut rect: RECT = std::mem::zeroed();
            GetWindowRect(hwnd, &mut rect);

            out.push(WindowInfo {
                handle: hwnd as usize as u64,
                process: process_name(hwnd),
                title,
                class,
                monitor: 0,
                rect: Rect {
                    x: rect.left as i64,
                    y: rect.top as i64,
                    w: (rect.right - rect.left) as i64,
                    h: (rect.bottom - rect.top) as i64,
                },
            });
            TRUE
        }
    }

    let mut out: Vec<WindowInfo> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(enum_proc), &mut out as *mut _ as LPARAM);
    }
    out
}

#[cfg(target_os = "windows")]
fn process_name(hwnd: winapi::shared::windef::HWND) -> String {
    use std::path::Path;

    use winapi::shared::minwindef::DWORD;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winbase::QueryFullProcessImageNameW;
    use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;
    use winapi::um::winuser::GetWindowThreadProcessId;

    unsafe {
        let mut pid: DWORD = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return String::new();
        }

        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return String::new();
        }

        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as DWORD;
        let ok = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size);
        let _ = CloseHandle(process);
        if ok == 0 {
            return String::new();
        }

        let full = String::from_utf16_lossy(&buffer[..size as usize]);
        Path::new(&full)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or(full)
    }
}

/// Lista os monitores com suas áreas úteis (excluindo a barra de tarefas).
#[cfg(target_os = "windows")]
pub fn list_monitors() -> Vec<MonitorInfo> {
    use winapi::shared::minwindef::{BOOL, DWORD, LPARAM, TRUE};
    use winapi::shared::windef::{HDC, HMONITOR, LPRECT};
    use winapi::um::winuser::{
        EnumDisplayMonitors, GetMonitorInfoW, MONITORINFO, MONITORINFOEXW, MONITORINFOF_PRIMARY,
    };

    unsafe extern "system" fn enum_proc(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: LPRECT,
        lparam: LPARAM,
    ) -> BOOL {
        unsafe {
            let out = &mut *(lparam as *mut Vec<MonitorInfo>);
            let mut info: MONITORINFOEXW = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFOEXW>() as DWORD;

            let info_ptr = &mut info as *mut MONITORINFOEXW as *mut MONITORINFO;
            if GetMonitorInfoW(hmon, info_ptr) != 0 {
                let name = String::from_utf16_lossy(&info.szDevice)
                    .trim_end_matches('\0')
                    .to_string();
                out.push(MonitorInfo {
                    index: out.len() as i32,
                    name,
                    x: info.rcMonitor.left as i64,
                    y: info.rcMonitor.top as i64,
                    width: (info.rcMonitor.right - info.rcMonitor.left) as i64,
                    height: (info.rcMonitor.bottom - info.rcMonitor.top) as i64,
                    work_x: info.rcWork.left as i64,
                    work_y: info.rcWork.top as i64,
                    work_width: (info.rcWork.right - info.rcWork.left) as i64,
                    work_height: (info.rcWork.bottom - info.rcWork.top) as i64,
                    primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
                });
            }
        }
        TRUE
    }

    let mut out: Vec<MonitorInfo> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            Some(enum_proc),
            &mut out as *mut _ as LPARAM,
        );
    }
    out
}

/// Move e redimensiona uma janela (restaurando se estiver maximizada).
#[cfg(target_os = "windows")]
pub fn move_window_rect(
    handle: u64,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
) -> Result<(), String> {
    use winapi::shared::windef::HWND;
    use winapi::um::winuser::{
        IsZoomed, SW_RESTORE, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, ShowWindow,
    };

    let hwnd = handle as usize as HWND;
    unsafe {
        if IsZoomed(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        let ok = SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            left,
            top,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        if ok == 0 {
            return Err(format!("SetWindowPos falhou para handle {handle}"));
        }
    }
    Ok(())
}

#[allow(dead_code)]
#[cfg(target_os = "windows")]
pub fn focus_window(handle: u64) {
    use winapi::shared::windef::HWND;
    use winapi::um::winuser::SetForegroundWindow;

    let hwnd = handle as usize as HWND;
    unsafe {
        SetForegroundWindow(hwnd);
    }
}

#[cfg(target_os = "windows")]
pub fn foreground_window() -> Option<u64> {
    use winapi::um::winuser::GetForegroundWindow;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            None
        } else {
            Some(hwnd as usize as u64)
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn list_open_windows() -> Vec<WindowInfo> {
    Vec::new()
}

#[cfg(not(target_os = "windows"))]
pub fn list_monitors() -> Vec<MonitorInfo> {
    Vec::new()
}

#[cfg(not(target_os = "windows"))]
pub fn move_window_rect(
    _handle: u64,
    _left: i32,
    _top: i32,
    _width: i32,
    _height: i32,
) -> Result<(), String> {
    Err("gerenciador de janelas disponível apenas no Windows".into())
}

#[cfg(not(target_os = "windows"))]
pub fn focus_window(_handle: u64) {}

#[cfg(not(target_os = "windows"))]
pub fn foreground_window() -> Option<u64> {
    None
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn enumerates_monitors() {
        let monitors = list_monitors();
        assert!(!monitors.is_empty(), "deve existir ao menos um monitor");
        assert!(
            monitors.iter().any(|monitor| monitor.work_width > 0),
            "work area deve ser positiva"
        );
    }

    #[test]
    fn enumerates_windows_without_panicking() {
        let windows = list_open_windows();
        for window in windows {
            assert_ne!(window.handle, 0);
        }
    }
}
