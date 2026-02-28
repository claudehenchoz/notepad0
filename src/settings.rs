//! Persist and restore UI preferences in HKCU\Software\Notepad0.
//!
//! All values are HKCU — backed by the kernel registry hive already mapped
//! into the process address space.  Reading a handful of DWORDs + one short
//! string adds no perceptible latency to startup.

use std::ffi::c_void;
use std::ptr::null_mut;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_READ, REG_DWORD, REG_SZ, REG_VALUE_TYPE,
};

use crate::state::STATE;

/// Load persisted settings into `STATE`.  Call once at the very start of
/// `main()`, before the first window is created, so that `WM_CREATE` and
/// `apply_initial_font` already see the correct values.
pub fn load() {
    let mut hkey = HKEY(null_mut::<c_void>());
    let ret = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Notepad0"),
            0,
            KEY_READ,
            &mut hkey,
        )
    };
    if ret != ERROR_SUCCESS {
        return; // key doesn't exist yet — defaults are fine
    }

    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if let Some(v) = read_dword(hkey, w!("WordWrap"))    { st.word_wrap_enabled        = v != 0;        }
        if let Some(v) = read_dword(hkey, w!("StatusBar"))   { st.status_bar_visible       = v != 0;        }
        if let Some(v) = read_dword(hkey, w!("FontSize"))    { st.current_font.height_pts  = v as i32;       }
        if let Some(v) = read_dword(hkey, w!("FontWeight"))  { st.current_font.weight      = v as i32;       }
        if let Some(v) = read_dword(hkey, w!("FontItalic"))  { st.current_font.italic      = v != 0;        }
        if let Some(v) = read_dword(hkey, w!("FontCharSet")) { st.current_font.charset     = v as u8;        }
        if let Some(face) = read_sz(hkey, w!("FontFace")) {
            if !face.is_empty() {
                st.current_font.face_name = face;
            }
        }
    });

    unsafe { let _ = RegCloseKey(hkey); }
}

/// Save current `STATE` settings to the registry.  Call from `WM_DESTROY`.
pub fn save() {
    let (word_wrap, status_bar_vis, face, size, weight, italic, charset) =
        STATE.with(|s| {
            let st = s.borrow();
            (
                st.word_wrap_enabled,
                st.status_bar_visible,
                st.current_font.face_name.clone(),
                st.current_font.height_pts,
                st.current_font.weight,
                st.current_font.italic,
                st.current_font.charset,
            )
        });

    let mut hkey = HKEY(null_mut::<c_void>());
    let ret = unsafe {
        RegCreateKeyW(HKEY_CURRENT_USER, w!("Software\\Notepad0"), &mut hkey)
    };
    if ret != ERROR_SUCCESS {
        return;
    }

    write_dword(hkey, w!("WordWrap"),    word_wrap as u32);
    write_dword(hkey, w!("StatusBar"),   status_bar_vis as u32);
    write_dword(hkey, w!("FontSize"),    size as u32);
    write_dword(hkey, w!("FontWeight"),  weight as u32);
    write_dword(hkey, w!("FontItalic"),  italic as u32);
    write_dword(hkey, w!("FontCharSet"), charset as u32);
    write_sz   (hkey, w!("FontFace"),    &face);

    unsafe { let _ = RegCloseKey(hkey); }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn read_dword(hkey: HKEY, name: PCWSTR) -> Option<u32> {
    let mut val  = 0u32;
    let mut size = 4u32;
    let mut typ  = REG_VALUE_TYPE(0);
    let ret = unsafe {
        RegQueryValueExW(
            hkey,
            name,
            None,
            Some(&mut typ),
            Some(&mut val as *mut u32 as *mut u8),
            Some(&mut size),
        )
    };
    if ret == ERROR_SUCCESS && typ == REG_DWORD { Some(val) } else { None }
}

fn read_sz(hkey: HKEY, name: PCWSTR) -> Option<String> {
    // First call: query byte size of the stored string.
    let mut size = 0u32;
    unsafe { RegQueryValueExW(hkey, name, None, None, None, Some(&mut size)); }
    if size < 2 {
        return None;
    }

    let cap = (size as usize / 2) + 1; // size is in bytes; we need u16 elements
    let mut buf = vec![0u16; cap];
    let mut typ = REG_VALUE_TYPE(0);
    let ret = unsafe {
        RegQueryValueExW(
            hkey,
            name,
            None,
            Some(&mut typ),
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut size),
        )
    };
    if ret == ERROR_SUCCESS && typ == REG_SZ {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]))
    } else {
        None
    }
}

fn write_dword(hkey: HKEY, name: PCWSTR, val: u32) {
    let bytes = val.to_ne_bytes();
    unsafe {
        RegSetValueExW(hkey, name, 0, REG_DWORD, Some(&bytes));
    }
}

fn write_sz(hkey: HKEY, name: PCWSTR, val: &str) {
    let mut wide: Vec<u16> = val.encode_utf16().collect();
    wide.push(0); // null terminator
    let bytes = unsafe {
        std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2)
    };
    unsafe {
        RegSetValueExW(hkey, name, 0, REG_SZ, Some(bytes));
    }
}
