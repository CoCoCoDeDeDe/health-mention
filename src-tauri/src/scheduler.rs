use crate::settings::Settings;
use std::time::{Duration, Instant};

/// 单一「休息」提醒的计时器。
/// 简单策略：启用且未暂停时维护 next_due；任何 reload（设置变更、暂停切换、
/// 休息结束）都重新计一个完整间隔。
pub struct Scheduler {
    next_due: Option<Instant>,
}

impl Scheduler {
    pub fn new(settings: &Settings) -> Self {
        let mut s = Self { next_due: None };
        s.reload(settings);
        s
    }

    pub fn reload(&mut self, settings: &Settings) {
        self.next_due = if settings.break_.enabled && !settings.global.paused {
            Some(Instant::now() + Duration::from_secs(settings.break_.interval_min as u64 * 60))
        } else {
            None
        };
    }

    pub fn is_due(&self) -> bool {
        matches!(self.next_due, Some(t) if Instant::now() >= t)
    }
}
