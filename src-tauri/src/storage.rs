use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    pub ts: String,
    pub trigger: String,
    pub planned_sec: f64,
    pub actual_sec: f64,
    pub result: String,
}

/// 追加一条休息记录到 logs/YYYY-MM.jsonl
pub fn append_log(logs_dir: &Path, record: &LogRecord) -> Result<(), String> {
    fs::create_dir_all(logs_dir).map_err(|e| e.to_string())?;
    let month = chrono::Local::now().format("%Y-%m").to_string();
    let path = logs_dir.join(format!("{month}.jsonl"));
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let line = serde_json::to_string(record).map_err(|e| e.to_string())?;
    writeln!(file, "{line}").map_err(|e| e.to_string())
}

/// 一个 overlay 窗口记忆的位置与宽高（逻辑像素）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OverlayRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub fn load_overlay_rects(path: &Path) -> HashMap<String, OverlayRect> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_overlay_rects(path: &Path, rects: &HashMap<String, OverlayRect>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(rects).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}
