use crate::storage::LogRecord;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 读取某日记录（None = 今天），最新在前。坏行跳过。
pub fn list_logs(logs_dir: &Path, date: Option<String>) -> Result<Vec<LogRecord>, String> {
    let date = date.unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let target = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| format!("日期格式无效：{date}"))?;
    let month = date.get(..7).ok_or("日期格式无效")?;
    let path = logs_dir.join(format!("{month}.jsonl"));
    let mut out = Vec::new();
    if let Ok(content) = fs::read_to_string(&path) {
        for line in content.lines() {
            if let Ok(r) = serde_json::from_str::<LogRecord>(line) {
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&r.ts) {
                    if ts.with_timezone(&chrono::Local).date_naive() == target {
                        out.push(r);
                    }
                }
            }
        }
    }
    out.reverse();
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub today_done: u32,
    pub streak_days: u32,
}

/// 统计：今日完成次数（done 记录数）、连续坚持天数。
/// 连续天数口径：今天尚无 done 时从昨天往前数（今天仍有机会，不算断）。
pub fn get_stats(logs_dir: &Path) -> Stats {    let mut done_by_day: HashMap<chrono::NaiveDate, u32> = HashMap::new();
    if let Ok(entries) = fs::read_dir(logs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&path) {
                for line in content.lines() {
                    if let Ok(r) = serde_json::from_str::<LogRecord>(line) {
                        if r.result == "done" {
                            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&r.ts) {
                                let day = ts.with_timezone(&chrono::Local).date_naive();
                                *done_by_day.entry(day).or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    let today = chrono::Local::now().date_naive();
    let today_done = done_by_day.get(&today).copied().unwrap_or(0);

    let mut streak_days = 0u32;
    let mut day = if today_done > 0 {
        today
    } else {
        today.pred_opt().unwrap_or(today)
    };
    while done_by_day.contains_key(&day) {
        streak_days += 1;
        match day.pred_opt() {
            Some(prev) => day = prev,
            None => break,
        }
    }

    Stats {
        today_done,
        streak_days,
    }
}

/// 清空全部休息记录（删除 logs 目录下所有 jsonl 文件）
pub fn clear_logs(logs_dir: &Path) -> Result<(), String> {
    if let Ok(entries) = fs::read_dir(logs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
