use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, SendMessageW, ShowWindow, SW_HIDE, SW_SHOW,
    WM_CHAR, WM_KEYUP, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_SETTEXT, WM_SIZE,
    WS_CHILD, WS_VISIBLE, HMENU, WINDOW_EX_STYLE, WINDOW_STYLE,
};
use crate::state::STATE;
use crate::{hwnd_edit, hwnd_status};

const SUBCLASS_ID: usize = 1;

// Status bar message constants
const SB_SETTEXT_W: u32 = 0x0401; // WM_USER + 1

// SBARS_SIZEGRIP = 0x0100 (size grip on right side)
const SBARS_SIZEGRIP_VAL: u32 = 0x0100;

/// Create the msctls_statusbar32 child window
pub fn create_status_bar(hwnd_parent: HWND, hinst: HINSTANCE) -> HWND {
    let class: Vec<u16> = "msctls_statusbar32\0".encode_utf16().collect();
    unsafe {
        use std::ffi::c_void;
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | SBARS_SIZEGRIP_VAL),
            0, 0, 0, 0,
            hwnd_parent,
            HMENU(std::ptr::null_mut::<c_void>()),
            hinst,
            None,
        ).unwrap_or(HWND(std::ptr::null_mut()))
    }
}

/// Subclass the EDIT control for caret-position tracking
pub fn subclass_edit(hwnd_edit: HWND) {
    unsafe {
        let _ = SetWindowSubclass(hwnd_edit, Some(edit_subclass_proc), SUBCLASS_ID, 0);
    }
}

/// Remove the subclass (needed before destroying the EDIT on word wrap toggle)
pub fn unsubclass_edit(hwnd_edit: HWND) {
    unsafe {
        let _ = RemoveWindowSubclass(hwnd_edit, Some(edit_subclass_proc), SUBCLASS_ID);
    }
}

unsafe extern "system" fn edit_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid_subclass: usize,
    _ref_data: usize,
) -> LRESULT {
    let result = DefSubclassProc(hwnd, msg, wparam, lparam);
    if msg == WM_KEYUP
        || msg == WM_LBUTTONUP
        || msg == WM_LBUTTONDBLCLK
        || msg == WM_CHAR
        || msg == WM_SETTEXT
    {
        update_status_bar();
    }
    result
}

/// Update the status bar Ln/Col display
pub fn update_status_bar() {
    unsafe {
        let edit = hwnd_edit();
        let status = hwnd_status();

        if status.is_invalid() || edit.is_invalid() {
            return;
        }

        const EM_GETSEL: u32 = 0x00B0;
        const EM_LINEFROMCHAR: u32 = 0x00C9;
        const EM_LINEINDEX: u32 = 0x00BB;

        let sel = SendMessageW(edit, EM_GETSEL, WPARAM(0), LPARAM(0)).0 as u64;
        let char_index = (sel & 0xFFFF) as i32;

        let line_lresult = SendMessageW(
            edit,
            EM_LINEFROMCHAR,
            WPARAM(char_index as usize),
            LPARAM(0),
        );
        let line_num = line_lresult.0 as i32;

        let line_start_lresult = SendMessageW(
            edit,
            EM_LINEINDEX,
            WPARAM(line_num as usize),
            LPARAM(0),
        );
        let line_start = line_start_lresult.0 as i32;

        let col = char_index - line_start + 1;

        let text: Vec<u16> = format!("Ln {}, Col {}\0", line_num + 1, col)
            .encode_utf16()
            .collect();

        SendMessageW(
            status,
            SB_SETTEXT_W,
            WPARAM(0), // part 0
            LPARAM(text.as_ptr() as isize),
        );
    }
}

/// Toggle status bar visibility and force EDIT resize
pub fn toggle(hwnd_main_window: HWND) {
    let visible = STATE.with(|s| {
        let mut st = s.borrow_mut();
        st.status_bar_visible = !st.status_bar_visible;
        st.status_bar_visible
    });

    unsafe {
        ShowWindow(hwnd_status(), if visible { SW_SHOW } else { SW_HIDE });
        // Trigger WM_SIZE to resize EDIT control
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
        let mut rc = RECT::default();
        GetClientRect(hwnd_main_window, &mut rc);
        SendMessageW(
            hwnd_main_window,
            WM_SIZE,
            WPARAM(0),
            LPARAM(((rc.right as u64) | ((rc.bottom as u64) << 16)) as isize),
        );
    }
}
