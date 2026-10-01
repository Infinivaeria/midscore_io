// filepath: /c:/BashAssetFinal/rust/bash-asset-engine/build.rs
use std::path::PathBuf;

// Embeds the rustby-vm MD5 file. On Windows this is the original
// C:/BashAssetFinal-build artifact; elsewhere set RUSTBY_VM_MD5 to a file,
// or an empty placeholder is embedded so the crate still builds.
fn main() {
    println!("cargo:rerun-if-env-changed=RUSTBY_VM_MD5");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("rustby-vm.md5");

    let source = std::env::var("RUSTBY_VM_MD5").ok().or_else(|| {
        if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
            Some("C:/BashAssetFinal-build/rustby-vm.md5".to_string())
        } else {
            None
        }
    });

    match source {
        Some(path) => {
            println!("cargo:rerun-if-changed={path}");
            std::fs::copy(&path, &out)
                .unwrap_or_else(|e| panic!("failed to read rustby-vm md5 file {path}: {e}"));
        }
        None => {
            println!("cargo:warning=RUSTBY_VM_MD5 not set; embedding an empty rustby-vm.md5");
            std::fs::write(&out, b"").unwrap();
        }
    }
}
