#![windows_subsystem = "windows"]
#![allow(unused_must_use)]

mod dialogs;
mod file_io;
mod menu;
mod state;
mod status_bar;

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::DeleteObject;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, INITCOMMONCONTROLSEX, ICC_BAR_CLASSES,
};
use windows::Win32::UI::Controls::Dialogs::FINDREPLACEW;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetClientRect, GetMessageW, IsDialogMessageW, PostQuitMessage,
    RegisterClassExW, RegisterWindowMessageW, SendMessageW, SetWindowPos,
    ShowWindow, TranslateMessage, GetWindowTextLengthW, GetWindowTextW,
    SetWindowTextW,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, EN_CHANGE, HMENU,
    IDC_ARROW, LoadCursorW, MSG, SWP_NOZORDER, SW_SHOW,
    WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_INITMENUPOPUP,
    WM_SETFONT, WM_SIZE, WNDCLASSEXW, WS_CHILD, WS_EX_CLIENTEDGE,
    WS_HSCROLL, WS_OVERLAPPEDWINDOW, WS_VISIBLE, WS_VSCROLL,
    WINDOW_EX_STYLE, WINDOW_STYLE,
};

use crate::state::STATE;

// Raw edit control message IDs
const EM_SETLIMITTEXT: u32 = 0x00C5;
const EM_GETSEL: u32 = 0x00B0;
const EM_SETSEL: u32 = 0x00B1;

// ES_* style bits
const ES_MULTILINE: u32 = 0x0004;
const ES_AUTOVSCROLL: u32 = 0x0040;
const ES_AUTOHSCROLL: u32 = 0x0080;
const ES_NOHIDESEL: u32 = 0x0100;

// ─── Global HWND storage (as usize = raw pointer bits) ───────────────────────
static HWND_MAIN_RAW: AtomicUsize = AtomicUsize::new(0);
static HWND_EDIT_RAW: AtomicUsize = AtomicUsize::new(0);
static HWND_STATUS_RAW: AtomicUsize = AtomicUsize::new(0);

pub static FIND_MSG_ID: OnceLock<u32> = OnceLock::new();

#[inline]
pub fn hwnd_main() -> HWND {
    HWND(HWND_MAIN_RAW.load(Ordering::Relaxed) as *mut c_void)
}
#[inline]
pub fn hwnd_edit() -> HWND {
    HWND(HWND_EDIT_RAW.load(Ordering::Relaxed) as *mut c_void)
}
#[inline]
pub fn hwnd_status() -> HWND {
    HWND(HWND_STATUS_RAW.load(Ordering::Relaxed) as *mut c_void)
}
pub fn set_hwnd_edit(h: HWND) {
    HWND_EDIT_RAW.store(h.0 as usize, Ordering::Relaxed);
}

const IDC_EDIT: usize = 1;

fn main() {
    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);

        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: Default::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(6usize as *mut c_void),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: w!("Notepad0Class"),
            hIconSm: Default::default(),
        };

        RegisterClassExW(&wc);

        let find_msg = RegisterWindowMessageW(w!("commdlg_FindReplace"));
        FIND_MSG_ID.get_or_init(|| find_msg);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("Notepad0Class"),
            w!("Untitled - Notepad Zero"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT, CW_USEDEFAULT,
            800, 600,
            None, None,
            hinstance,
            None,
        ).unwrap_or(HWND(std::ptr::null_mut()));

        if hwnd.is_invalid() { return; }

        ShowWindow(hwnd, SW_SHOW);
        windows::Win32::Graphics::Gdi::UpdateWindow(hwnd);

        let mut msg = MSG::default();
        loop {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            if ret.0 == 0 || ret.0 == -1 { break; }

            let fr_hwnd = STATE.with(|s| s.borrow().find_replace_hwnd);
            if let Some(fh) = fr_hwnd {
                if !fh.is_invalid() && IsDialogMessageW(fh, &mut msg).as_bool() {
                    continue;
                }
            }

            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(&find_msg) = FIND_MSG_ID.get() {
        if msg == find_msg {
            let fr = lparam.0 as *const FINDREPLACEW;
            dialogs::handle_find_message(fr);
            return LRESULT(0);
        }
    }

    match msg {
        WM_CREATE => {
            HWND_MAIN_RAW.store(hwnd.0 as usize, Ordering::Relaxed);
            let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();

            let edit_style = WINDOW_STYLE(
                WS_CHILD.0 | WS_VISIBLE.0 | WS_VSCROLL.0
                    | ES_MULTILINE | ES_AUTOVSCROLL | ES_NOHIDESEL
                    | ES_AUTOHSCROLL | WS_HSCROLL.0,
            );

            let hwnd_edit_new = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                PCWSTR::null(),
                edit_style,
                0, 0, 0, 0,
                hwnd,
                HMENU(IDC_EDIT as *mut c_void),
                hinstance,
                None,
            ).unwrap_or(HWND(std::ptr::null_mut()));

            HWND_EDIT_RAW.store(hwnd_edit_new.0 as usize, Ordering::Relaxed);
            SendMessageW(hwnd_edit_new, EM_SETLIMITTEXT, WPARAM(0), LPARAM(0));
            apply_initial_font(hwnd_edit_new);

            let hwnd_status_new = status_bar::create_status_bar(hwnd, hinstance);
            HWND_STATUS_RAW.store(hwnd_status_new.0 as usize, Ordering::Relaxed);

            status_bar::subclass_edit(hwnd_edit_new);
            menu::create_menu(hwnd);
            status_bar::update_status_bar();

            LRESULT(0)
        }

        WM_SIZE => {
            let width = (lparam.0 as u32 & 0xFFFF) as i32;
            let height = ((lparam.0 as u32 >> 16) & 0xFFFF) as i32;

            let status_visible = STATE.with(|s| s.borrow().status_bar_visible);
            let hwnd_s = hwnd_status();

            if status_visible && !hwnd_s.is_invalid() {
                SendMessageW(hwnd_s, WM_SIZE, WPARAM(0), LPARAM(0));
            }

            let status_height = if status_visible && !hwnd_s.is_invalid() {
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd_s, &mut rc);
                rc.bottom
            } else {
                0
            };

            let _ = SetWindowPos(
                hwnd_edit(),
                None,
                0, 0,
                width,
                (height - status_height).max(0),
                SWP_NOZORDER,
            );
            LRESULT(0)
        }

        WM_COMMAND => {
            let notify = (wparam.0 >> 16) as u16;
            let id = (wparam.0 & 0xFFFF) as u32;

            if id == IDC_EDIT as u32 && (notify as u32) == EN_CHANGE {
                let suppress = STATE.with(|s| s.borrow().suppress_dirty);
                if !suppress {
                    STATE.with(|s| s.borrow_mut().is_dirty = true);
                    file_io::update_title(hwnd);
                }
                LRESULT(0)
            } else if notify == 0 || notify == 1 {
                menu::handle_command(hwnd, id);
                LRESULT(0)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }

        WM_INITMENUPOPUP => {
            let hmenu_popup = HMENU(wparam.0 as *mut c_void);
            menu::on_init_menu_popup(hwnd, hmenu_popup);
            LRESULT(0)
        }

        WM_CLOSE => {
            if menu::prompt_save_if_dirty(hwnd) {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }

        WM_DESTROY => {
            let hfont = STATE.with(|s| s.borrow_mut().hfont.take());
            if let Some(f) = hfont {
                if !f.is_invalid() { let _ = DeleteObject(f); }
            }
            PostQuitMessage(0);
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn apply_initial_font(hwnd_edit: HWND) {
    use crate::state::pts_to_lfheight;
    unsafe {
        let height = pts_to_lfheight(10);
        let face: Vec<u16> = "Courier New\0".encode_utf16().collect();
        let hfont = windows::Win32::Graphics::Gdi::CreateFontW(
            height, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 0, 0,
            PCWSTR(face.as_ptr()),
        );
        if !hfont.is_invalid() {
            SendMessageW(hwnd_edit, WM_SETFONT, WPARAM(hfont.0 as usize), LPARAM(1));
            STATE.with(|s| s.borrow_mut().hfont = Some(hfont));
        }
    }
}

/// Recreate EDIT control for Word Wrap toggle
pub fn recreate_edit(hwnd: HWND) {
    use std::ffi::c_void;
    unsafe {
        let old_edit = hwnd_edit();

        let len = GetWindowTextLengthW(old_edit);
        let mut text_buf: Vec<u16> = vec![0u16; (len + 1) as usize];
        GetWindowTextW(old_edit, &mut text_buf);
        text_buf.truncate(len as usize);

        let sel = SendMessageW(old_edit, EM_GETSEL, WPARAM(0), LPARAM(0)).0 as usize;
        let sel_start = sel & 0xFFFF;
        let sel_end = (sel >> 16) & 0xFFFF;

        status_bar::unsubclass_edit(old_edit);
        let _ = DestroyWindow(old_edit);
        HWND_EDIT_RAW.store(0, Ordering::Relaxed);

        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
        let word_wrap = STATE.with(|s| s.borrow().word_wrap_enabled);

        let base = WS_CHILD.0 | WS_VISIBLE.0 | WS_VSCROLL.0
            | ES_MULTILINE | ES_AUTOVSCROLL | ES_NOHIDESEL;

        let edit_style = if word_wrap {
            WINDOW_STYLE(base)
        } else {
            WINDOW_STYLE(base | ES_AUTOHSCROLL | WS_HSCROLL.0)
        };

        let new_edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR::null(),
            edit_style,
            0, 0, 0, 0,
            hwnd,
            HMENU(IDC_EDIT as *mut c_void),
            hinstance,
            None,
        ).unwrap_or(HWND(std::ptr::null_mut()));

        HWND_EDIT_RAW.store(new_edit.0 as usize, Ordering::Relaxed);
        SendMessageW(new_edit, EM_SETLIMITTEXT, WPARAM(0), LPARAM(0));

        STATE.with(|s| s.borrow_mut().suppress_dirty = true);
        text_buf.push(0);
        SetWindowTextW(new_edit, PCWSTR(text_buf.as_ptr()));
        STATE.with(|s| s.borrow_mut().suppress_dirty = false);

        SendMessageW(new_edit, EM_SETSEL, WPARAM(sel_start), LPARAM(sel_end as isize));

        let hfont = STATE.with(|s| s.borrow().hfont);
        if let Some(f) = hfont {
            if !f.is_invalid() {
                SendMessageW(new_edit, WM_SETFONT, WPARAM(f.0 as usize), LPARAM(1));
            }
        }

        status_bar::subclass_edit(new_edit);

        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        SendMessageW(
            hwnd,
            WM_SIZE,
            WPARAM(0),
            LPARAM(((rc.right as usize) | ((rc.bottom as usize) << 16)) as isize),
        );

        SetFocus(new_edit);
    }
}
