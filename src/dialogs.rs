use std::cell::RefCell;
use std::mem;

use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateFontIndirectW, DeleteObject, LOGFONTW};
use windows::Win32::UI::Controls::Dialogs::{
    ChooseFontW, CHOOSEFONTW, CF_INITTOLOGFONTSTRUCT, CF_NOVERTFONTS, CF_SCREENFONTS,
    FindTextW, ReplaceTextW, FINDREPLACEW,
    FR_DIALOGTERM, FR_DOWN, FR_FINDNEXT, FR_MATCHCASE, FR_REPLACE, FR_REPLACEALL, FR_WHOLEWORD,
};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    DialogBoxIndirectParamW, EndDialog, GetDlgItem, GetDlgItemTextW,
    MessageBoxW, SendMessageW, SetForegroundWindow,
    MB_ICONINFORMATION, MB_OK, WM_COMMAND, WM_INITDIALOG, WM_SETFONT,
    DLGTEMPLATE,
};

use crate::state::{wide, wide_sz, STATE};
use crate::{file_io, hwnd_edit, hwnd_main, find_replace_hwnd, set_find_replace_hwnd};

// ─── Edit control message constants ──────────────────────────────────────────
const EM_GETSEL: u32 = 0x00B0;
const EM_SETSEL: u32 = 0x00B1;
const EM_SCROLLCARET: u32 = 0x00B7;
const EM_LINEINDEX: u32 = 0x00BB;
const EM_LINECOUNT: u32 = 0x00BA;
const EM_REPLACESEL: u32 = 0x00C2;

// ─── Find/Replace state (heap-pinned so pointer stays valid) ─────────────────

struct FindReplaceState {
    fr: FINDREPLACEW,
    search_buf: Vec<u16>,
    replace_buf: Vec<u16>,
}

thread_local! {
    static FIND_STATE: RefCell<Option<Box<FindReplaceState>>> = RefCell::new(None);
}

const SEARCH_BUF_LEN: usize = 512;

fn make_find_state() -> Box<FindReplaceState> {
    let last_search = STATE.with(|s| s.borrow().last_search.clone());
    let last_replace = STATE.with(|s| s.borrow().last_replace.clone());

    let mut search_buf = vec![0u16; SEARCH_BUF_LEN];
    let mut replace_buf = vec![0u16; SEARCH_BUF_LEN];

    for (dst, src) in search_buf.iter_mut().zip(last_search.encode_utf16()) {
        *dst = src;
    }
    for (dst, src) in replace_buf.iter_mut().zip(last_replace.encode_utf16()) {
        *dst = src;
    }

    Box::new(FindReplaceState {
        fr: unsafe { mem::zeroed() },
        search_buf,
        replace_buf,
    })
}

pub fn open_find(hwnd: HWND) {
    let existing = find_replace_hwnd();
    if !existing.is_invalid() {
        unsafe { SetForegroundWindow(existing); }
        return;
    }

    FIND_STATE.with(|fs| {
        let mut guard = fs.borrow_mut();
        let mut state = make_find_state();

        state.fr.lStructSize = mem::size_of::<FINDREPLACEW>() as u32;
        state.fr.hwndOwner = hwnd;
        state.fr.lpstrFindWhat = PWSTR(state.search_buf.as_mut_ptr());
        state.fr.wFindWhatLen = SEARCH_BUF_LEN as u16;
        state.fr.Flags = FR_DOWN;

        let fr_ptr = &mut state.fr as *mut FINDREPLACEW;
        *guard = Some(state);

        let hdlg = unsafe { FindTextW(fr_ptr) };
        if !hdlg.is_invalid() {
            set_find_replace_hwnd(Some(hdlg));
        }
    });
}

pub fn open_replace(hwnd: HWND) {
    let existing = find_replace_hwnd();
    if !existing.is_invalid() {
        unsafe { SetForegroundWindow(existing); }
        return;
    }

    FIND_STATE.with(|fs| {
        let mut guard = fs.borrow_mut();
        let mut state = make_find_state();

        state.fr.lStructSize = mem::size_of::<FINDREPLACEW>() as u32;
        state.fr.hwndOwner = hwnd;
        state.fr.lpstrFindWhat = PWSTR(state.search_buf.as_mut_ptr());
        state.fr.wFindWhatLen = SEARCH_BUF_LEN as u16;
        state.fr.lpstrReplaceWith = PWSTR(state.replace_buf.as_mut_ptr());
        state.fr.wReplaceWithLen = SEARCH_BUF_LEN as u16;
        state.fr.Flags = FR_DOWN;

        let fr_ptr = &mut state.fr as *mut FINDREPLACEW;
        *guard = Some(state);

        let hdlg = unsafe { ReplaceTextW(fr_ptr) };
        if !hdlg.is_invalid() {
            set_find_replace_hwnd(Some(hdlg));
        }
    });
}

/// Handle the FINDMSGSTRING registered message dispatched to WndProc
pub fn handle_find_message(fr: *const FINDREPLACEW) {
    unsafe {
        if fr.is_null() { return; }
        let flags = (*fr).Flags;

        if (flags & FR_DIALOGTERM).0 != 0 {
            FIND_STATE.with(|fs| *fs.borrow_mut() = None);
            set_find_replace_hwnd(None);
            return;
        }

        let needle = pwstr_to_string((*fr).lpstrFindWhat.0);
        let match_case = (flags & FR_MATCHCASE).0 != 0;
        let whole_word = (flags & FR_WHOLEWORD).0 != 0;
        let forward = (flags & FR_DOWN).0 != 0;

        STATE.with(|s| {
            let mut st = s.borrow_mut();
            st.last_search = needle.clone();
            st.search_match_case = match_case;
            st.search_whole_word = whole_word;
        });

        if (flags & FR_REPLACEALL).0 != 0 {
            let replacement = pwstr_to_string((*fr).lpstrReplaceWith.0);
            STATE.with(|s| s.borrow_mut().last_replace = replacement.clone());
            do_replace_all(&needle, &replacement, match_case);
        } else if (flags & FR_REPLACE).0 != 0 {
            let replacement = pwstr_to_string((*fr).lpstrReplaceWith.0);
            STATE.with(|s| s.borrow_mut().last_replace = replacement.clone());
            do_replace_current(&needle, &replacement, match_case, whole_word);
        } else if (flags & FR_FINDNEXT).0 != 0 {
            find_next_internal(hwnd_main(), &needle, match_case, whole_word, forward);
        }
    }
}

unsafe fn pwstr_to_string(ptr: *mut u16) -> String {
    if ptr.is_null() { return String::new(); }
    let mut len = 0;
    while *ptr.add(len) != 0 { len += 1; }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

/// Find next (called from F3 / Edit > Find Next menu item)
pub fn find_next(hwnd: HWND, forward: bool) {
    let (needle, match_case, whole_word) = STATE.with(|s| {
        let st = s.borrow();
        (st.last_search.clone(), st.search_match_case, st.search_whole_word)
    });
    if needle.is_empty() {
        open_find(hwnd);
        return;
    }
    find_next_internal(hwnd, &needle, match_case, whole_word, forward);
}

fn find_next_internal(hwnd: HWND, needle: &str, match_case: bool, _whole_word: bool, forward: bool) {
    if needle.is_empty() { return; }

    let edit = hwnd_edit();
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowTextLengthW, GetWindowTextW,
        };

        let len = GetWindowTextLengthW(edit);
        if len == 0 { return; }
        let mut buf: Vec<u16> = vec![0u16; (len + 1) as usize];
        GetWindowTextW(edit, &mut buf);
        buf.truncate(len as usize);

        // Pointer args keep positions 32-bit (return value truncates at 64K)
        let mut sel_start: u32 = 0;
        let mut sel_end: u32 = 0;
        SendMessageW(
            edit,
            EM_GETSEL,
            WPARAM(&mut sel_start as *mut u32 as usize),
            LPARAM(&mut sel_end as *mut u32 as isize),
        );
        let sel_start = sel_start as usize;
        let sel_end = sel_end as usize;

        let needle_wide = wide(needle);

        let found = if forward {
            find_in_wide(&buf, &needle_wide, sel_end, match_case)
                .or_else(|| {
                    if sel_end > 0 {
                        find_in_wide(&buf, &needle_wide, 0, match_case)
                    } else {
                        None
                    }
                })
        } else {
            let search_end = sel_start;
            find_in_wide_backward(&buf, &needle_wide, search_end, match_case)
                .or_else(|| find_in_wide_backward(&buf, &needle_wide, buf.len(), match_case))
        };

        match found {
            Some(pos) => {
                let end = pos + needle_wide.len();
                SendMessageW(edit, EM_SETSEL, WPARAM(pos), LPARAM(end as isize));
                SendMessageW(edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
            }
            None => {
                let msg = wide_sz(&format!("Cannot find \"{}\".", needle));
                MessageBoxW(hwnd, PCWSTR(msg.as_ptr()), w!("Notepad Zero"), MB_OK);
            }
        }
    }
}

fn find_in_wide(haystack: &[u16], needle: &[u16], start: usize, match_case: bool) -> Option<usize> {
    if needle.is_empty() { return Some(start); }
    if needle.len() > haystack.len() { return None; }
    let limit = haystack.len() - needle.len() + 1;
    if start >= limit { return None; }

    for i in start..limit {
        if compare_wide(&haystack[i..i + needle.len()], needle, match_case) {
            return Some(i);
        }
    }
    None
}

fn find_in_wide_backward(haystack: &[u16], needle: &[u16], end: usize, match_case: bool) -> Option<usize> {
    if needle.is_empty() { return Some(0); }
    if needle.len() > haystack.len() { return None; }
    let max_start = end.saturating_sub(needle.len());

    for i in (0..=max_start).rev() {
        if compare_wide(&haystack[i..i + needle.len()], needle, match_case) {
            return Some(i);
        }
    }
    None
}

fn compare_wide(a: &[u16], b: &[u16], match_case: bool) -> bool {
    if match_case {
        a == b
    } else {
        a.len() == b.len()
            && a.iter().zip(b.iter()).all(|(&x, &y)| {
                // Simple ASCII case-fold; good enough for most text
                let xlo = if x >= b'A' as u16 && x <= b'Z' as u16 { x + 32 } else { x };
                let ylo = if y >= b'A' as u16 && y <= b'Z' as u16 { y + 32 } else { y };
                xlo == ylo
            })
    }
}

fn do_replace_current(needle: &str, replacement: &str, match_case: bool, whole_word: bool) {
    let edit = hwnd_edit();
    unsafe {
        // If current selection matches needle, replace it
        // Pointer args keep positions 32-bit (return value truncates at 64K)
        let mut sel_start: u32 = 0;
        let mut sel_end: u32 = 0;
        SendMessageW(
            edit,
            EM_GETSEL,
            WPARAM(&mut sel_start as *mut u32 as usize),
            LPARAM(&mut sel_end as *mut u32 as isize),
        );
        let sel_start = sel_start as usize;
        let sel_end = sel_end as usize;

        let needle_wide = wide(needle);
        if sel_end - sel_start == needle_wide.len() {
            // Verify the selected text matches
            use windows::Win32::UI::WindowsAndMessaging::{
                GetWindowTextLengthW, GetWindowTextW,
            };
            let len = GetWindowTextLengthW(edit) as usize;
            let mut buf: Vec<u16> = vec![0u16; len + 1];
            GetWindowTextW(edit, &mut buf);

            if sel_start + needle_wide.len() <= len
                && compare_wide(
                    &buf[sel_start..sel_start + needle_wide.len()],
                    &needle_wide,
                    match_case,
                )
            {
                let rep_wide = wide_sz(replacement);
                SendMessageW(
                    edit,
                    EM_REPLACESEL,
                    WPARAM(1),
                    LPARAM(rep_wide.as_ptr() as isize),
                );
                STATE.with(|s| s.borrow_mut().is_dirty = true);
                file_io::update_title(hwnd_main());
            }
        }
    }
    // Find next occurrence
    find_next_internal(hwnd_main(), needle, match_case, whole_word, true);
}

fn do_replace_all(needle: &str, replacement: &str, match_case: bool) {
    if needle.is_empty() { return; }

    let edit = hwnd_edit();
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowTextLengthW, GetWindowTextW, SetWindowTextW,
        };

        let len = GetWindowTextLengthW(edit) as usize;
        if len == 0 { return; }

        let mut buf: Vec<u16> = vec![0u16; len + 1];
        GetWindowTextW(edit, &mut buf);
        buf.truncate(len);

        let needle_wide = wide(needle);
        let rep_wide = wide(replacement);

        let mut positions: Vec<usize> = Vec::new();
        let mut pos = 0;
        while pos + needle_wide.len() <= buf.len() {
            if let Some(p) = find_in_wide(&buf, &needle_wide, pos, match_case) {
                positions.push(p);
                pos = p + needle_wide.len().max(1);
            } else {
                break;
            }
        }

        let count = positions.len();
        if count == 0 {
            let msg = wide_sz(&format!("Cannot find \"{}\".", needle));
            MessageBoxW(hwnd_main(), PCWSTR(msg.as_ptr()), w!("Notepad Zero"), MB_OK);
            return;
        }

        // Replace back-to-front to preserve indices
        let mut result = buf;
        for &p in positions.iter().rev() {
            result.splice(p..p + needle_wide.len(), rep_wide.iter().copied());
        }
        result.push(0);

        STATE.with(|s| s.borrow_mut().suppress_dirty = true);
        SetWindowTextW(edit, PCWSTR(result.as_ptr()));
        STATE.with(|s| s.borrow_mut().suppress_dirty = false);
        STATE.with(|s| s.borrow_mut().is_dirty = true);
        file_io::update_title(hwnd_main());

        let msg = wide_sz(&format!("{} replacement(s) made.", count));
        MessageBoxW(
            hwnd_main(),
            PCWSTR(msg.as_ptr()),
            w!("Notepad Zero"),
            MB_ICONINFORMATION | MB_OK,
        );
    }
}

// ─── Go To Line Dialog ────────────────────────────────────────────────────────

pub fn open_goto(hwnd: HWND) {
    unsafe {
        let template = build_goto_template();
        DialogBoxIndirectParamW(
            None,
            template.as_ptr() as *const DLGTEMPLATE,
            hwnd,
            Some(goto_dlg_proc),
            LPARAM(0),
        );
    }
}

fn build_goto_template() -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();

    let push_u16 = |buf: &mut Vec<u8>, v: u16| buf.extend_from_slice(&v.to_le_bytes());
    let push_u32 = |buf: &mut Vec<u8>, v: u32| buf.extend_from_slice(&v.to_le_bytes());
    let push_i16 = |buf: &mut Vec<u8>, v: i16| buf.extend_from_slice(&v.to_le_bytes());
    let align4 = |buf: &mut Vec<u8>| {
        while buf.len() % 4 != 0 { buf.push(0); }
    };
    let push_wstr = |buf: &mut Vec<u8>, s: &str| {
        for c in s.encode_utf16() {
            buf.extend_from_slice(&c.to_le_bytes());
        }
        buf.extend_from_slice(&0u16.to_le_bytes());
    };

    // DS_MODALFRAME=0x80, DS_CENTER=0x0800, DS_SETFONT=0x40
    // WS_POPUP=0x80000000, WS_CAPTION=0x00C00000, WS_SYSMENU=0x00080000
    let style: u32 = 0x80000000 | 0x00C00000 | 0x00080000 | 0x0800 | 0x0080 | 0x0040;
    push_u32(&mut buf, style);
    push_u32(&mut buf, 0);  // exstyle
    push_u16(&mut buf, 4);  // cDlgItems
    push_i16(&mut buf, 0); push_i16(&mut buf, 0);    // x, y
    push_i16(&mut buf, 185); push_i16(&mut buf, 65); // cx, cy
    push_u16(&mut buf, 0);  // no menu
    push_u16(&mut buf, 0);  // default dialog class
    push_wstr(&mut buf, "Go To Line");
    // DS_SETFONT: point size + typeface
    push_u16(&mut buf, 9);
    push_wstr(&mut buf, "MS Shell Dlg");

    // Item 1: Static "Line number:" label
    align4(&mut buf);
    // SS_LEFT=0, WS_CHILD=0x40000000, WS_VISIBLE=0x10000000
    push_u32(&mut buf, 0x40000000 | 0x10000000);
    push_u32(&mut buf, 0);
    push_i16(&mut buf, 7); push_i16(&mut buf, 12);
    push_i16(&mut buf, 120); push_i16(&mut buf, 12);
    push_u16(&mut buf, 0xFFFF);  // id (static)
    push_u16(&mut buf, 0xFFFF); push_u16(&mut buf, 0x0082); // STATIC class atom
    push_wstr(&mut buf, "Line number:");
    push_u16(&mut buf, 0); // no creation data

    // Item 2: Edit control (id=100)
    align4(&mut buf);
    // ES_AUTOHSCROLL=0x80, WS_BORDER=0x00800000, WS_CHILD, WS_VISIBLE, WS_TABSTOP=0x00010000
    push_u32(&mut buf, 0x40000000 | 0x10000000 | 0x00010000 | 0x00800000 | 0x0080);
    push_u32(&mut buf, 0);
    push_i16(&mut buf, 7); push_i16(&mut buf, 28);
    push_i16(&mut buf, 120); push_i16(&mut buf, 14);
    push_u16(&mut buf, 100); // id
    push_u16(&mut buf, 0xFFFF); push_u16(&mut buf, 0x0081); // EDIT class atom
    push_u16(&mut buf, 0); // empty title
    push_u16(&mut buf, 0); // no creation data

    // Item 3: OK button (IDOK=1)
    align4(&mut buf);
    // BS_DEFPUSHBUTTON=0x01, WS_CHILD, WS_VISIBLE, WS_TABSTOP
    push_u32(&mut buf, 0x40000000 | 0x10000000 | 0x00010000 | 0x01);
    push_u32(&mut buf, 0);
    push_i16(&mut buf, 133); push_i16(&mut buf, 8);
    push_i16(&mut buf, 48); push_i16(&mut buf, 14);
    push_u16(&mut buf, 1); // IDOK
    push_u16(&mut buf, 0xFFFF); push_u16(&mut buf, 0x0080); // BUTTON class atom
    push_wstr(&mut buf, "OK");
    push_u16(&mut buf, 0);

    // Item 4: Cancel button (IDCANCEL=2)
    align4(&mut buf);
    push_u32(&mut buf, 0x40000000 | 0x10000000 | 0x00010000);
    push_u32(&mut buf, 0);
    push_i16(&mut buf, 133); push_i16(&mut buf, 26);
    push_i16(&mut buf, 48); push_i16(&mut buf, 14);
    push_u16(&mut buf, 2); // IDCANCEL
    push_u16(&mut buf, 0xFFFF); push_u16(&mut buf, 0x0080);
    push_wstr(&mut buf, "Cancel");
    push_u16(&mut buf, 0);

    buf
}

unsafe extern "system" fn goto_dlg_proc(
    hdlg: HWND,
    msg: u32,
    wparam: WPARAM,
    _lparam: LPARAM,
) -> isize {
    match msg {
        m if m == WM_INITDIALOG => {
            if let Ok(edit_ctrl) = GetDlgItem(hdlg, 100) {
                let _ = SetFocus(edit_ctrl);
            }
            1
        }
        m if m == WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            if id == 1 {
                // IDOK
                let mut buf = vec![0u16; 32];
                GetDlgItemTextW(hdlg, 100, &mut buf);
                let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                let s = String::from_utf16_lossy(&buf[..end]);
                if let Ok(line_num) = s.trim().parse::<i32>() {
                    goto_line(line_num);
                }
                let _ = EndDialog(hdlg, 0);
                1
            } else if id == 2 {
                // IDCANCEL
                let _ = EndDialog(hdlg, 0);
                1
            } else {
                0
            }
        }
        _ => 0,
    }
}

fn goto_line(line_num: i32) {
    let edit = hwnd_edit();
    unsafe {
        let total_lines = SendMessageW(edit, EM_LINECOUNT, WPARAM(0), LPARAM(0)).0 as i32;
        let target = (line_num - 1).max(0).min(total_lines - 1);
        let char_idx = SendMessageW(edit, EM_LINEINDEX, WPARAM(target as usize), LPARAM(0)).0
            as usize;
        SendMessageW(edit, EM_SETSEL, WPARAM(char_idx), LPARAM(char_idx as isize));
        SendMessageW(edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
    }
}

// ─── Font Dialog ──────────────────────────────────────────────────────────────

pub fn choose_font(hwnd: HWND) {
    use crate::state::{lfheight_to_pts, pts_to_lfheight};

    let (face, pts, weight, italic, charset) = STATE.with(|s| {
        let st = s.borrow();
        (
            st.current_font.face_name.clone(),
            st.current_font.height_pts,
            st.current_font.weight,
            st.current_font.italic,
            st.current_font.charset,
        )
    });

    unsafe {
        let mut lf: LOGFONTW = mem::zeroed();
        lf.lfHeight = pts_to_lfheight(pts);
        lf.lfWeight = weight;
        lf.lfItalic = if italic { 1 } else { 0 };
        lf.lfCharSet = windows::Win32::Graphics::Gdi::FONT_CHARSET(charset as _);

        let face_wide: Vec<u16> = face.encode_utf16().collect();
        let copy_len = face_wide.len().min(lf.lfFaceName.len() - 1);
        lf.lfFaceName[..copy_len].copy_from_slice(&face_wide[..copy_len]);

        let mut cf: CHOOSEFONTW = mem::zeroed();
        cf.lStructSize = mem::size_of::<CHOOSEFONTW>() as u32;
        cf.hwndOwner = hwnd;
        cf.lpLogFont = &mut lf;
        cf.Flags = CF_SCREENFONTS | CF_INITTOLOGFONTSTRUCT | CF_NOVERTFONTS;

        if ChooseFontW(&mut cf).as_bool() {
            let face_end = lf.lfFaceName.iter().position(|&c| c == 0).unwrap_or(lf.lfFaceName.len());
            let new_face = String::from_utf16_lossy(&lf.lfFaceName[..face_end]);
            let new_pts = lfheight_to_pts(lf.lfHeight.unsigned_abs() as i32);

            STATE.with(|s| {
                let mut st = s.borrow_mut();
                st.current_font.face_name = new_face;
                st.current_font.height_pts = new_pts;
                st.current_font.weight = lf.lfWeight;
                st.current_font.italic = lf.lfItalic != 0;
                st.current_font.charset = lf.lfCharSet.0 as u8;
            });

            let new_font = CreateFontIndirectW(&lf);
            if !new_font.is_invalid() {
                let edit = hwnd_edit();
                SendMessageW(edit, WM_SETFONT, WPARAM(new_font.0 as usize), LPARAM(1));

                let old = STATE.with(|s| s.borrow_mut().hfont.take());
                if let Some(old_font) = old {
                    if !old_font.is_invalid() {
                        let _ = DeleteObject(old_font);
                    }
                }
                STATE.with(|s| s.borrow_mut().hfont = Some(new_font));
            }
        }
    }
}
