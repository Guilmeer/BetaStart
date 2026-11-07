//! Interceptação da tecla Windows via low-level keyboard hook (WH_KEYBOARD_LL).
//!
//! Estratégia: a tecla Win "solta sozinha" é engolida no key-up, impedindo o
//! Menu Iniciar de abrir, e o Beta Start é alternado. Se outra tecla for
//! pressionada enquanto Win está down, deixamos o combo passar (Win+D, Win+E,
//! Win+L, etc. continuam funcionando).

#[allow(unused_imports)]
pub use imp::{install, is_installed, uninstall};

#[cfg(target_os = "windows")]
mod imp {
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};

    use tauri::AppHandle;
    use winapi::shared::minwindef::{LPARAM, LRESULT, WPARAM};
    use winapi::shared::windef::HHOOK;
    use winapi::um::libloaderapi::GetModuleHandleW;
    use winapi::um::winuser::{
        CallNextHookEx, KBDLLHOOKSTRUCT, SetWindowsHookExW, UnhookWindowsHookEx, VK_LWIN, VK_RWIN,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    static APP: OnceLock<AppHandle> = OnceLock::new();
    static HOOK: AtomicIsize = AtomicIsize::new(0);
    static WIN_DOWN: AtomicBool = AtomicBool::new(false);
    static WIN_COMBO: AtomicBool = AtomicBool::new(false);

    pub fn install(app: AppHandle) -> Result<(), String> {
        let _ = APP.set(app);
        if HOOK.load(Ordering::SeqCst) != 0 {
            return Ok(());
        }

        unsafe {
            let module = GetModuleHandleW(std::ptr::null());
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), module, 0);
            if hook.is_null() {
                return Err("SetWindowsHookExW falhou".into());
            }
            HOOK.store(hook as isize, Ordering::SeqCst);
        }
        Ok(())
    }

    pub fn uninstall() {
        let hook = HOOK.swap(0, Ordering::SeqCst);
        if hook != 0 {
            unsafe {
                UnhookWindowsHookEx(hook as HHOOK);
            }
        }
    }

    pub fn is_installed() -> bool {
        HOOK.load(Ordering::SeqCst) != 0
    }

    unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            if code < 0 {
                return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
            }

            let keyboard = &*(lparam as *const KBDLLHOOKSTRUCT);
            let vk = keyboard.vkCode;
            let is_win = vk == VK_LWIN as u32 || vk == VK_RWIN as u32;
            let message = wparam as u32;

            match message {
                WM_KEYDOWN | WM_SYSKEYDOWN => {
                    if is_win {
                        WIN_DOWN.store(true, Ordering::SeqCst);
                        WIN_COMBO.store(false, Ordering::SeqCst);
                    } else if WIN_DOWN.load(Ordering::SeqCst) {
                        // Alguma tecla junto com Win: deixa o combo do Windows passar.
                        WIN_COMBO.store(true, Ordering::SeqCst);
                    }
                }
                WM_KEYUP | WM_SYSKEYUP => {
                    if is_win {
                        let alone = WIN_DOWN.swap(false, Ordering::SeqCst)
                            && !WIN_COMBO.load(Ordering::SeqCst);
                        if alone {
                            WIN_COMBO.store(false, Ordering::SeqCst);
                            if let Some(app) = APP.get() {
                                let worker = app.clone();
                                let _ = app.run_on_main_thread(move || {
                                    crate::core::window::toggle(&worker);
                                });
                            }
                            // Engole o key-up: o Menu Iniciar não abre.
                            return 1;
                        }
                    } else if WIN_DOWN.load(Ordering::SeqCst) {
                        WIN_COMBO.store(true, Ordering::SeqCst);
                    }
                }
                _ => {}
            }

            CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use tauri::AppHandle;

    pub fn install(_app: AppHandle) -> Result<(), String> {
        Ok(())
    }

    pub fn uninstall() {}

    pub fn is_installed() -> bool {
        false
    }
}
