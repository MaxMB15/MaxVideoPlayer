const COMMANDS: &[&str] = &[
    "mpv_load",
    "mpv_play",
    "mpv_pause",
    "mpv_stop",
    "mpv_seek",
    "mpv_set_volume",
    "mpv_get_state",
];

fn main() {
    link_libmpv();
    tauri_plugin::Builder::new(COMMANDS).build();
}

/// Configure linking to libmpv from libs/<platform>/ or pkg-config.
fn link_libmpv() {
    // Allow override via environment variable
    if let Ok(dir) = std::env::var("MPV_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", dir);
        return;
    }

    #[cfg(target_os = "macos")]
    {
        // Try libs/macos/ at workspace root
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let libs_macos = std::path::Path::new(&manifest_dir)
            .join("..")
            .join("..")
            .join("libs")
            .join("macos");
        if libs_macos.join("libmpv.dylib").exists()
            || libs_macos.join("libmpv.2.dylib").exists()
        {
            if let Ok(abs) = libs_macos.canonicalize() {
                println!("cargo:rustc-link-search=native={}", abs.display());
            } else {
                println!("cargo:rustc-link-search=native={}", libs_macos.display());
            }
            return;
        }
        // Fallback: Homebrew path (for development)
        if let Ok(brew_prefix) = std::process::Command::new("brew")
            .args(["--prefix", "mpv"])
            .output()
        {
            if brew_prefix.status.success() {
                let prefix = String::from_utf8_lossy(&brew_prefix.stdout).trim().to_string();
                let lib_dir = std::path::Path::new(&prefix).join("lib");
                if lib_dir.exists() {
                    println!("cargo:rustc-link-search=native={}", lib_dir.display());
                    return;
                }
            }
        }
        println!("cargo:warning=libmpv not found. Run ./scripts/build-libmpv.sh macos or install mpv via Homebrew.");
    }

    #[cfg(target_os = "linux")]
    {
        // Use pkg-config if available; libmpv2-sys may handle this, but we ensure the link
        if let Ok(output) = std::process::Command::new("pkg-config")
            .args(["--libs", "--cflags", "libmpv"])
            .output()
        {
            if output.status.success() {
                let args = String::from_utf8_lossy(&output.stdout);
                for arg in args.split_whitespace() {
                    if arg.starts_with("-L") {
                        println!("cargo:rustc-link-search=native={}", &arg[2..]);
                    }
                }
                return;
            }
        }
        // Fallback: libs/linux/ at workspace root
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let libs_linux = std::path::Path::new(&manifest_dir)
            .join("..")
            .join("..")
            .join("libs")
            .join("linux");
        if libs_linux.join("libmpv.so").exists() {
            println!("cargo:rustc-link-search=native={}", libs_linux.display());
            return;
        }
    }

    #[cfg(target_os = "windows")]
    {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let libs_win = std::path::Path::new(&manifest_dir)
            .join("..")
            .join("..")
            .join("libs")
            .join("windows");
        if libs_win.join("mpv.dll").exists() || libs_win.join("mpv-2.dll").exists() {
            println!("cargo:rustc-link-search=native={}", libs_win.display());
            return;
        }
    }
}
