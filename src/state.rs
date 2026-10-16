use std::cell::RefCell;
use windows::Win32::Graphics::Gdi::{GetDC, GetDeviceCaps, ReleaseDC, HFONT, LOGPIXELSY};

/// Encode a Rust string as UTF-16 *with* a null terminator, pre-sized so the
/// `push(0)` never reallocs. Replaces the `encode_utf16().collect()` + `push(0)`
/// pattern that used to allocate twice.
// (UTF-8 byte length is always >= UTF-16 code-unit count, so this capacity
// never reallocs.)
#[inline]
pub fn wide_sz(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = Vec::with_capacity(s.len() + 1);
    v.extend(s.encode_utf16());
    v.push(0);
    v
}

/// Same as `wide_sz` but without the null terminator (e.g. needle buffers).
#[inline]
pub fn wide(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = Vec::with_capacity(s.len());
    v.extend(s.encode_utf16());
    v
}

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
    pub search_match_case: bool,
    pub search_whole_word: bool,
    // ── repaint caches (avoids SetWindowTextW / SB_SETTEXT on hot paths) ──
    /// Last UTF-16 title actually pushed to the title bar.
    pub title_cache: Vec<u16>,
    /// Last "Ln X, Col Y" values actually pushed to the status bar.
    pub status_ln: i32,
    pub status_col: i32,
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
            search_match_case: false,
            search_whole_word: false,
            title_cache: Vec::new(),
            status_ln: -1,
            status_col: -1,
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
