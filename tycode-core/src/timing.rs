//! Timing utility for performance analysis

use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct Timer {
    timers: HashMap<String, Duration>,
    current_starts: HashMap<String, Instant>,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            timers: HashMap::new(),
            current_starts: HashMap::new(),
        }
    }

    pub fn start(&mut self, name: &str) {
        self.current_starts.insert(name.to_string(), Instant::now());
    }

    pub fn end(&mut self, name: &str) {
        if let Some(start) = self.current_starts.remove(name) {
            let duration = start.elapsed();
            *self
                .timers
                .entry(name.to_string())
                .or_insert(Duration::ZERO) += duration;
        }
    }

    pub fn get(&self, name: &str) -> Duration {
        self.timers.get(name).copied().unwrap_or(Duration::ZERO)
    }

    pub fn log_all(&self, total_name: &str, threshold_ms: u64) {
        let total = self.get(total_name);
        if total.as_millis() as u64 > threshold_ms {
            let mut parts = Vec::new();
            parts.push(format!("{}={}ms", total_name, total.as_millis()));

            for (name, duration) in &self.timers {
                if name != total_name {
                    parts.push(format!("{}={}ms", name, duration.as_millis()));
                }
            }

            tracing::info!("Timing: {}", parts.join(", "));
        }
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}
