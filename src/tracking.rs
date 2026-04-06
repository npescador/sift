#![allow(dead_code)] // Stub: implemented in Phase 10

use std::time::SystemTime;

/// A single record capturing Sift's output reduction for one invocation.
#[derive(Debug)]
pub struct TrackingRecord {
    pub command_family: String,
    pub original_bytes: usize,
    pub filtered_bytes: usize,
    pub exit_code: i32,
    pub timestamp: SystemTime,
}

impl TrackingRecord {
    pub fn savings_bytes(&self) -> usize {
        self.original_bytes.saturating_sub(self.filtered_bytes)
    }

    pub fn savings_percent(&self) -> f64 {
        if self.original_bytes == 0 {
            return 0.0;
        }
        self.savings_bytes() as f64 / self.original_bytes as f64 * 100.0
    }
}

/// Session-scoped tracker that accumulates records in memory.
///
/// Phase 10 will persist these to SQLite via rusqlite.
#[derive(Debug, Default)]
pub struct Tracker {
    records: Vec<TrackingRecord>,
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, entry: TrackingRecord) {
        self.records.push(entry);
    }

    pub fn total_saved_bytes(&self) -> usize {
        self.records.iter().map(|r| r.savings_bytes()).sum()
    }

    pub fn invocation_count(&self) -> usize {
        self.records.len()
    }
}
