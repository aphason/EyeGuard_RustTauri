use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppStateEnum {
    Working,
    Resting,
}

pub struct AppTimer {
    pub state: AppStateEnum,
    pub remaining_secs: u32,
    pub total_secs: u32,
    pub last_tick: Instant,
    pub paused: bool,
    pub postpone_count: u32,
}

impl AppTimer {
    pub fn new(work_interval_minutes: u32) -> Self {
        let total = work_interval_minutes * 60;
        Self {
            state: AppStateEnum::Working,
            remaining_secs: total,
            total_secs: total,
            last_tick: Instant::now(),
            paused: false,
            postpone_count: 0,
        }
    }

    pub fn reset_work(&mut self, work_interval_minutes: u32) {
        let total = work_interval_minutes * 60;
        self.state = AppStateEnum::Working;
        self.remaining_secs = total;
        self.total_secs = total;
        self.last_tick = Instant::now();
        self.paused = false;
        self.postpone_count = 0;
    }

    pub fn reset_rest(&mut self, rest_duration_minutes: u32) {
        let total = rest_duration_minutes * 60;
        self.state = AppStateEnum::Resting;
        self.remaining_secs = total;
        self.total_secs = total;
        self.last_tick = Instant::now();
        self.paused = false;
    }

    pub fn postpone(&mut self, minutes: u32, max_postpone: u32) -> bool {
        if self.postpone_count >= max_postpone {
            return false;
        }
        self.remaining_secs += minutes * 60;
        self.total_secs += minutes * 60;
        self.postpone_count += 1;
        true
    }

    pub fn tick(&mut self) -> Option<u32> {
        if self.paused {
            return None;
        }
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick).as_secs() as u32;
        if elapsed > 0 {
            self.last_tick = now;
            self.remaining_secs = self.remaining_secs.saturating_sub(elapsed);
            Some(self.remaining_secs)
        } else {
            None
        }
    }
}
