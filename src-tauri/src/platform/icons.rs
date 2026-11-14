//! Extração de ícones reais do Windows para os resultados do launcher.
//!
//! `SHGetFileInfoW` resolve o ícone de um `.lnk`/`.exe`; desenhamos num DIB
//! 32-bit e devolvemos um `data:image/png;base64,...` cacheado em memória.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

pub fn data_url(path: &str) -> Option<String> {
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }

    #[cfg(windows)]
    {
        let key = path.to_lowercase();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(cache) = cache.lock() {
            if let Some(value) = cache.get(&key) {
                return value.clone();
            }
        }
        let value = extract(path);
        if let Ok(mut cache) = cache.lock() {
            cache.insert(key, value.clone());
        }
        value
    }
}

#[cfg(windows)]
fn extract(path: &str) -> Option<String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;

    use base64::Engine as _;
    use winapi::shared::minwindef::{DWORD, UINT};
    use winapi::shared::windef::{HGDIOBJ, HICON};
    use winapi::um::shellapi::{SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGetFileInfoW};
    use winapi::um::wingdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, SelectObject,
    };
    use winapi::um::winuser::{DestroyIcon, DrawIconEx, GetIconInfo, ICONINFO};

    // `DI_NORMAL` (DI_IMAGE | DI_MASK) não existe no bindgen do winapi 0.3.9.
    const DI_NORMAL: UINT = 0x0003;

    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut file_info: SHFILEINFOW = unsafe { std::mem::zeroed() };
    let result = unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut file_info,
            std::mem::size_of::<SHFILEINFOW>() as UINT,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if result == 0 || file_info.hIcon.is_null() {
        return None;
    }
    let icon: HICON = file_info.hIcon;

    let mut icon_info: ICONINFO = unsafe { std::mem::zeroed() };
    if unsafe { GetIconInfo(icon, &mut icon_info) } == 0 {
        unsafe {
            DestroyIcon(icon);
        }
        return None;
    }

    let size = 32i32;
    let mut bmi: BITMAPINFO = unsafe { std::mem::zeroed() };
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as DWORD;
    bmi.bmiHeader.biWidth = size;
    bmi.bmiHeader.biHeight = -size; // top-down
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    let hdc = unsafe { CreateCompatibleDC(null_mut()) };
    if hdc.is_null() {
        unsafe {
            DestroyIcon(icon);
            if !icon_info.hbmColor.is_null() {
                DeleteObject(icon_info.hbmColor as HGDIOBJ);
            }
            if !icon_info.hbmMask.is_null() {
                DeleteObject(icon_info.hbmMask as HGDIOBJ);
            }
        }
        return None;
    }

    let mut bits: *mut winapi::ctypes::c_void = null_mut();
    let hbmp = unsafe { CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0) };
    if hbmp.is_null() || bits.is_null() {
        unsafe {
            DeleteDC(hdc);
            DestroyIcon(icon);
            if !icon_info.hbmColor.is_null() {
                DeleteObject(icon_info.hbmColor as HGDIOBJ);
            }
            if !icon_info.hbmMask.is_null() {
                DeleteObject(icon_info.hbmMask as HGDIOBJ);
            }
        }
        return None;
    }

    let old = unsafe { SelectObject(hdc, hbmp as HGDIOBJ) };
    let pixel_len = (size as usize) * (size as usize) * 4;
    unsafe {
        std::ptr::write_bytes(bits as *mut u8, 0, pixel_len);
    }
    let drawn = unsafe { DrawIconEx(hdc, 0, 0, icon, size, size, 0, null_mut(), DI_NORMAL) };

    let mut rgba = Vec::with_capacity(pixel_len);
    if drawn != 0 {
        let source = unsafe { std::slice::from_raw_parts(bits as *const u8, pixel_len) };
        let mut any_alpha = false;
        for pixel in source.chunks_exact(4) {
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            if pixel[3] != 0 {
                any_alpha = true;
            }
        }
        // Ícones antigos podem vir sem canal alpha; deixa-os opacos.
        if !any_alpha {
            for pixel in rgba.chunks_exact_mut(4) {
                pixel[3] = 255;
            }
        }
    }

    unsafe {
        SelectObject(hdc, old);
        DeleteObject(hbmp as HGDIOBJ);
        DeleteDC(hdc);
        DestroyIcon(icon);
        if !icon_info.hbmColor.is_null() {
            DeleteObject(icon_info.hbmColor as HGDIOBJ);
        }
        if !icon_info.hbmMask.is_null() {
            DeleteObject(icon_info.hbmMask as HGDIOBJ);
        }
    }

    if drawn == 0 {
        return None;
    }

    let image = image::RgbaImage::from_raw(size as u32, size as u32, rgba)?;
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn extracts_icon_from_windows_executable() {
        let path = r"C:\Windows\System32\notepad.exe";
        if !std::path::Path::new(path).exists() {
            return;
        }
        let data = data_url(path).expect("ícone do notepad");
        assert!(data.starts_with("data:image/png;base64,"));
        assert!(data.len() > 500);
        // Cache: a segunda chamada não deve mudar o valor.
        assert_eq!(Some(data), data_url(path));
    }
}
