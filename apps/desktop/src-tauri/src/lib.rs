mod commands;

use commands::AppState;
use mvp_core::cache::store::CacheStore;
use std::sync::Mutex;
use tauri::Manager;

/// Install a SIGSEGV/SIGABRT handler that logs context before crashing.
/// Uses SA_SIGINFO to capture the faulting address for diagnostics.
#[cfg(target_os = "linux")]
fn install_crash_handler() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        unsafe extern "C" fn crash_handler(sig: libc::c_int, info: *mut libc::siginfo_t, _ctx: *mut libc::c_void) {
            let msg = match sig {
                libc::SIGSEGV => b"[CRASH] SIGSEGV - segmentation fault in MaxVideoPlayer.\n" as &[u8],
                libc::SIGABRT => b"[CRASH] SIGABRT - abort signal in MaxVideoPlayer.\n" as &[u8],
                _ => b"[CRASH] Fatal signal in MaxVideoPlayer.\n" as &[u8],
            };
            libc::write(2, msg.as_ptr() as *const _, msg.len());

            // Print faulting address (async-signal-safe: only write() and itoa).
            if !info.is_null() && sig == libc::SIGSEGV {
                let addr = (*info).si_addr() as usize;
                let mut buf = [0u8; 80];
                let prefix = b"[CRASH] Faulting address: 0x";
                buf[..prefix.len()].copy_from_slice(prefix);
                let mut pos = prefix.len();
                // Manual hex formatting (no allocator).
                let mut val = addr;
                let mut hex = [0u8; 16];
                let mut hlen = 0;
                if val == 0 {
                    hex[0] = b'0';
                    hlen = 1;
                } else {
                    while val > 0 && hlen < 16 {
                        let d = (val & 0xF) as u8;
                        hex[hlen] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
                        hlen += 1;
                        val >>= 4;
                    }
                    hex[..hlen].reverse();
                }
                buf[pos..pos + hlen].copy_from_slice(&hex[..hlen]);
                pos += hlen;
                buf[pos] = b'\n';
                pos += 1;
                libc::write(2, buf.as_ptr() as *const _, pos);
            }

            let advice = b"[CRASH] For a full stack trace, run:\n\
                           [CRASH]   gdb -ex run -ex bt -ex quit --args <your-command>\n\
                           [CRASH] Try: GDK_BACKEND=x11 or MVP_DISABLE_EMBEDDED_RENDERER=1\n" as &[u8];
            libc::write(2, advice.as_ptr() as *const _, advice.len());

            libc::kill(libc::getpid(), sig);
            libc::_exit(128 + sig);
        }

        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_flags = libc::SA_RESETHAND | libc::SA_SIGINFO;
        action.sa_sigaction = crash_handler as *const () as usize;
        libc::sigemptyset(&mut action.sa_mask);

        libc::sigaction(libc::SIGSEGV, &action, std::ptr::null_mut());
        libc::sigaction(libc::SIGABRT, &action, std::ptr::null_mut());
    });
}

/// Work around WebKit2GTK DMABUF renderer conflict with our EGL subsurface
/// rendering on Wayland. WebKit's DMABUF renderer shares GPU buffers in a way
/// that conflicts with our separate EGL context, causing an immediate SIGSEGV
/// when the first frame is rendered. Disabling it forces WebKit to use the
/// SHM (shared-memory) renderer, which composites correctly alongside our
/// wl_subsurface.
///
/// Only applied for **bundled** builds (AppImage, deb, rpm) — NOT dev mode.
/// In dev mode, WebKit's DMABUF renderer works fine and disabling it causes
/// an opaque/black WebView (SHM doesn't support RGBA transparency well) plus
/// increased latency.
#[cfg(target_os = "linux")]
fn apply_linux_workarounds() {
    let wayland_up = std::env::var("WAYLAND_DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    let bundled = std::env::var("APPIMAGE").is_ok() || {
        std::env::current_exe()
            .map(|p| {
                let s = p.to_string_lossy();
                s.starts_with("/usr/") || s.starts_with("/opt/")
            })
            .unwrap_or(false)
    };

    if wayland_up && bundled && std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
        tracing::info!("[Linux] Wayland session (bundled build) — setting WEBKIT_DISABLE_DMABUF_RENDERER=1 \
                        (prevents EGL context conflict with WebKit DMABUF renderer)");
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    } else if wayland_up && !bundled {
        tracing::info!("[Linux] Wayland session (dev build) — DMABUF renderer left enabled");
    }

    // Determine which GDK backend to use, in priority order:
    //
    // 1. MVP_GDK_BACKEND — explicit user override (highest priority).
    //    Validated: if the named backend's display socket is absent the
    //    value is ignored and we fall through to auto-detection.
    //
    // 2. Auto-detect:
    //    - Dev builds: prefer Wayland when WAYLAND_DISPLAY is available.
    //    - Bundled builds (AppImage/deb): force X11 (via XWayland) on
    //      Wayland sessions. Bundled builds ship their own GLib/GTK, which
    //      can ABI-conflict with the system's Mesa GPU driver on Wayland,
    //      causing SIGSEGV in the GL render pipeline. X11 (via XWayland)
    //      uses a simpler driver path that avoids this conflict.
    //      Users can opt back into Wayland with MVP_GDK_BACKEND=wayland.
    let wayland_up = std::env::var("WAYLAND_DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    let x11_up = std::env::var("DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    let explicit = std::env::var("MVP_GDK_BACKEND").ok().filter(|v| !v.is_empty());
    let resolved = match explicit.as_deref() {
        Some("wayland") if wayland_up => {
            tracing::info!("[Linux] MVP_GDK_BACKEND=wayland (user override, valid)");
            Some("wayland")
        }
        Some("wayland") => {
            tracing::warn!("[Linux] MVP_GDK_BACKEND=wayland but WAYLAND_DISPLAY unset — ignoring");
            None
        }
        Some("x11") if x11_up => {
            tracing::info!("[Linux] MVP_GDK_BACKEND=x11 (user override, valid)");
            Some("x11")
        }
        Some("x11") => {
            tracing::warn!("[Linux] MVP_GDK_BACKEND=x11 but DISPLAY unset — ignoring");
            None
        }
        Some(v) => {
            tracing::warn!("[Linux] MVP_GDK_BACKEND={v} unrecognised — ignoring");
            None
        }
        None => {
            if wayland_up && bundled && x11_up {
                tracing::info!(
                    "[Linux] Bundled build on Wayland — forcing GDK_BACKEND=x11 (XWayland) \
                     to avoid GLib ABI conflicts with system GPU drivers. \
                     Set MVP_GDK_BACKEND=wayland to override."
                );
                Some("x11")
            } else if wayland_up && !bundled {
                Some("wayland")
            } else {
                None
            }
        }
    };

    if let Some(backend) = resolved {
        std::env::set_var("GDK_BACKEND", backend);
        tracing::info!("[Linux] GDK_BACKEND set to {backend}");
    }
}

/// Log system display environment info for diagnostics.
#[cfg(target_os = "linux")]
fn log_display_environment() {
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into());
    let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "unset".into());
    let x11_display = std::env::var("DISPLAY").unwrap_or_else(|_| "unset".into());
    let gdk_backend = std::env::var("GDK_BACKEND").unwrap_or_else(|_| "auto".into());
    let disable_embedded = std::env::var("MVP_DISABLE_EMBEDDED_RENDERER").unwrap_or_else(|_| "0".into());
    let webkit_dmabuf = std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").unwrap_or_else(|_| "unset".into());

    let bundled = std::env::var("APPIMAGE").is_ok() || {
        std::env::current_exe()
            .map(|p| {
                let s = p.to_string_lossy();
                s.starts_with("/usr/") || s.starts_with("/opt/")
            })
            .unwrap_or(false)
    };

    tracing::info!(
        "[diagnostics] session={} wayland={} x11={} gdk_backend={} disable_embedded={} webkit_dmabuf={} bundled={}",
        session_type, wayland_display, x11_display, gdk_backend, disable_embedded, webkit_dmabuf, bundled
    );
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("max_video_player=debug".parse().unwrap())
                .add_directive("tauri_plugin_mpv=debug".parse().unwrap())
                .add_directive("mvp_core=debug".parse().unwrap()),
        )
        .init();

    #[cfg(target_os = "linux")]
    {
        apply_linux_workarounds();
        install_crash_handler();
        log_display_environment();
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_mpv::init())
        .invoke_handler(tauri::generate_handler![
            commands::load_m3u_playlist,
            commands::load_m3u_file,
            commands::load_xtream_provider,
            commands::get_providers,
            commands::remove_provider,
            commands::get_all_channels,
            commands::toggle_favorite,
            commands::refresh_provider,
            commands::update_provider,
            commands::get_xtream_series_episodes,
            commands::refresh_epg,
            commands::get_epg_programmes,
            commands::get_epg_for_live_channels,
            commands::search_epg_programmes,
            commands::set_epg_url,
            commands::detect_epg_url,
            commands::get_omdb_api_key,
            commands::set_omdb_api_key,
            commands::fetch_omdb_data,
            commands::get_mdblist_api_key,
            commands::set_mdblist_api_key,
            commands::test_mdblist_api_key,
            commands::fetch_mdblist_data,
            commands::fetch_whatson_data,
            commands::get_opensubtitles_api_key,
            commands::set_opensubtitles_api_key,
            commands::test_opensubtitles_api_key,
            commands::search_subtitles,
            commands::download_subtitle,
            commands::read_subtitle_file,
            commands::record_play_start,
            commands::record_play_end,
            commands::get_watch_history,
            commands::delete_history_entry,
            commands::clear_watch_history,
            commands::get_group_hierarchy,
            commands::update_group_hierarchy_entry,
            commands::delete_group_hierarchy,
            commands::pin_group,
            commands::unpin_group,
            commands::get_pinned_groups,
            commands::get_gemini_api_key,
            commands::set_gemini_api_key,
            commands::test_gemini_api_key,
            commands::categorize_provider,
            commands::clear_all_caches,
            commands::reorder_group_hierarchy_entry,
            commands::fix_uncategorized_groups,
            commands::rename_super_category,
            commands::delete_super_category,
            commands::get_install_info,
            commands::package_update,
        ])
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            let db_path = app_dir.join("maxvideoplayer.db");
            tracing::info!("Database path: {}", db_path.display());

            let cache = CacheStore::open(&db_path)
                .expect("failed to open database");

            app.manage(AppState {
                cache: Mutex::new(cache),
            });

            // Set the WebView's native background to fully transparent so the
            // video surface (positioned below the WebView) is visible through it.
            // CSS `background: transparent` alone is not sufficient on all Linux
            // compositors (e.g. Pop!_OS/COSMIC with WebKit2GTK DMABUF renderer).
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            if let Some(ref window) = app.get_webview_window("main") {
                use tauri::webview::Color;
                if let Err(e) = window.set_background_color(Some(Color(0, 0, 0, 0))) {
                    tracing::warn!("Failed to set WebView background transparent: {}", e);
                }
            }

            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }

            #[cfg(any(target_os = "macos", target_os = "linux"))]
            {
                if let Some(window) = app.get_webview_window("main") {
                    let handle = app.handle().clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::Resized(size) = event {
                            if let Some(state) = handle.try_state::<tauri_plugin_mpv::MpvState>() {
                                state.resize(size.width, size.height);
                            }
                        }
                    });
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
