use std::cell::RefCell;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{GetDC, GetDeviceCaps, ReleaseDC, HFONT, LOGPIXELSY};

pub struct FontData {
    pub face_name: String,
    pub height_pts: i32,
    pub weight: i32,
    pub italic: bool,
    pub charset: u8,
}

impl Default for FontData {
    fn default() -> Self {
        FontData {
            face_name: "Courier New".to_string(),
            height_pts: 10,
            weight: 400,
            italic: false,
            charset: 1, // DEFAULT_CHARSET
        }
    }
}

pub struct AppState {
    pub current_file_path: Option<String>,
    pub is_dirty: bool,
    pub suppress_dirty: bool,
    pub word_wrap_enabled: bool,
    pub status_bar_visible: bool,
    pub current_font: FontData,
    pub hfont: Option<HFONT>,
    pub last_search: String,
    pub last_replace: String,
    pub find_replace_hwnd: Option<HWND>,
    pub search_match_case: bool,
    pub search_whole_word: bool,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            current_file_path: None,
            is_dirty: false,
            suppress_dirty: false,
            word_wrap_enabled: false,
            status_bar_visible: true,
            current_font: FontData::default(),
            hfont: None,
            last_search: String::new(),
            last_replace: String::new(),
            find_replace_hwnd: None,
            search_match_case: false,
            search_whole_word: false,
        }
    }
}

thread_local! {
    pub static STATE: RefCell<AppState> = RefCell::new(AppState::default());
}

/// Convert point size to negative LOGFONT lfHeight using screen DPI
pub fn pts_to_lfheight(pts: i32) -> i32 {
    unsafe {
        let hdc = GetDC(None); // screen DC (shared, released with ReleaseDC)
        let dpi = GetDeviceCaps(hdc, LOGPIXELSY);
        ReleaseDC(None, hdc);
        if dpi == 0 { return -pts; }
        -((pts * dpi + 36) / 72)
    }
}

/// Convert absolute lfHeight magnitude back to point size
pub fn lfheight_to_pts(lf_height_abs: i32) -> i32 {
    unsafe {
        let hdc = GetDC(None);
        let dpi = GetDeviceCaps(hdc, LOGPIXELSY);
        ReleaseDC(None, hdc);
        if dpi == 0 { return lf_height_abs; }
        (lf_height_abs * 72 + dpi / 2) / dpi
    }
}
