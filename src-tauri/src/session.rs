use crate::{settings::Settings, storage, AppState};
use serde::Serialize;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// 进行中的休息会话。
pub struct ActiveSession {
    pub seq: u64,
    pub planned_sec: f64,
    pub started: Instant,
    pub trigger: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BreakStartPayload {
    pub planned_sec: f64,
    pub trigger: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BreakEndPayload {
    pub result: String,
}

/// overlay 前端初始化时拉取的会话状态（事件可能早于 webview 加载完成，
/// 所以以主动查询为准）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BreakStatePayload {
    pub planned_sec: f64,
    pub elapsed_sec: f64,
}

pub fn get_break_state(app: &AppHandle) -> Option<BreakStatePayload> {
    app.state::<AppState>()
        .session
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| BreakStatePayload {
            planned_sec: s.planned_sec,
            elapsed_sec: s.started.elapsed().as_secs_f64(),
        })
}

/// 开启一个休息会话；已有会话进行中时忽略。
pub fn start_session(app: &AppHandle, trigger: &str) {
    let state = app.state::<AppState>();
    let (seq, planned_sec, mode) = {
        let mut session = state.session.lock().unwrap();
        if session.is_some() {
            return;
        }
        let settings = state.settings.lock().unwrap();
        let planned = settings.break_.duration_sec;
        let mode = settings.global.overlay_mode.clone();
        let mut seq_guard = state.session_seq.lock().unwrap();
        *seq_guard += 1;
        let seq = *seq_guard;
        *session = Some(ActiveSession {
            seq,
            planned_sec: planned,
            started: Instant::now(),
            trigger: trigger.to_string(),
        });
        (seq, planned, mode)
    };

    show_overlay(app, &mode);
    let _ = app.emit(
        "break://start",
        BreakStartPayload {
            planned_sec,
            trigger: trigger.to_string(),
        },
    );

    // 倒计时归零自动结束（done）。提前结束时 seq 对应会话已被取走，此处自然失效。
    let app_clone = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs_f64(planned_sec));
        let still_active = {
            let session = app_clone.state::<AppState>().session.lock().unwrap();
            matches!(session.as_ref(), Some(s) if s.seq == seq)
        };
        if still_active {
            end_session(&app_clone, "done");
        }
    });
}

/// 结束当前会话：写记录、隐藏 overlay、重置计时器。
pub fn end_session(app: &AppHandle, result: &str) {
    let state = app.state::<AppState>();
    let session = state.session.lock().unwrap().take();
    let Some(s) = session else { return };

    let record = storage::LogRecord {
        ts: chrono::Local::now().to_rfc3339(),
        trigger: s.trigger.clone(),
        planned_sec: s.planned_sec,
        actual_sec: s.started.elapsed().as_secs_f64(),
        result: result.to_string(),
    };
    if let Err(e) = storage::append_log(&state.logs_dir, &record) {
        eprintln!("append log failed: {e}");
    }

    hide_overlay(app);
    let _ = app.emit(
        "break://end",
        BreakEndPayload {
            result: result.to_string(),
        },
    );

    let settings: Settings = state.settings.lock().unwrap().clone();
    state.scheduler.lock().unwrap().reload(&settings);
}

fn show_overlay(app: &AppHandle, mode: &str) {
    if let Some(win) = app.get_webview_window("overlay") {
        if mode == "fullscreen" {
            let _ = win.set_fullscreen(true);
        } else {
            let _ = win.set_fullscreen(false);
            let _ = win.center();
        }
        let _ = win.show();
        let _ = win.set_focus();
    }
}

fn hide_overlay(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("overlay") {
        let _ = win.hide();
    }
}
