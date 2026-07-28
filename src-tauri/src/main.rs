mod hotkey;
mod scheduler;
mod session;
mod settings;
mod stats;
mod storage;
mod tray;

use scheduler::Scheduler;
use serde::Serialize;
use session::{ActiveSession, BreakStatePayload};
use settings::Settings;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{menu::CheckMenuItem, Manager, State};
use tauri_plugin_autostart::ManagerExt;

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    pub logs_dir: PathBuf,
    pub scheduler: Mutex<Scheduler>,
    pub session: Mutex<Option<ActiveSession>>,
    pub session_seq: Mutex<u64>,
    /// 托盘「暂停提醒」勾选框句柄，用于快捷键切换时同步勾选状态
    pub pause_item: Mutex<Option<CheckMenuItem<tauri::Wry>>>,
    /// 设置页「预览休息浮窗」开关状态
    pub overlay_preview: Mutex<bool>,
    /// 各 overlay 窗口记忆的位置与宽高（按窗口标签）
    pub overlay_rects: Mutex<HashMap<String, storage::OverlayRect>>,
    pub rects_path: PathBuf,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ScheduleTickPayload {
    remaining_sec: Option<f64>,
    status: &'static str,
}

fn fmt_hms(secs: f64) -> String {
    let total = secs.ceil() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
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
    // 快捷键全量重注册；失败（被占用/格式错）返回错误提示用户
    hotkey::apply_hotkeys(&app, &settings.hotkeys)?;
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

#[tauri::command]
fn get_schedule_state(state: State<AppState>) -> ScheduleTickPayload {
    let in_session = state.session.lock().unwrap().is_some();
    schedule_state(&state, in_session)
}

#[tauri::command]
fn list_logs(state: State<AppState>, date: Option<String>) -> Result<Vec<storage::LogRecord>, String> {
    stats::list_logs(&state.logs_dir, date)
}

#[tauri::command]
fn get_stats(state: State<AppState>) -> stats::Stats {
    stats::get_stats(&state.logs_dir)
}

#[tauri::command]
fn save_overlay_rect(
    state: State<AppState>,
    label: String,
    rect: storage::OverlayRect,
) -> Result<(), String> {
    let mut rects = state.overlay_rects.lock().unwrap();
    if rects.get(&label) == Some(&rect) {
        return Ok(());
    }
    rects.insert(label, rect);
    storage::save_overlay_rects(&state.rects_path, &rects)
}

#[tauri::command]
fn set_overlay_preview(app: tauri::AppHandle, open: bool) {
    session::set_preview(&app, open);
}

#[tauri::command]
fn get_overlay_preview(state: State<AppState>) -> bool {
    *state.overlay_preview.lock().unwrap()
}

/// 计算当前调度状态（下一次休息倒计时）。
fn schedule_state(state: &AppState, in_session: bool) -> ScheduleTickPayload {
    let settings = state.settings.lock().unwrap();
    if in_session {
        ScheduleTickPayload {
            remaining_sec: None,
            status: "inSession",
        }
    } else if settings.global.paused {
        ScheduleTickPayload {
            remaining_sec: None,
            status: "paused",
        }
    } else if !settings.break_.enabled {
        ScheduleTickPayload {
            remaining_sec: None,
            status: "disabled",
        }
    } else {
        ScheduleTickPayload {
            remaining_sec: state
                .scheduler
                .lock()
                .unwrap()
                .remaining()
                .map(|d| d.as_secs_f64()),
            status: "running",
        }
    }
}

/// 每秒更新托盘 tooltip。
fn update_tray_tooltip(handle: &tauri::AppHandle, in_session: bool) {
    let payload = schedule_state(&handle.state::<AppState>(), in_session);
    if let Some(tray) = handle.tray_by_id(tray::TRAY_ID) {
        let tooltip = match payload.remaining_sec {
            Some(s) => format!("health-mention · {} 后休息", fmt_hms(s)),
            None => match payload.status {
                "paused" => "health-mention · 已暂停".to_string(),
                "disabled" => "health-mention · 提醒已禁用".to_string(),
                _ => "health-mention · 休息中".to_string(),
            },
        };
        let _ = tray.set_tooltip(Some(&tooltip));
    }
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
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        hotkey::handle_shortcut(app, shortcut);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let dir = app.path().app_data_dir().expect("app data dir unavailable");
            std::fs::create_dir_all(&dir).ok();
            let settings_path = dir.join("settings.json");
            let settings = settings::load(&settings_path);
            let rects_path = dir.join("overlay-rects.json");
            app.manage(AppState {
                scheduler: Mutex::new(Scheduler::new(&settings)),
                settings: Mutex::new(settings),
                settings_path,
                logs_dir: dir.join("logs"),
                session: Mutex::new(None),
                session_seq: Mutex::new(0),
                pause_item: Mutex::new(None),
                overlay_preview: Mutex::new(false),
                overlay_rects: Mutex::new(storage::load_overlay_rects(&rects_path)),
                rects_path,
            });
            tray::setup_tray(&app.handle())?;

            // 启动时按配置注册全局快捷键
            let hotkeys = app
                .state::<AppState>()
                .settings
                .lock()
                .unwrap()
                .hotkeys
                .clone();
            if let Err(e) = hotkey::apply_hotkeys(&app.handle(), &hotkeys) {
                eprintln!("apply hotkeys failed: {e}");
            }

            // 调度器 tick：每秒检查到期 + 更新托盘 tooltip
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let state = handle.state::<AppState>();
                let in_session = state.session.lock().unwrap().is_some();
                let due = state.scheduler.lock().unwrap().is_due();
                // 会话进行中到期无需处理：会话结束时会 reload 重新计时
                if due && !in_session {
                    session::start_session(&handle, "scheduled");
                } else {
                    update_tray_tooltip(&handle, in_session);
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label().starts_with("overlay") {
                    // overlay 被关闭（如 Alt+F4）视为提前结束；无会话时允许直接关闭
                    let has_session = window
                        .app_handle()
                        .state::<AppState>()
                        .session
                        .lock()
                        .unwrap()
                        .is_some();
                    if has_session {
                        api.prevent_close();
                        session::end_session(window.app_handle(), "stopped");
                    }
                } else {
                    // 主窗口关闭转隐藏到托盘
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            trigger_break,
            end_break,
            get_break_state,
            get_schedule_state,
            list_logs,
            get_stats,
            save_overlay_rect,
            set_overlay_preview,
            get_overlay_preview
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
