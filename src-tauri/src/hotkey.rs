use crate::{session, settings::Hotkeys, tray, AppState};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

/// 快捷键按下时的分发：与托盘菜单行为一致
pub fn handle_shortcut(app: &AppHandle, shortcut: &Shortcut) {
    let hotkeys = app.state::<AppState>().settings.lock().unwrap().hotkeys.clone();
    let matches = |acc: &Option<String>| {
        acc.as_deref()
            .and_then(|s| s.parse::<Shortcut>().ok())
            .as_ref()
            == Some(shortcut)
    };
    if matches(&hotkeys.break_now) {
        session::start_session(app, "manual");
    } else if matches(&hotkeys.toggle_enabled) {
        tray::toggle_enabled(app);
    } else if matches(&hotkeys.end_break) {
        session::end_session(app, "stopped");
    } else if matches(&hotkeys.open_settings) {
        tray::show_main_window(app);
    }
}

/// 按当前配置全量重注册。None = 该动作关闭。
/// 注册失败（格式无效/被占用）返回错误，由调用方提示用户。
pub fn apply_hotkeys(app: &AppHandle, hotkeys: &Hotkeys) -> Result<(), String> {
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| e.to_string())?;
    for (name, acc) in [
        ("立即休息", &hotkeys.break_now),
        ("开关启用提醒", &hotkeys.toggle_enabled),
        ("手动结束休息", &hotkeys.end_break),
        ("打开设置", &hotkeys.open_settings),
    ] {
        if let Some(acc) = acc {
            let shortcut: Shortcut = acc
                .parse()
                .map_err(|_| format!("快捷键「{name}」格式无效：{acc}"))?;
            gs.register(shortcut)
                .map_err(|e| format!("快捷键「{name}」({acc}) 注册失败：{e}"))?;
        }
    }
    Ok(())
}
