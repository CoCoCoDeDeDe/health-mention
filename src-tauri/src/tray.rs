use crate::{settings, AppState};
use tauri::{
    menu::{CheckMenuItemBuilder, Menu, MenuItemBuilder, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

pub const TRAY_ID: &str = "tray";

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let paused = app.state::<AppState>().settings.lock().unwrap().global.paused;

    let show = MenuItemBuilder::with_id("show", "打开设置").build(app)?;
    // M2 接入调度器后启用
    let break_now = MenuItemBuilder::with_id("break_now", "立即休息")
        .enabled(false)
        .build(app)?;
    let pause = CheckMenuItemBuilder::with_id("toggle_pause", "暂停提醒")
        .checked(paused)
        .build(app)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;
    let menu = Menu::with_items(app, &[&show, &break_now, &pause, &sep, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("health-mention")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "toggle_pause" => toggle_pause(app),
            "quit" => app.exit(0),
            _ => {}
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

fn toggle_pause(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut settings_guard = state.settings.lock().unwrap();
    settings_guard.global.paused = !settings_guard.global.paused;
    let _ = settings::save(&state.settings_path, &settings_guard);
}
