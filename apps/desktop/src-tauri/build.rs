fn main() {
    #[cfg(target_os = "macos")]
    embed_macos_rpath();

    tauri_build::build();
}

/// Bake an rpath into the binary so it finds libmpv.2.dylib in libs/macos/
/// at runtime without requiring DYLD_LIBRARY_PATH.
#[cfg(target_os = "macos")]
fn embed_macos_rpath() {
    let libs_macos = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../libs/macos");

    match libs_macos.canonicalize() {
        Ok(abs) => {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", abs.display());
        }
        Err(_) => {
            eprintln!(
                "cargo:warning=libs/macos/ not found — run ./scripts/build-libmpv.sh macos first"
            );
        }
    }
}
