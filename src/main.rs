// Notepad0 - a Windows 7-style Notepad clone
// Copyright (C) 2026 Claude Henchoz
// SPDX-License-Identifier: GPL-3.0-or-later
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

#![windows_subsystem = "windows"]
#![allow(unused_must_use)]

mod dialogs;
mod file_io;
mod menu;
mod settings;
mod state;
mod status_bar;

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, HINSTANCE, HWND, LPARAM, LRESULT, MAX_PATH, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::DeleteObject;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::FINDREPLACEW;
use windows::Win32::UI::Controls::{InitCommonControlsEx, ICC_BAR_CLASSES, ICC_LINK_CLASS, INITCOMMONCONTROLSEX};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect, GetMessageW,
    GetSystemMetrics, GetWindowTextLengthW, GetWindowTextW, IsDialogMessageW, LoadCursorW,
    LoadIconW, LoadImageW, PostQuitMessage, HICON, IMAGE_ICON, LR_DEFAULTCOLOR, SM_CXSMICON,
    SM_CYSMICON,
    RegisterClassExW, RegisterWindowMessageW, SendMessageW, SetWindowPos, SetWindowTextW,
    ShowWindow, TranslateAcceleratorW, TranslateMessage, ACCEL,
    CreateAcceleratorTableW, CW_USEDEFAULT, EN_CHANGE, FCONTROL,
    FVIRTKEY, HACCEL, HMENU, IDC_ARROW, MSG, SWP_NOZORDER, SW_HIDE, SW_SHOW, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_CLOSE, WM_COMMAND,
    WM_CREATE, WM_DESTROY, WM_INITMENUPOPUP, WM_SETFONT, WM_SIZE, WNDCLASSEXW, WS_CHILD,
    WS_EX_CLIENTEDGE, WS_HSCROLL, WS_OVERLAPPEDWINDOW, WS_VISIBLE, WS_VSCROLL,
};

use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};

use crate::state::STATE;

// Raw edit control message IDs
const EM_SETLIMITTEXT: u32 = 0x00C5;
const EM_GETSEL: u32 = 0x00B0;
const EM_SETSEL: u32 = 0x00B1;

const WM_DROPFILES: u32 = 0x0233;

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

// Find/Replace dialog HWND lives in an atomic so the message loop (which sees
// every message) doesn't need a thread-local RefCell borrow per message.
static FIND_REPLACE_HWND_RAW: AtomicUsize = AtomicUsize::new(0);

#[inline]
pub fn find_replace_hwnd() -> HWND {
    HWND(FIND_REPLACE_HWND_RAW.load(Ordering::Relaxed) as *mut c_void)
}
pub fn set_find_replace_hwnd(h: Option<HWND>) {
    FIND_REPLACE_HWND_RAW.store(h.map_or(0, |h| h.0 as usize), Ordering::Relaxed);
}

const IDC_EDIT: usize = 1;

fn main() {
    // Load persisted settings into STATE before any window is created.
    // HKCU registry reads are memory-backed — no disk I/O, negligible latency.
    settings::load();

    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES | ICC_LINK_CLASS,
        };
        let _ = InitCommonControlsEx(&icc);

        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();

        // No CS_HREDRAW/CS_VREDRAW: they force a full erase+repaint of the
        // whole window on every resize. The EDIT control covers the client
        // area and paints itself, so the class styles are pure overhead.
        // Icon resource ID 1, embedded from res/notepad0.rc by build.rs.
        let icon_id = PCWSTR(1usize as *const u16);
        let icon_sm = LoadImageW(
            hinstance,
            icon_id,
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_DEFAULTCOLOR,
        )
        .map(|h| HICON(h.0))
        .unwrap_or_default();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: Default::default(),
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: LoadIconW(hinstance, icon_id).unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(6usize as *mut c_void),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: w!("Notepad0Class"),
            hIconSm: icon_sm,
        };

        RegisterClassExW(&wc);

        let find_msg = RegisterWindowMessageW(w!("commdlg_FindReplace"));
        FIND_MSG_ID.get_or_init(|| find_msg);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("Notepad0Class"),
            w!("Untitled - Notepad Zero"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            800,
            600,
            None,
            None,
            hinstance,
            None,
        )
        .unwrap_or(HWND(std::ptr::null_mut()));

        if hwnd.is_invalid() {
            return;
        }

        ShowWindow(hwnd, SW_SHOW);
        windows::Win32::Graphics::Gdi::UpdateWindow(hwnd);
        SetFocus(hwnd_edit());

        // Open file passed on the command line (e.g. notepad0.exe foo.txt)
        if let Some(arg_path) = std::env::args().nth(1) {
            let resolved = std::fs::canonicalize(&arg_path)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or(arg_path);
            file_io::load_file(hwnd, &resolved);
        }

        // Build the accelerator table for all menu keyboard shortcuts.
        // CreateAcceleratorTableW copies the array internally so it needs no lifetime.
        let cv = FVIRTKEY | FCONTROL;
        let fv = FVIRTKEY;
        let accels = [
            ACCEL { fVirt: cv, key: b'N' as u16, cmd: menu::FILE_NEW    as u16 }, // Ctrl+N
            ACCEL { fVirt: cv, key: b'O' as u16, cmd: menu::FILE_OPEN   as u16 }, // Ctrl+O
            ACCEL { fVirt: cv, key: b'S' as u16, cmd: menu::FILE_SAVE   as u16 }, // Ctrl+S
            ACCEL { fVirt: cv, key: b'P' as u16, cmd: menu::FILE_PRINT  as u16 }, // Ctrl+P
            ACCEL { fVirt: cv, key: b'F' as u16, cmd: menu::EDIT_FIND   as u16 }, // Ctrl+F
            ACCEL { fVirt: cv, key: b'H' as u16, cmd: menu::EDIT_REPLACE as u16 }, // Ctrl+H
            ACCEL { fVirt: cv, key: b'G' as u16, cmd: menu::EDIT_GOTO   as u16 }, // Ctrl+G
            ACCEL { fVirt: fv, key: 0x72,         cmd: menu::EDIT_FINDNEXT as u16 }, // F3
            ACCEL { fVirt: fv, key: 0x74,         cmd: menu::EDIT_TIMEDATE as u16 }, // F5
        ];
        let haccel: Option<HACCEL> = CreateAcceleratorTableW(&accels).ok();

        let mut msg = MSG::default();
        loop {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            if ret.0 == 0 || ret.0 == -1 {
                break;
            }

            // Accelerators must be checked first so Ctrl+N/O/S/P etc. are
            // intercepted before the EDIT control or dialog sees them.
            if let Some(ha) = haccel {
                if TranslateAcceleratorW(hwnd, ha, &msg) != 0 {
                    continue;
                }
            }

            let fh = find_replace_hwnd();
            if !fh.is_invalid() && IsDialogMessageW(fh, &mut msg).as_bool() {
                continue;
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

            let base = WS_CHILD.0 | WS_VISIBLE.0 | WS_VSCROLL.0
                | ES_MULTILINE | ES_AUTOVSCROLL | ES_NOHIDESEL;
            let word_wrap_on = STATE.with(|s| s.borrow().word_wrap_enabled);
            let edit_style = if word_wrap_on {
                WINDOW_STYLE(base)
            } else {
                WINDOW_STYLE(base | ES_AUTOHSCROLL | WS_HSCROLL.0)
            };

            let hwnd_edit_new = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                PCWSTR::null(),
                edit_style,
                0,
                0,
                0,
                0,
                hwnd,
                HMENU(IDC_EDIT as *mut c_void),
                hinstance,
                None,
            )
            .unwrap_or(HWND(std::ptr::null_mut()));

            HWND_EDIT_RAW.store(hwnd_edit_new.0 as usize, Ordering::Relaxed);
            SendMessageW(hwnd_edit_new, EM_SETLIMITTEXT, WPARAM(0), LPARAM(0));
            apply_initial_font(hwnd_edit_new);

            let hwnd_status_new = status_bar::create_status_bar(hwnd, hinstance);
            HWND_STATUS_RAW.store(hwnd_status_new.0 as usize, Ordering::Relaxed);
            if !STATE.with(|s| s.borrow().status_bar_visible) {
                ShowWindow(hwnd_status_new, SW_HIDE);
            }

            status_bar::subclass_edit(hwnd_edit_new);
            menu::create_menu(hwnd);
            status_bar::update_status_bar();
            DragAcceptFiles(hwnd, BOOL(1));

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
                0,
                0,
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
                // Per-keystroke hot path: only flip clean→dirty once and only
                // then touch the title bar (update_title is cached as well).
                let became_dirty = STATE.with(|s| {
                    let mut st = s.borrow_mut();
                    if st.suppress_dirty || st.is_dirty {
                        false
                    } else {
                        st.is_dirty = true;
                        true
                    }
                });
                if became_dirty {
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
                if !f.is_invalid() {
                    let _ = DeleteObject(f);
                }
            }
            settings::save();
            PostQuitMessage(0);
            LRESULT(0)
        }

        WM_DROPFILES => {
            let hdrop = HDROP(wparam.0 as *mut c_void);
            let mut path_buf = vec![0u16; MAX_PATH as usize];
            let char_count = DragQueryFileW(hdrop, 0, Some(&mut path_buf));
            DragFinish(hdrop);

            if char_count > 0 {
                let path = String::from_utf16_lossy(&path_buf[..char_count as usize]);
                if menu::prompt_save_if_dirty(hwnd) {
                    file_io::load_file(hwnd, &path);
                }
            }
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn apply_initial_font(hwnd_edit: HWND) {
    use crate::state::pts_to_lfheight;
    unsafe {
        let (face_str, pts, weight, italic, charset) = STATE.with(|s| {
            let st = s.borrow();
            (
                st.current_font.face_name.clone(),
                st.current_font.height_pts,
                st.current_font.weight,
                st.current_font.italic,
                st.current_font.charset,
            )
        });

        let height = pts_to_lfheight(pts);
        let face: Vec<u16> = crate::state::wide_sz(&face_str);

        let hfont = windows::Win32::Graphics::Gdi::CreateFontW(
            height,
            0, 0, 0,
            weight,
            if italic { 1 } else { 0 },
            0, 0,
            charset as u32,
            0, 0, 0, 0,
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

        // Pointer args keep the selection 32-bit (return value truncates at 64K)
        let mut sel_start: u32 = 0;
        let mut sel_end: u32 = 0;
        SendMessageW(
            old_edit,
            EM_GETSEL,
            WPARAM(&mut sel_start as *mut u32 as usize),
            LPARAM(&mut sel_end as *mut u32 as isize),
        );

        status_bar::unsubclass_edit(old_edit);
        let _ = DestroyWindow(old_edit);
        HWND_EDIT_RAW.store(0, Ordering::Relaxed);

        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();
        let word_wrap = STATE.with(|s| s.borrow().word_wrap_enabled);

        let base =
            WS_CHILD.0 | WS_VISIBLE.0 | WS_VSCROLL.0 | ES_MULTILINE | ES_AUTOVSCROLL | ES_NOHIDESEL;

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
            0,
            0,
            0,
            0,
            hwnd,
            HMENU(IDC_EDIT as *mut c_void),
            hinstance,
            None,
        )
        .unwrap_or(HWND(std::ptr::null_mut()));

        HWND_EDIT_RAW.store(new_edit.0 as usize, Ordering::Relaxed);
        SendMessageW(new_edit, EM_SETLIMITTEXT, WPARAM(0), LPARAM(0));

        STATE.with(|s| s.borrow_mut().suppress_dirty = true);
        text_buf.push(0);
        SetWindowTextW(new_edit, PCWSTR(text_buf.as_ptr()));
        STATE.with(|s| s.borrow_mut().suppress_dirty = false);

        SendMessageW(
            new_edit,
            EM_SETSEL,
            WPARAM(sel_start as usize),
            LPARAM(sel_end as isize),
        );

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

        status_bar::update_status_bar();
        SetFocus(new_edit);
    }
}
