use crate::{settings::Settings, storage, tray, AppState};
use serde::Serialize;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const FLOAT_W: f64 = 320.0;
const FLOAT_H: f64 = 44.0;

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
    pub mode: String,
}

pub fn get_break_state(app: &AppHandle) -> Option<BreakStatePayload> {
    let state = app.state::<AppState>();
    let session = state.session.lock().unwrap();
    session.as_ref().map(|s| {
        let mode = state.settings.lock().unwrap().global.overlay_mode.clone();
        BreakStatePayload {
            planned_sec: s.planned_sec,
            elapsed_sec: s.started.elapsed().as_secs_f64(),
            mode,
        }
    })
}

/// 设置页「预览休息浮窗」开关：展示/隐藏静态预览窗（无会话）。
pub fn set_preview(app: &AppHandle, open: bool) {
    *app.state::<AppState>().overlay_preview.lock().unwrap() = open;
    if open {
        let mode = app
            .state::<AppState>()
            .settings
            .lock()
            .unwrap()
            .global
            .overlay_mode
            .clone();
        show_overlays(app, &mode);
    } else {
        hide_overlays(app);
    }
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
        // 预览若开着，随真实会话关闭（窗口会重建为真实倒计时）
        *state.overlay_preview.lock().unwrap() = false;
        (seq, planned, mode)
    };

    show_overlays(app, &mode);
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
            let state = app_clone.state::<AppState>();
            let session = state.session.lock().unwrap();
            matches!(session.as_ref(), Some(s) if s.seq == seq)
        };
        if still_active {
            end_session(&app_clone, "done");
        }
    });
}

/// 结束当前会话：写记录、销毁所有 overlay、重置计时器、刷新托盘统计。
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

    hide_overlays(app);
    let _ = app.emit(
        "break://end",
        BreakEndPayload {
            result: result.to_string(),
        },
    );

    let settings: Settings = state.settings.lock().unwrap().clone();
    state.scheduler.lock().unwrap().reload(&settings);
    tray::update_stats_item(app);
}

/// 每个显示器各创建一个 overlay 窗口：优先使用记忆的位置与宽高，
/// 否则初始同尺寸、各自居中。先销毁已有窗口保证状态干净。
fn show_overlays(app: &AppHandle, mode: &str) {
    hide_overlays(app);
    let monitors = app.available_monitors().unwrap_or_default();
    if monitors.is_empty() {
        // 兜底：拿不到显示器信息时至少创建一个默认位置的窗口
        build_overlay(app, "overlay-0", mode, None);
        return;
    }
    for (i, monitor) in monitors.iter().enumerate() {
        build_overlay(app, &format!("overlay-{i}"), mode, Some(monitor));
    }
}

fn build_overlay(app: &AppHandle, label: &str, mode: &str, monitor: Option<&tauri::Monitor>) {
    if app.get_webview_window(label).is_some() {
        return;
    }
    let saved = app
        .state::<AppState>()
        .overlay_rects
        .lock()
        .unwrap()
        .get(label)
        .copied();
    let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App("overlay.html".into()))
        .title("休息一下")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        // 出现时不夺取输入焦点，避免打断用户打字
        .focused(false)
        .visible(true);
    if mode == "fullscreen" {
        builder = builder.fullscreen(true);
    } else if let Some(r) = saved {
        builder = builder.inner_size(r.w, r.h).position(r.x, r.y);
    } else if let Some(m) = monitor {
        let scale = m.scale_factor();
        let mw = m.size().width as f64 / scale;
        let mh = m.size().height as f64 / scale;
        let mx = m.position().x as f64 / scale;
        let my = m.position().y as f64 / scale;
        builder = builder
            .inner_size(FLOAT_W, FLOAT_H)
            .position(mx + (mw - FLOAT_W) / 2.0, my + (mh - FLOAT_H) / 2.0);
    } else {
        builder = builder.inner_size(FLOAT_W, FLOAT_H);
    }
    if let Err(e) = builder.build() {
        eprintln!("create overlay window {label} failed: {e}");
    }
}

fn hide_overlays(app: &AppHandle) {
    for (label, win) in app.webview_windows() {
        if label.starts_with("overlay") {
            let _ = win.destroy();
        }
    }
}
