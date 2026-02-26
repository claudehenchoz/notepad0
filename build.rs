fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        use embed_manifest::{embed_manifest, new_manifest};
        use embed_manifest::manifest::DpiAwareness;
        let manifest = new_manifest("notepad0.Application")
            .dpi_awareness(DpiAwareness::PerMonitorV2);
        embed_manifest(manifest).expect("failed to embed manifest");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
