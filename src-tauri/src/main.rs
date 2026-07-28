mod scheduler;
mod session;
mod settings;
mod storage;
mod tray;

use scheduler::Scheduler;
use session::{ActiveSession, BreakStatePayload};
use settings::Settings;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Manager, State};
use tauri_plugin_autostart::ManagerExt;

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    pub logs_dir: PathBuf,
    pub scheduler: Mutex<Scheduler>,
    pub session: Mutex<Option<ActiveSession>>,
    pub session_seq: Mutex<u64>,
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

    state.scheduler.lock().unwrap().reload(&settings);
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
fn trigger_break(app: tauri::AppHandle) {
    session::start_session(&app, "manual");
}

#[tauri::command]
fn end_break(app: tauri::AppHandle) {
    session::end_session(&app, "stopped");
}

#[tauri::command]
fn get_break_state(app: tauri::AppHandle) -> Option<BreakStatePayload> {
    session::get_break_state(&app)
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
                scheduler: Mutex::new(Scheduler::new(&settings)),
                settings: Mutex::new(settings),
                settings_path,
                logs_dir: dir.join("logs"),
                session: Mutex::new(None),
                session_seq: Mutex::new(0),
            });
            tray::setup_tray(&app.handle())?;

            // 调度器 tick：每秒检查是否到期
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let state = handle.state::<AppState>();
                let due = state.scheduler.lock().unwrap().is_due();
                let in_session = state.session.lock().unwrap().is_some();
                // 会话进行中到期无需处理：会话结束时会 reload 重新计时
                if due && !in_session {
                    session::start_session(&handle, "scheduled");
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.label() == "overlay" {
                    // overlay 被关闭（如 Alt+F4）视为提前结束
                    session::end_session(window.app_handle(), "stopped");
                } else {
                    // 主窗口关闭转隐藏到托盘
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            trigger_break,
            end_break,
            get_break_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
