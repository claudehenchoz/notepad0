use std::ffi::c_void;
use std::path::Path;

use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{HLOCAL, HWND, LPARAM, MAX_PATH, WPARAM, LocalFree};
use windows::Win32::System::Memory::{LocalAlloc, LocalLock, LocalUnlock, LMEM_MOVEABLE};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OPENFILENAMEW,
    OFN_FILEMUSTEXIST, OFN_HIDEREADONLY, OFN_OVERWRITEPROMPT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowTextLengthW, GetWindowTextW, MessageBoxW, SendMessageW, SetWindowTextW,
    MB_ICONERROR, MB_OK,
};

use crate::state::wide_sz;
use crate::hwnd_edit;
use crate::STATE;

// Edit control messages not re-exported by the `windows` crate
const EM_SETLIMITTEXT: u32 = 0x00C5;
const EM_SETHANDLE: u32 = 0x00BC;
const EM_GETHANDLE: u32 = 0x00BD;

// ─── Encoding detection / decoding (replaces encoding_rs) ─────────────────────
//
// Only three encodings are supported: UTF-16 (by BOM), UTF-8 (validated), and
// Windows-1252 (fallback).  Hand-rolling this removes encoding_rs' static
// tables from the binary and avoids its extra full-file scans.

/// Windows-1252 mapping for bytes 0x80..=0x9F. 0x00..=0x7F and 0xA0..=0xFF are
/// identity mappings (Latin-1), so they need no table.
const CP1252_HIGH: [u16; 32] = [
    0x20AC, 0xFFFD, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021,
    0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0xFFFD, 0x017D, 0xFFFD,
    0xFFFD, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0xFFFD, 0x017E, 0x0178,
];

#[inline]
fn cp1252_to_utf16(b: u8) -> u16 {
    if (0x80..=0x9F).contains(&b) {
        CP1252_HIGH[(b - 0x80) as usize]
    } else {
        b as u16
    }
}

/// Append UTF-16 code units to `out`, expanding bare `\n` into `\r\n` when
/// `expand` is set.  `expand` is false when the source text already contains a
/// `\r` (matching the original "if it has CR, leave line endings alone" rule).
fn push_crlf(out: &mut Vec<u16>, units: impl Iterator<Item = u16>, expand: bool) {
    if expand {
        for u in units {
            if u == 0x0A {
                out.push(0x0D);
            }
            out.push(u);
        }
    } else {
        out.extend(units);
    }
}

/// Decode raw file bytes into a null-terminated UTF-16 buffer with CRLF
/// normalization.  Single pass, no intermediate `String` allocations:
/// UTF-8 validates in place (`from_utf8` borrows), UTF-16 decodes chunk-wise,
/// and Windows-1252 maps 1:1.
fn decode_to_utf16_crlf(raw: &[u8]) -> Vec<u16> {
    if raw.starts_with(&[0xFF, 0xFE]) {
        return utf16_to_utf16_crlf(&raw[2..], false);
    }
    if raw.starts_with(&[0xFE, 0xFF]) {
        return utf16_to_utf16_crlf(&raw[2..], true);
    }

    let body = if raw.starts_with(&[0xEF, 0xBB, 0xBF]) { &raw[3..] } else { raw };
    // A 0x0D byte in UTF-8 / Windows-1252 always decodes to U+000D, so a byte
    // scan is equivalent to scanning the decoded text.
    let expand = !body.contains(&b'\r');

    let mut out = Vec::with_capacity(body.len() + 16);
    match std::str::from_utf8(body) {
        Ok(s) => push_crlf(&mut out, s.encode_utf16(), expand),
        Err(_) => push_crlf(&mut out, body.iter().copied().map(cp1252_to_utf16), expand),
    }
    out.push(0); // null terminator
    out
}

fn utf16_to_utf16_crlf(bytes: &[u8], big_endian: bool) -> Vec<u16> {
    let mut units = Vec::with_capacity(bytes.len() / 2);
    if big_endian {
        for c in bytes.chunks_exact(2) {
            units.push(u16::from_be_bytes([c[0], c[1]]));
        }
    } else {
        for c in bytes.chunks_exact(2) {
            units.push(u16::from_le_bytes([c[0], c[1]]));
        }
    }
    let expand = !units.contains(&0x0D);
    let mut out = Vec::with_capacity(units.len() + 16);
    push_crlf(&mut out, units.into_iter(), expand);
    out.push(0); // null terminator
    out
}

// ─── Edit control text installation ──────────────────────────────────────────

/// Fast text install: hand the edit control ownership of a pre-sized UTF-16
/// buffer via `EM_SETHANDLE` instead of `SetWindowTextW`, which would make the
/// control copy the entire document into its own (re)allocated buffer.
///
/// `EM_SETHANDLE` contract: the buffer must come from
/// `LocalAlloc(LMEM_MOVEABLE)` and contain null-terminated UTF-16 text.  The
/// control uses the buffer directly (no copy).
unsafe fn set_edit_text_fast(edit: HWND, wide: &[u16]) {
    if let Ok(h) = LocalAlloc(LMEM_MOVEABLE, wide.len() * 2) {
        let p = LocalLock(h);
        if !p.is_null() {
            std::ptr::copy_nonoverlapping(wide.as_ptr(), p as *mut u16, wide.len());
            let _ = LocalUnlock(h);

            // Retrieve the control's previous buffer and free it after the
            // swap; app-provided and control-internal buffers alike are
            // process-heap LocalAlloc blocks owned by whoever holds them, and
            // after EM_SETHANDLE only the new handle belongs to the control.
            let old = SendMessageW(edit, EM_GETHANDLE, WPARAM(0), LPARAM(0));
            SendMessageW(edit, EM_SETHANDLE, WPARAM(h.0 as usize), LPARAM(0));
            let old = HLOCAL(old.0 as *mut c_void);
            if !old.is_invalid() && old != h {
                let _ = LocalFree(old);
            }
            return;
        }
        let _ = LocalFree(h);
    }
    // Fallback (allocation failure): classic copy path.
    SetWindowTextW(edit, PCWSTR(wide.as_ptr()));
}

// ─── File loading ────────────────────────────────────────────────────────────

/// Read a file and load it into the EDIT control
pub fn load_file(hwnd: HWND, path: &str) -> bool {
    let raw = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            let msg = wide_sz(&format!("Cannot open file:\n{}", e));
            unsafe {
                MessageBoxW(hwnd, PCWSTR(msg.as_ptr()), w!("Notepad Zero"), MB_ICONERROR | MB_OK);
            }
            return false;
        }
    };

    // Single pass: decode → CRLF-normalize → UTF-16 (with null terminator).
    let wide_text = decode_to_utf16_crlf(&raw);

    unsafe {
        let edit = hwnd_edit();

        STATE.with(|s| s.borrow_mut().suppress_dirty = true);
        set_edit_text_fast(edit, &wide_text);

        // Reset limit after setting text (recommended after EM_SETHANDLE too).
        SendMessageW(edit, EM_SETLIMITTEXT, WPARAM(0), LPARAM(0));

        STATE.with(|s| {
            let mut st = s.borrow_mut();
            st.suppress_dirty = false;
            st.is_dirty = false;
            st.current_file_path = Some(path.to_string());
        });
    }

    // EM_SETHANDLE doesn't raise WM_SETTEXT, so refresh the Ln/Col display.
    crate::status_bar::update_status_bar();

    update_title(hwnd);
    true
}

// ─── File saving ─────────────────────────────────────────────────────────────

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

    // Fetch UTF-16 from the edit control once and encode straight into a
    // UTF-8 byte buffer — no intermediate String copy of the whole document.
    let bytes = get_edit_text_as_utf8();

    if let Err(e) = std::fs::write(&file_path, &bytes) {
        let msg = wide_sz(&format!("Cannot save file:\n{}", e));
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

/// Get all text from the EDIT control as UTF-16 (no null terminator).
pub fn get_edit_text_wide() -> Vec<u16> {
    let edit = hwnd_edit();
    unsafe {
        let len = GetWindowTextLengthW(edit);
        if len == 0 {
            return Vec::new();
        }
        let mut buf: Vec<u16> = vec![0u16; len as usize + 1];
        GetWindowTextW(edit, &mut buf);
        buf.truncate(len as usize);
        buf
    }
}

/// Get all text from the EDIT control as UTF-8 bytes, without an intermediate
/// `String` allocation.  Unpaired surrogates become U+FFFD (same as the
/// previous `from_utf16_lossy` behaviour).
pub fn get_edit_text_as_utf8() -> Vec<u8> {
    let units = get_edit_text_wide();
    let mut out = Vec::with_capacity(units.len());
    for r in std::char::decode_utf16(units.iter().copied()) {
        match r {
            Ok(ch) => {
                let mut b = [0u8; 4];
                out.extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
            }
            Err(_) => out.extend_from_slice("\u{FFFD}".as_bytes()),
        }
    }
    out
}

// ─── Open / Save dialogs ─────────────────────────────────────────────────────

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

// ─── Title bar (cached — no SetWindowTextW unless the title actually changes) ─

/// Update window title: "*filename - Notepad Zero" or "Untitled - Notepad Zero".
/// The rendered title is cached as UTF-16; `SetWindowTextW` (and the non-client
/// repaint it triggers) is only called when the title text actually changes.
/// This used to run — with a path `String` clone, `Path` parsing, `format!`
/// and a fresh heap buffer — on every single keystroke via `EN_CHANGE`.
pub fn update_title(hwnd: HWND) {
    let new_title: Vec<u16> = STATE.with(|s| {
        let st = s.borrow();
        let base_name = match &st.current_file_path {
            Some(p) => Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Untitled"),
            None => "Untitled",
        };
        let prefix = if st.is_dirty { "*" } else { "" };
        wide_sz(&format!("{}{} - Notepad Zero", prefix, base_name))
    });

    let mut need_set = false;
    STATE.with(|s| {
        let mut st = s.borrow_mut();
        if st.title_cache != new_title {
            st.title_cache = new_title.clone();
            need_set = true;
        }
    });

    if need_set {
        unsafe { SetWindowTextW(hwnd, PCWSTR(new_title.as_ptr())); }
    }
}
