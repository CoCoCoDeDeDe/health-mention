use crate::{session, settings, AppState};
use tauri::{
    menu::{CheckMenuItemBuilder, Menu, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub const TRAY_ID: &str = "tray";

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let paused = app.state::<AppState>().settings.lock().unwrap().global.paused;

    let show = MenuItemBuilder::with_id("show", "打开设置").build(app)?;
    let break_now = MenuItemBuilder::with_id("break_now", "立即休息").build(app)?;
    let pause = CheckMenuItemBuilder::with_id("toggle_pause", "暂停提醒")
        .checked(paused)
        .build(app)?;
    // 保存句柄：快捷键切换暂停时同步菜单勾选状态
    *app.state::<AppState>().pause_item.lock().unwrap() = Some(pause.clone());
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;
    let menu = Menu::with_items(app, &[&show, &break_now, &pause, &sep, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("health-mention")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "break_now" => session::start_session(app, "manual"),
            "toggle_pause" => toggle_pause(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // 双击左键直接打开设置窗口
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

pub fn toggle_pause(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (settings, paused) = {
        let mut settings_guard = state.settings.lock().unwrap();
        settings_guard.global.paused = !settings_guard.global.paused;
        let _ = settings::save(&state.settings_path, &settings_guard);
        (settings_guard.clone(), settings_guard.global.paused)
    };
    if let Some(item) = state.pause_item.lock().unwrap().as_ref() {
        let _ = item.set_checked(paused);
    }
    state.scheduler.lock().unwrap().reload(&settings);
}

/// 开关「启用提醒」（快捷键动作）
pub fn toggle_enabled(app: &AppHandle) {
    let state = app.state::<AppState>();
    let settings = {
        let mut settings_guard = state.settings.lock().unwrap();
        settings_guard.break_.enabled = !settings_guard.break_.enabled;
        let _ = settings::save(&state.settings_path, &settings_guard);
        settings_guard.clone()
    };
    state.scheduler.lock().unwrap().reload(&settings);
}
