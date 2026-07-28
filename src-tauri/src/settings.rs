use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakSettings {
    /// 间隔分钟，支持小数（如 0.5 = 30 秒）
    pub interval_min: f64,
    /// 休息时长秒，支持小数
    pub duration_sec: f64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hotkeys {
    pub break_now: Option<String>,
    pub toggle_pause: Option<String>,
    pub open_settings: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSettings {
    pub paused: bool,
    pub autostart: bool,
    /// "float" | "fullscreen"
    pub overlay_mode: String,
    pub theme: String,
    pub progress_plugin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    #[serde(rename = "break")]
    pub break_: BreakSettings,
    pub hotkeys: Hotkeys,
    pub global: GlobalSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            break_: BreakSettings {
                interval_min: 30.0,
                duration_sec: 60.0,
                enabled: true,
            },
            hotkeys: Hotkeys {
                break_now: None,
                toggle_pause: None,
                open_settings: None,
            },
            global: GlobalSettings {
                paused: false,
                autostart: false,
                overlay_mode: "float".into(),
                theme: "default".into(),
                progress_plugin: None,
            },
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        // NaN 比较恒为 false，会自然落入错误分支
        if !(self.break_.interval_min > 0.0 && self.break_.interval_min <= 1440.0) {
            return Err("间隔分钟需在 0~1440 之间（不含 0）".into());
        }
        if !(self.break_.duration_sec > 0.0 && self.break_.duration_sec <= 3600.0) {
            return Err("休息时长需在 0~3600 秒之间（不含 0）".into());
        }
        if !matches!(self.global.overlay_mode.as_str(), "float" | "fullscreen") {
            return Err("overlayMode 只能是 float 或 fullscreen".into());
        }
        Ok(())
    }
}

pub fn load(path: &Path) -> Settings {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    settings.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}
