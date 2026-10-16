use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Controls::Dialogs::{
    PageSetupDlgW, PrintDlgW, PAGESETUPDLGW, PRINTDLGW, PD_RETURNDC,
};
use windows::Win32::UI::Shell::ShellAboutW;
use windows::Win32::System::DataExchange::IsClipboardFormatAvailable;
// CF_UNICODETEXT = 13
const CF_UNICODETEXT_RAW: u32 = 13;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CheckMenuItem, CreateMenu, CreatePopupMenu, EnableMenuItem,
    MessageBoxW, PostMessageW, SendMessageW, SetMenu,
    MB_ICONQUESTION, MB_OK, MF_BYCOMMAND, MF_CHECKED,
    MF_ENABLED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED,
    WM_CLEAR, WM_CLOSE, WM_COPY, WM_CUT, WM_PASTE, HMENU,
};

use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;

use crate::state::STATE;
use crate::{file_io, dialogs, status_bar, hwnd_edit, recreate_edit};

// ─── Menu IDs ─────────────────────────────────────────────────────────────────
pub const FILE_NEW: u32 = 101;
pub const FILE_OPEN: u32 = 102;
pub const FILE_SAVE: u32 = 103;
pub const FILE_SAVEAS: u32 = 104;
pub const FILE_PAGESETUP: u32 = 105;
pub const FILE_PRINT: u32 = 106;
pub const FILE_EXIT: u32 = 107;

pub const EDIT_UNDO: u32 = 201;
pub const EDIT_CUT: u32 = 202;
pub const EDIT_COPY: u32 = 203;
pub const EDIT_PASTE: u32 = 204;
pub const EDIT_DELETE: u32 = 205;
pub const EDIT_FIND: u32 = 206;
pub const EDIT_FINDNEXT: u32 = 207;
pub const EDIT_REPLACE: u32 = 208;
pub const EDIT_GOTO: u32 = 209;
pub const EDIT_SELECTALL: u32 = 210;
pub const EDIT_TIMEDATE: u32 = 211;

pub const FMT_WORDWRAP: u32 = 301;
pub const FMT_FONT: u32 = 302;

pub const VIEW_STATUSBAR: u32 = 401;

pub const HELP_HELP: u32 = 501;
pub const HELP_ABOUT: u32 = 502;

// Edit control message constants
const EM_CANUNDO: u32 = 0x00C6;
const EM_GETSEL: u32 = 0x00B0;
const EM_SETSEL: u32 = 0x00B1;
const EM_UNDO: u32 = 0x00C7;
const EM_REPLACESEL: u32 = 0x00C2;

/// Build and attach the full Win7-style menu bar
pub fn create_menu(hwnd: HWND) {
    unsafe {
        let hmenu = CreateMenu().unwrap();

        // File menu
        let file_menu = CreatePopupMenu().unwrap();
        AppendMenuW(file_menu, MF_STRING, FILE_NEW as usize, w!("&New\tCtrl+N"));
        AppendMenuW(file_menu, MF_STRING, FILE_OPEN as usize, w!("&Open...\tCtrl+O"));
        AppendMenuW(file_menu, MF_STRING, FILE_SAVE as usize, w!("&Save\tCtrl+S"));
        AppendMenuW(file_menu, MF_STRING, FILE_SAVEAS as usize, w!("Save &As..."));
        AppendMenuW(file_menu, MF_SEPARATOR, 0, PCWSTR::null());
        AppendMenuW(file_menu, MF_STRING, FILE_PAGESETUP as usize, w!("Page Se&tup..."));
        AppendMenuW(file_menu, MF_STRING, FILE_PRINT as usize, w!("&Print...\tCtrl+P"));
        AppendMenuW(file_menu, MF_SEPARATOR, 0, PCWSTR::null());
        AppendMenuW(file_menu, MF_STRING, FILE_EXIT as usize, w!("E&xit"));
        AppendMenuW(hmenu, MF_POPUP, file_menu.0 as usize, w!("&File"));

        // Edit menu
        let edit_menu = CreatePopupMenu().unwrap();
        AppendMenuW(edit_menu, MF_STRING, EDIT_UNDO as usize, w!("&Undo\tCtrl+Z"));
        AppendMenuW(edit_menu, MF_SEPARATOR, 0, PCWSTR::null());
        AppendMenuW(edit_menu, MF_STRING, EDIT_CUT as usize, w!("Cu&t\tCtrl+X"));
        AppendMenuW(edit_menu, MF_STRING, EDIT_COPY as usize, w!("&Copy\tCtrl+C"));
        AppendMenuW(edit_menu, MF_STRING, EDIT_PASTE as usize, w!("&Paste\tCtrl+V"));
        AppendMenuW(edit_menu, MF_STRING, EDIT_DELETE as usize, w!("De&lete\tDel"));
        AppendMenuW(edit_menu, MF_SEPARATOR, 0, PCWSTR::null());
        AppendMenuW(edit_menu, MF_STRING, EDIT_FIND as usize, w!("&Find...\tCtrl+F"));
        AppendMenuW(edit_menu, MF_STRING, EDIT_FINDNEXT as usize, w!("Find &Next\tF3"));
        AppendMenuW(edit_menu, MF_STRING, EDIT_REPLACE as usize, w!("&Replace...\tCtrl+H"));
        AppendMenuW(edit_menu, MF_STRING, EDIT_GOTO as usize, w!("&Go To...\tCtrl+G"));
        AppendMenuW(edit_menu, MF_SEPARATOR, 0, PCWSTR::null());
        AppendMenuW(edit_menu, MF_STRING, EDIT_SELECTALL as usize, w!("Select &All\tCtrl+A"));
        AppendMenuW(edit_menu, MF_STRING, EDIT_TIMEDATE as usize, w!("Time/&Date\tF5"));
        AppendMenuW(hmenu, MF_POPUP, edit_menu.0 as usize, w!("&Edit"));

        // Format menu
        let fmt_menu = CreatePopupMenu().unwrap();
        AppendMenuW(fmt_menu, MF_STRING, FMT_WORDWRAP as usize, w!("&Word Wrap"));
        AppendMenuW(fmt_menu, MF_STRING, FMT_FONT as usize, w!("&Font..."));
        AppendMenuW(hmenu, MF_POPUP, fmt_menu.0 as usize, w!("F&ormat"));

        // View menu
        let view_menu = CreatePopupMenu().unwrap();
        AppendMenuW(view_menu, MF_STRING, VIEW_STATUSBAR as usize, w!("&Status Bar"));
        AppendMenuW(hmenu, MF_POPUP, view_menu.0 as usize, w!("&View"));

        // Help menu
        let help_menu = CreatePopupMenu().unwrap();
        AppendMenuW(help_menu, MF_STRING, HELP_HELP as usize, w!("View &Help"));
        AppendMenuW(help_menu, MF_SEPARATOR, 0, PCWSTR::null());
        AppendMenuW(help_menu, MF_STRING, HELP_ABOUT as usize, w!("&About Notepad Zero"));
        AppendMenuW(hmenu, MF_POPUP, help_menu.0 as usize, w!("&Help"));

        SetMenu(hwnd, hmenu);
    }
}

/// WM_INITMENUPOPUP: gray/check items based on current state
pub fn on_init_menu_popup(_hwnd: HWND, hmenu: HMENU) {
    unsafe {
        let edit = hwnd_edit();

        // Undo
        let can_undo = SendMessageW(edit, EM_CANUNDO, WPARAM(0), LPARAM(0)).0 != 0;
        EnableMenuItem(hmenu, EDIT_UNDO, MF_BYCOMMAND | if can_undo { MF_ENABLED } else { MF_GRAYED });

        // Selection-dependent items
        let sel = SendMessageW(edit, EM_GETSEL, WPARAM(0), LPARAM(0)).0 as u64;
        let sel_start = (sel & 0xFFFF) as u32;
        let sel_end = ((sel >> 16) & 0xFFFF) as u32;
        let has_sel = sel_start != sel_end;

        let sel_flag = MF_BYCOMMAND | if has_sel { MF_ENABLED } else { MF_GRAYED };
        EnableMenuItem(hmenu, EDIT_CUT, sel_flag);
        EnableMenuItem(hmenu, EDIT_COPY, sel_flag);
        EnableMenuItem(hmenu, EDIT_DELETE, sel_flag);

        // Paste
        let can_paste = IsClipboardFormatAvailable(CF_UNICODETEXT_RAW).is_ok();
        EnableMenuItem(
            hmenu,
            EDIT_PASTE,
            MF_BYCOMMAND | if can_paste { MF_ENABLED } else { MF_GRAYED },
        );

        // Word Wrap checkmark
        let (word_wrap, status_bar_vis) = STATE.with(|s| {
            let st = s.borrow();
            (st.word_wrap_enabled, st.status_bar_visible)
        });

        CheckMenuItem(hmenu, FMT_WORDWRAP, (MF_BYCOMMAND | if word_wrap { MF_CHECKED } else { MF_UNCHECKED }).0);
        CheckMenuItem(hmenu, VIEW_STATUSBAR, (MF_BYCOMMAND | if status_bar_vis { MF_CHECKED } else { MF_UNCHECKED }).0);
    }
}

/// Dispatch WM_COMMAND menu/accelerator IDs
pub fn handle_command(hwnd: HWND, id: u32) {
    match id {
        FILE_NEW => file_new(hwnd),
        FILE_OPEN => file_open(hwnd),
        FILE_SAVE => { file_io::save_file(hwnd, false); }
        FILE_SAVEAS => { file_io::save_file(hwnd, true); }
        FILE_PAGESETUP => page_setup(hwnd),
        FILE_PRINT => print_doc(hwnd),
        FILE_EXIT => unsafe { PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)); }

        EDIT_UNDO => unsafe { SendMessageW(hwnd_edit(), EM_UNDO, WPARAM(0), LPARAM(0)); }
        EDIT_CUT => unsafe { SendMessageW(hwnd_edit(), WM_CUT, WPARAM(0), LPARAM(0)); }
        EDIT_COPY => unsafe { SendMessageW(hwnd_edit(), WM_COPY, WPARAM(0), LPARAM(0)); }
        EDIT_PASTE => unsafe { SendMessageW(hwnd_edit(), WM_PASTE, WPARAM(0), LPARAM(0)); }
        EDIT_DELETE => unsafe { SendMessageW(hwnd_edit(), WM_CLEAR, WPARAM(0), LPARAM(0)); }

        EDIT_FIND => dialogs::open_find(hwnd),
        EDIT_FINDNEXT => dialogs::find_next(hwnd, true),
        EDIT_REPLACE => dialogs::open_replace(hwnd),
        EDIT_GOTO => dialogs::open_goto(hwnd),

        EDIT_SELECTALL => unsafe {
            SendMessageW(hwnd_edit(), EM_SETSEL, WPARAM(0), LPARAM(-1_isize));
        }
        EDIT_TIMEDATE => insert_time_date(),

        FMT_WORDWRAP => {
            STATE.with(|s| {
                let mut st = s.borrow_mut();
                st.word_wrap_enabled = !st.word_wrap_enabled;
            });
            recreate_edit(hwnd);
        }
        FMT_FONT => dialogs::choose_font(hwnd),

        VIEW_STATUSBAR => status_bar::toggle(hwnd),

        HELP_HELP => unsafe {
            MessageBoxW(
                hwnd,
                w!("Notepad Zero \u{2014} a minimal Notepad clone.\n\nNo built-in help topics."),
                w!("Help"),
                MB_OK,
            );
        }
        HELP_ABOUT => unsafe {
            ShellAboutW(
                hwnd,
                w!("Notepad Zero"),
                w!("A minimal Win32 Notepad clone written in Rust."),
                None,
            );
        }

        _ => {}
    }
}

fn file_new(hwnd: HWND) {
    if !prompt_save_if_dirty(hwnd) {
        return;
    }
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::SetWindowTextW;
        STATE.with(|s| s.borrow_mut().suppress_dirty = true);
        SetWindowTextW(hwnd_edit(), PCWSTR::null());
        STATE.with(|s| {
            let mut st = s.borrow_mut();
            st.suppress_dirty = false;
            st.is_dirty = false;
            st.current_file_path = None;
        });
        SetFocus(hwnd_edit());
    }
    file_io::update_title(hwnd);
}

fn file_open(hwnd: HWND) {
    if !prompt_save_if_dirty(hwnd) {
        return;
    }
    if let Some(path) = file_io::open_file_dialog(hwnd) {
        file_io::load_file(hwnd, &path);
        unsafe { SetFocus(hwnd_edit()); }
    }
}

/// Returns true if OK to proceed (not dirty, or user saved/discarded).
/// Returns false if user cancelled.
pub fn prompt_save_if_dirty(hwnd: HWND) -> bool {
    let (dirty, path) = STATE.with(|s| {
        let st = s.borrow();
        (st.is_dirty, st.current_file_path.clone())
    });
    if !dirty {
        return true;
    }

    let file_name = path
        .as_deref()
        .and_then(|p| std::path::Path::new(p).file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled");

    let msg = crate::state::wide_sz(&format!("Do you want to save changes to {}?", file_name));

    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            MB_YESNOCANCEL, IDYES, IDNO,
        };
        let result = MessageBoxW(
            hwnd,
            PCWSTR(msg.as_ptr()),
            w!("Notepad Zero"),
            MB_ICONQUESTION | MB_YESNOCANCEL,
        );
        if result == IDYES {
            file_io::save_file(hwnd, false)
        } else if result == IDNO {
            true
        } else {
            false // Cancel
        }
    }
}

fn insert_time_date() {
    unsafe {
        let st = GetLocalTime();

        let text = crate::state::wide_sz(&format!(
            "{:02}:{:02} {}/{}/{}",
            st.wHour, st.wMinute, st.wMonth, st.wDay, st.wYear
        ));

        SendMessageW(
            hwnd_edit(),
            EM_REPLACESEL,
            WPARAM(1),
            LPARAM(text.as_ptr() as isize),
        );
    }
}

fn page_setup(hwnd: HWND) {
    unsafe {
        let mut ps: PAGESETUPDLGW = std::mem::zeroed();
        ps.lStructSize = std::mem::size_of::<PAGESETUPDLGW>() as u32;
        ps.hwndOwner = hwnd;
        let _ = PageSetupDlgW(&mut ps);
    }
}

fn print_doc(hwnd: HWND) {
    unsafe {
        let mut pd: PRINTDLGW = std::mem::zeroed();
        pd.lStructSize = std::mem::size_of::<PRINTDLGW>() as u32;
        pd.hwndOwner = hwnd;
        pd.Flags = PD_RETURNDC;
        let _ = PrintDlgW(&mut pd);
    }
}
