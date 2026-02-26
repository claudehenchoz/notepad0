# Notepad Zero

A plain text editor for Windows, written in Rust. It reproduces the feature set of Windows 7 Notepad — no tabs, no AI, no markdown preview.

## Why

Windows 11 Notepad added tabs and a Copilot button. This is a replacement that behaves like the old one.

## Features

- Open, save, save as (UTF-8 output; detects UTF-8/UTF-16/Windows-1252 on read)
- Find, Find Next (F3), Replace, Go To Line
- Word wrap toggle
- Font selection
- Status bar showing line and column number
- Time/Date insert (F5)
- Dirty-state tracking with save prompt on close
- PerMonitorV2 DPI awareness

## Building

Requires Rust (stable) and a Windows MSVC toolchain.

```
cargo build --release
```

The release binary is around 360 KB with no runtime dependencies beyond standard Windows DLLs.

## Usage

Run `notepad0.exe` directly or associate `.txt` files with it. It accepts a file path as a command-line argument (drag and drop onto the exe works).

> **Note:** Command-line argument file opening is not yet implemented. Open files via File > Open or drag and drop onto a running instance is not yet implemented either. Use File > Open.

## Keyboard shortcuts

These match Windows 7 Notepad exactly.

| Action | Shortcut |
|---|---|
| New | Ctrl+N |
| Open | Ctrl+O |
| Save | Ctrl+S |
| Print | Ctrl+P |
| Undo | Ctrl+Z |
| Find | Ctrl+F |
| Find Next | F3 |
| Replace | Ctrl+H |
| Go To | Ctrl+G |
| Select All | Ctrl+A |
| Time/Date | F5 |

## License

Unlicensed / public domain. Do what you want with it.
