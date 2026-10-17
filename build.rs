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

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        use embed_manifest::{embed_manifest, new_manifest};
        use embed_manifest::manifest::DpiAwareness;
        let manifest = new_manifest("notepad0.Application")
            .dpi_awareness(DpiAwareness::PerMonitorV2);
        embed_manifest(manifest).expect("failed to embed manifest");
        embed_resource::compile("res/notepad0.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("failed to compile resources");
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=res/notepad0.rc");
    println!("cargo:rerun-if-changed=res/notepad0.ico");
}
