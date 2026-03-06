mod commands;

use commands::AppState;
use mvp_core::cache::store::CacheStore;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("max_video_player=debug".parse().unwrap())
                .add_directive("tauri_plugin_mpv=debug".parse().unwrap()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_mpv::init())
        .invoke_handler(tauri::generate_handler![
            commands::load_m3u_playlist,
            commands::load_m3u_file,
            commands::load_xtream_provider,
            commands::get_providers,
            commands::remove_provider,
            commands::get_all_channels,
            commands::toggle_favorite,
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

            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
            }

            #[cfg(target_os = "macos")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    let handle = app.handle().clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::Resized(_) = event {
                            if let Some(state) = handle.try_state::<tauri_plugin_mpv::MpvState>() {
                                state.update_surface_frame();
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
