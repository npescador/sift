use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single record capturing Sift's output reduction for one invocation.
#[derive(Debug, Serialize, Deserialize)]
pub struct TrackingRecord {
    pub command_family: String,
    pub original_bytes: usize,
    pub filtered_bytes: usize,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub timestamp: u64,
}

impl TrackingRecord {
    /// Create a new record stamped with the current Unix timestamp.
    pub fn new(
        command_family: &str,
        original_bytes: usize,
        filtered_bytes: usize,
        exit_code: i32,
        duration_ms: u64,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            command_family: command_family.to_string(),
            original_bytes,
            filtered_bytes,
            exit_code,
            duration_ms,
            timestamp,
        }
    }

    pub fn savings_bytes(&self) -> usize {
        self.original_bytes.saturating_sub(self.filtered_bytes)
    }

    /// Percentage of bytes saved for this single invocation.
    #[allow(dead_code)] // available for per-record display in future phases
    pub fn savings_percent(&self) -> f64 {
        if self.original_bytes == 0 {
            return 0.0;
        }
        self.savings_bytes() as f64 / self.original_bytes as f64 * 100.0
    }
}

/// TOML-backed persistent store for tracking records.
///
/// The file lives at `$XDG_DATA_HOME/sift/stats.toml`
/// or `$HOME/.local/share/sift/stats.toml`.
/// Missing file → treated as empty. Write errors are silently ignored so
/// tracking never breaks the proxy workflow.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StatsFile {
    pub records: Vec<TrackingRecord>,
}

impl StatsFile {
    /// Load the stats file. Returns empty store on missing file or parse error.
    pub fn load() -> Self {
        let Some(path) = stats_path() else {
            return Self::default();
        };
        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => return Self::default(),
        };
        toml::from_str(&content).unwrap_or_default()
    }

    /// Append a record to the on-disk stats file.
    ///
    /// Silently ignores all I/O and serialization errors.
    pub fn append(record: TrackingRecord) {
        let Some(path) = stats_path() else { return };
        let mut store = Self::load();
        store.records.push(record);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = toml::to_string(&store) {
            let _ = std::fs::write(&path, content);
        }
    }

    /// Compute aggregate statistics over all records.
    pub fn summary(&self) -> StatsSummary {
        let total = self.records.len();
        let total_original_bytes: usize = self.records.iter().map(|r| r.original_bytes).sum();
        let total_filtered_bytes: usize = self
            .records
            .iter()
            .map(|r| r.original_bytes - r.savings_bytes())
            .sum();
        let mut by_family: BTreeMap<String, usize> = BTreeMap::new();
        for r in &self.records {
            *by_family.entry(r.command_family.clone()).or_insert(0) += 1;
        }
        StatsSummary {
            total,
            total_original_bytes,
            total_filtered_bytes,
            by_family,
        }
    }
}

/// Aggregated view over all tracking records for display.
pub struct StatsSummary {
    pub total: usize,
    pub total_original_bytes: usize,
    pub total_filtered_bytes: usize,
    pub by_family: BTreeMap<String, usize>,
}

impl StatsSummary {
    pub fn savings_bytes(&self) -> usize {
        self.total_original_bytes
            .saturating_sub(self.total_filtered_bytes)
    }

    pub fn savings_percent(&self) -> f64 {
        if self.total_original_bytes == 0 {
            return 0.0;
        }
        self.savings_bytes() as f64 / self.total_original_bytes as f64 * 100.0
    }
}

fn stats_path() -> Option<std::path::PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Some(
                std::path::PathBuf::from(xdg)
                    .join("sift")
                    .join("stats.toml"),
            );
        }
    }
    let home = std::env::var("HOME").ok()?;
    Some(
        std::path::PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("sift")
            .join("stats.toml"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracking_record_savings_bytes_correct() {
        let rec = TrackingRecord::new("git", 1000, 200, 0, 10);
        assert_eq!(rec.savings_bytes(), 800);
        assert!((rec.savings_percent() - 80.0).abs() < 0.001);
    }

    #[test]
    fn tracking_record_savings_zero_when_no_original() {
        let rec = TrackingRecord::new("git", 0, 0, 0, 0);
        assert_eq!(rec.savings_percent(), 0.0);
    }

    #[test]
    fn stats_summary_aggregates_correctly() {
        let mut store = StatsFile::default();
        store
            .records
            .push(TrackingRecord::new("git", 1000, 100, 0, 5));
        store
            .records
            .push(TrackingRecord::new("git", 500, 50, 0, 3));
        store
            .records
            .push(TrackingRecord::new("grep", 200, 20, 0, 2));
        let summary = store.summary();
        assert_eq!(summary.total, 3);
        assert_eq!(summary.total_original_bytes, 1700);
        assert_eq!(summary.total_filtered_bytes, 170);
        assert_eq!(summary.savings_bytes(), 1530);
        assert_eq!(*summary.by_family.get("git").unwrap(), 2);
        assert_eq!(*summary.by_family.get("grep").unwrap(), 1);
    }

    #[test]
    fn stats_file_deserializes_from_toml() {
        let toml_str = "[[records]]\n\
            command_family = \"git\"\n\
            original_bytes = 500\n\
            filtered_bytes = 50\n\
            exit_code = 0\n\
            duration_ms = 5\n\
            timestamp = 1712410000\n";
        let store: StatsFile = toml::from_str(toml_str).unwrap();
        assert_eq!(store.records.len(), 1);
        assert_eq!(store.records[0].command_family, "git");
    }
}
