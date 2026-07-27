use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakSettings {
    pub interval_min: u32,
    pub duration_sec: u32,
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
                interval_min: 30,
                duration_sec: 60,
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
        if !(1..=1440).contains(&self.break_.interval_min) {
            return Err("间隔分钟需在 1~1440 之间".into());
        }
        if !(5..=3600).contains(&self.break_.duration_sec) {
            return Err("休息时长需在 5~3600 秒之间".into());
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
