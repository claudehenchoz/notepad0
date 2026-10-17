# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build                  # debug build
cargo build --release        # release build (~358 KB stripped binary)
.\target\release\notepad0.exe
```

There are no tests or linting steps. The only CI is `cargo build`.

## Project Overview

A Windows 7-style Notepad clone written in pure Rust using the `windows` crate (v0.58) for raw Win32 API calls. No UI framework — everything is hand-crafted Win32. The binary is `#![windows_subsystem = "windows"]` (no console).

`build.rs` embeds a PerMonitorV2 DPI-awareness manifest via `embed-manifest` and the app icon (`res/notepad0.rc` → `res/notepad0.ico`, resource ID 1) via `embed-resource` (needs the Windows SDK `rc.exe`). The `.ico` is rendered from `res/notepad0.svg` (20–256 px) and `res/notepad0-small.svg` (16 px) with ImageMagick.

## Architecture

All state lives in a `thread_local! { STATE: RefCell<AppState> }` in `src/state.rs`. The three main window handles are stored as `AtomicUsize` globals in `src/main.rs` and accessed via `hwnd_main()`, `hwnd_edit()`, `hwnd_status()`.

**Module responsibilities:**
- `src/main.rs` — `WinMain`-equivalent, `RegisterClassExW`, message loop, `WndProc` (handles `WM_CREATE`, `WM_SIZE`, `WM_COMMAND`, `WM_INITMENUPOPUP`, `WM_CLOSE`, `WM_DESTROY`). Also owns `recreate_edit()` for word-wrap toggling.
- `src/state.rs` — `AppState` struct, `FontData` struct, `pts_to_lfheight`/`lfheight_to_pts` DPI helpers.
- `src/menu.rs` — Builds the full menu tree in `create_menu()`, updates checkmarks/grayed items in `on_init_menu_popup()`, dispatches `WM_COMMAND` IDs in `handle_command()`. Menu IDs are `pub const` values (100s=File, 200s=Edit, 300s=Format, 400s=View, 500s=Help).
- `src/file_io.rs` — Open/save dialogs (`GetOpenFileNameW`/`GetSaveFileNameW`), BOM detection + `encoding_rs` decoding (UTF-8 BOM → UTF-16 BOM → UTF-8 validation → Windows-1252 fallback), CRLF normalization, title bar update.
- `src/dialogs.rs` — Find (`FindTextW`), Replace (`ReplaceTextW`), GoTo (built from a hand-crafted `DLGTEMPLATE` byte buffer in `build_goto_template()`), Font (`ChooseFontW`). Find/Replace state is heap-pinned via `thread_local! { FIND_STATE: RefCell<Option<Box<FindReplaceState>>> }` so the pointer passed to `FindTextW` stays valid.
- `src/status_bar.rs` — Creates `msctls_statusbar32`, subclasses the EDIT control via `SetWindowSubclass` to track caret position for "Ln X, Col Y" display.

## Critical `windows = "0.58"` API Quirks

These are non-obvious and will cause compile errors if done wrong:

- **`HWND`/`HMENU` are `*mut c_void`** — store as `AtomicUsize` with `.0 as usize`; reconstruct with `HWND(v as *mut c_void)`.
- **`HBRUSH` for window background** — `HBRUSH(6usize as *mut c_void)` = `COLOR_WINDOW+1`.
- **`EN_CHANGE`, `EM_SETLIMITTEXT`, `EM_GETSEL`** etc. are not re-exported — use raw `u32` constants.
- **`FINDREPLACE_FLAGS` is a newtype** — test with `(flags & FR_DOWN).0 != 0`, not `flags & FR_DOWN.0 != 0`.
- **`SetFocus`** is in `Win32::UI::Input::KeyboardAndMouse`, not `WindowsAndMessaging`.
- **`IsClipboardFormatAvailable`** returns `Result<()>`; use `.is_ok()`. `CF_UNICODETEXT = 13u32`.
- **`GetLocalTime()`** takes no args in 0.58, returns `SYSTEMTIME`.
- **`SetWindowSubclass`** is in `Win32::UI::Shell`, not `Controls`.
- **`FONT_CHARSET`** is a newtype: `lf.lfCharSet = FONT_CHARSET(charset as _)`.
- **`CreateWindowExW`** returns `Result<HWND>` — use `.unwrap_or(HWND(null_mut()))`.

## Word Wrap

Word wrap cannot be toggled on the existing EDIT control because `ES_AUTOHSCROLL` cannot be changed post-creation. Instead, `recreate_edit()` in `main.rs` destroys the old EDIT, creates a new one with the correct style bits, copies text + selection, re-applies the font, and re-subclasses.

## Find/Replace Lifetime

The `FINDREPLACEW` struct and its string buffers must remain alive for the lifetime of the modeless dialog. They are owned by a `Box<FindReplaceState>` stored in `FIND_STATE` thread-local. The dialog's registered message (`FINDMSGSTRING`, stored in `FIND_MSG_ID: OnceLock<u32>`) is dispatched in the message loop before `TranslateMessage`/`DispatchMessageW`.
