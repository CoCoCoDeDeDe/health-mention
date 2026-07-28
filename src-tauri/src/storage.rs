use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    pub ts: String,
    pub trigger: String,
    pub planned_sec: u64,
    pub actual_sec: u64,
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
