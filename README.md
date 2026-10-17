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
- Open files from the command line or by drag and drop
- PerMonitorV2 DPI awareness

## Download

Each [release](https://github.com/claudehenchoz/notepad0/releases) has two files:

- `notepad0-<version>-setup.exe`: one-click per-user installer (no admin rights, no prompts). It adds a Start menu entry, makes `notepad0` work from Win+R, and adds Notepad Zero to the right-click **Open with** menu for all files. Uninstall from Settings > Apps.
- `notepad0-<version>-portable.zip`: the exe plus LICENSE; run it from anywhere.

## Building

Requires Rust (stable) and a Windows MSVC toolchain.

```
cargo build --release
```

The release binary is around 250 KB with no runtime dependencies beyond standard Windows DLLs.

## Usage

Run `notepad0.exe` directly, or associate `.txt` files with it. There are three ways to open a file:

- **Command line:** `notepad0.exe C:\notes\todo.txt` (absolute or relative paths, spaces allowed when quoted)
- **Drag onto the exe:** dropping a file on `notepad0.exe` or a shortcut to it launches the app with that file
- **Drag onto the window:** dropping a file on a running instance opens it, prompting to save unsaved changes first

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

Copyright (C) 2026 Claude Henchoz.

Notepad0 is free software: you can redistribute it and/or modify it under the terms of the
GNU General Public License as published by the Free Software Foundation, either version 3 of
the License, or (at your option) any later version. See [LICENSE](LICENSE) for the full text.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
