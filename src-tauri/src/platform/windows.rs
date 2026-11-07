use crate::platform::{Rect, WindowInfo};

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
                // TODO(platform): resolver nome do processo via GetWindowThreadProcessId.
                process: String::new(),
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

#[cfg(not(target_os = "windows"))]
pub fn list_open_windows() -> Vec<WindowInfo> {
    Vec::new()
}
