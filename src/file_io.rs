use std::path::Path;
use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{HWND, MAX_PATH};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OPENFILENAMEW,
    OFN_FILEMUSTEXIST, OFN_HIDEREADONLY, OFN_OVERWRITEPROMPT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowTextLengthW, GetWindowTextW, MessageBoxW, SetWindowTextW,
    MB_ICONERROR, MB_OK,
};

use crate::state::STATE;
use crate::hwnd_edit;

fn detect_encoding(bytes: &[u8]) -> (&'static Encoding, usize) {
    if let Some((enc, bom_len)) = Encoding::for_bom(bytes) {
        return (enc, bom_len);
    }
    if std::str::from_utf8(bytes).is_ok() {
        return (UTF_8, 0);
    }
    (WINDOWS_1252, 0)
}

/// Read a file and load it into the EDIT control
pub fn load_file(hwnd: HWND, path: &str) -> bool {
    let raw = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            let msg: Vec<u16> = format!("Cannot open file:\n{}\0", e)
                .encode_utf16()
                .collect();
            unsafe {
                MessageBoxW(hwnd, PCWSTR(msg.as_ptr()), w!("Notepad Zero"), MB_ICONERROR | MB_OK);
            }
            return false;
        }
    };

    let (encoding, bom_len) = detect_encoding(&raw);
    let (decoded, _, _) = encoding.decode(&raw[bom_len..]);
    let text = decoded.into_owned();

    // Normalize line endings to CRLF for the EDIT control
    let normalized: String = if text.contains('\r') {
        text
    } else {
        text.replace('\n', "\r\n")
    };

    let mut wide: Vec<u16> = normalized.encode_utf16().collect();
    wide.push(0); // null terminator

    unsafe {
        use crate::hwnd_edit;
        let edit = hwnd_edit();

        STATE.with(|s| s.borrow_mut().suppress_dirty = true);
        SetWindowTextW(edit, PCWSTR(wide.as_ptr()));

        // Reset limit after setting text (EM_SETLIMITTEXT = 0x00C5)
        windows::Win32::UI::WindowsAndMessaging::SendMessageW(
            edit,
            0x00C5u32,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );

        STATE.with(|s| {
            let mut st = s.borrow_mut();
            st.suppress_dirty = false;
            st.is_dirty = false;
            st.current_file_path = Some(path.to_string());
        });
    }

    update_title(hwnd);
    true
}

/// Save current content. If save_as or no path, show dialog first.
pub fn save_file(hwnd: HWND, save_as: bool) -> bool {
    let path = STATE.with(|s| s.borrow().current_file_path.clone());

    let file_path = if save_as || path.is_none() {
        match show_save_dialog(hwnd) {
            Some(p) => p,
            None => return false,
        }
    } else {
        path.unwrap()
    };

    let text = get_edit_text();
    let bytes = text.as_bytes();

    if let Err(e) = std::fs::write(&file_path, bytes) {
        let msg: Vec<u16> = format!("Cannot save file:\n{}\0", e)
            .encode_utf16()
            .collect();
        unsafe {
            MessageBoxW(hwnd, PCWSTR(msg.as_ptr()), w!("Notepad Zero"), MB_ICONERROR | MB_OK);
        }
        return false;
    }

    STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.current_file_path = Some(file_path);
        st.is_dirty = false;
    });

    update_title(hwnd);
    true
}

/// Get all text from the EDIT control as UTF-8 string
pub fn get_edit_text() -> String {
    let edit = hwnd_edit();
    unsafe {
        let len = GetWindowTextLengthW(edit);
        if len == 0 {
            return String::new();
        }
        let mut buf: Vec<u16> = vec![0u16; (len + 1) as usize];
        GetWindowTextW(edit, &mut buf);
        buf.truncate(len as usize);
        String::from_utf16_lossy(&buf)
    }
}

pub fn open_file_dialog(hwnd: HWND) -> Option<String> {
    let mut buf = vec![0u16; MAX_PATH as usize];
    let filter: Vec<u16> =
        "Text Files (*.txt)\0*.txt\0All Files (*.*)\0*.*\0\0"
            .encode_utf16()
            .collect();

    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = hwnd;
    ofn.lpstrFilter = PCWSTR(filter.as_ptr());
    ofn.lpstrFile = PWSTR(buf.as_mut_ptr());
    ofn.nMaxFile = MAX_PATH;
    ofn.lpstrTitle = w!("Open");
    ofn.Flags = OFN_FILEMUSTEXIST | OFN_HIDEREADONLY;

    unsafe {
        if GetOpenFileNameW(&mut ofn).as_bool() {
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            Some(String::from_utf16_lossy(&buf[..end]))
        } else {
            None
        }
    }
}

pub fn show_save_dialog(hwnd: HWND) -> Option<String> {
    let mut buf = vec![0u16; MAX_PATH as usize];

    // Pre-fill with existing filename
    let existing = STATE.with(|s| s.borrow().current_file_path.clone());
    if let Some(ref p) = existing {
        let fname: Vec<u16> = p.encode_utf16().collect();
        let len = fname.len().min(buf.len() - 1);
        buf[..len].copy_from_slice(&fname[..len]);
    }

    let filter: Vec<u16> =
        "Text Files (*.txt)\0*.txt\0All Files (*.*)\0*.*\0\0"
            .encode_utf16()
            .collect();
    let default_ext: Vec<u16> = "txt\0".encode_utf16().collect();

    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = hwnd;
    ofn.lpstrFilter = PCWSTR(filter.as_ptr());
    ofn.lpstrFile = PWSTR(buf.as_mut_ptr());
    ofn.nMaxFile = MAX_PATH;
    ofn.lpstrTitle = w!("Save As");
    ofn.lpstrDefExt = PCWSTR(default_ext.as_ptr());
    ofn.Flags = OFN_OVERWRITEPROMPT | OFN_HIDEREADONLY;

    unsafe {
        if GetSaveFileNameW(&mut ofn).as_bool() {
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            Some(String::from_utf16_lossy(&buf[..end]))
        } else {
            None
        }
    }
}

/// Update window title: "*filename - Notepad Zero" or "Untitled - Notepad Zero"
pub fn update_title(hwnd: HWND) {
    let (path, dirty) = STATE.with(|s| {
        let st = s.borrow();
        (st.current_file_path.clone(), st.is_dirty)
    });

    let base_name = match &path {
        Some(p) => Path::new(p)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string(),
        None => "Untitled".to_string(),
    };

    let title = if dirty {
        format!("*{} - Notepad Zero\0", base_name)
    } else {
        format!("{} - Notepad Zero\0", base_name)
    };

    let wide: Vec<u16> = title.encode_utf16().collect();
    unsafe {
        SetWindowTextW(hwnd, PCWSTR(wide.as_ptr()));
    }
}
