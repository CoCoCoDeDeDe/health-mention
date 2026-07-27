mod settings;
mod tray;

use settings::Settings;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};
use tauri_plugin_autostart::ManagerExt;

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    state: State<AppState>,
    settings: Settings,
) -> Result<(), String> {
    settings.validate()?;
    settings::save(&state.settings_path, &settings)?;

    // 跟随系统自启动：即时生效
    let autolaunch = app.autolaunch();
    if settings.global.autostart {
        autolaunch.enable().map_err(|e| e.to_string())?;
    } else {
        autolaunch.disable().map_err(|e| e.to_string())?;
    }

    *state.settings.lock().unwrap() = settings;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let dir = app.path().app_data_dir().expect("app data dir unavailable");
            std::fs::create_dir_all(&dir).ok();
            let settings_path = dir.join("settings.json");
            let settings = settings::load(&settings_path);
            app.manage(AppState {
                settings: Mutex::new(settings),
                settings_path,
            });
            tray::setup_tray(&app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // 关闭主窗口时隐藏到托盘，不退出
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![get_settings, save_settings])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
