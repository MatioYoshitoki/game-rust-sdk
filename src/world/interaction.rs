//! A bounded layered interaction clock, independent of rewards and World ownership.
use crate::{config::SealConfig, protocol::ChestInterruptReason};
#[derive(Default)]
pub(super) struct LayeredProgress {
    pub completed: u32,
    pub elapsed_ms: u64,
    pub active_since: Option<u64>,
    pub interruption: Option<ChestInterruptReason>,
}
impl LayeredProgress {
    pub fn start(&mut self, now: u64) {
        self.active_since = Some(now);
        self.interruption = None;
    }
    pub fn interrupt(&mut self, config: &SealConfig, reason: ChestInterruptReason) -> bool {
        if self.active_since.take().is_none() {
            return false;
        }
        self.elapsed_ms =
            (self.elapsed_ms as f64 * config.interrupt_retention_ratio).floor() as u64;
        self.interruption = Some(reason);
        true
    }
    pub fn advance(&mut self, now: u64, config: &SealConfig) -> std::ops::Range<u32> {
        let before = self.completed;
        if let Some(previous) = self.active_since {
            self.elapsed_ms += now.saturating_sub(previous);
            self.active_since = Some(now);
            while self.completed < config.layers && self.elapsed_ms >= config.layer_duration_ms {
                self.elapsed_ms -= config.layer_duration_ms;
                self.completed += 1;
            }
            if self.completed == config.layers {
                self.active_since = None;
                self.elapsed_ms = 0;
            }
        }
        before..self.completed
    }
}
