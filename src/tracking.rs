use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single record capturing Sift's output reduction for one invocation.
#[derive(Debug, Serialize, Deserialize, Clone)]
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
    #[allow(dead_code)]
    pub fn savings_percent(&self) -> f64 {
        if self.original_bytes == 0 {
            return 0.0;
        }
        self.savings_bytes() as f64 / self.original_bytes as f64 * 100.0
    }
}

// ── SQLite backend ─────────────────────────────────────────────────────────────

/// Append a record to the SQLite database.
///
/// Silently ignores all I/O and SQL errors — tracking must never break the proxy.
pub struct StatsFile;

impl StatsFile {
    /// Load all tracking records from the SQLite database.
    pub fn load() -> Vec<TrackingRecord> {
        Self::load_last(None)
    }

    /// Load up to `limit` most-recent records (None = all).
    pub fn load_last(limit: Option<usize>) -> Vec<TrackingRecord> {
        let path = match db_path() {
            Some(p) => p,
            None => return vec![],
        };
        let conn = match open_db(&path) {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let sql = match limit {
            Some(n) => format!(
                "SELECT command_family, original_bytes, filtered_bytes, \
                 exit_code, duration_ms, timestamp \
                 FROM records ORDER BY timestamp DESC LIMIT {n}"
            ),
            None => "SELECT command_family, original_bytes, filtered_bytes, \
                     exit_code, duration_ms, timestamp \
                     FROM records ORDER BY timestamp ASC"
                .to_string(),
        };
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map([], |row| {
            Ok(TrackingRecord {
                command_family: row.get(0)?,
                original_bytes: row.get::<_, i64>(1)? as usize,
                filtered_bytes: row.get::<_, i64>(2)? as usize,
                exit_code: row.get(3)?,
                duration_ms: row.get::<_, i64>(4)? as u64,
                timestamp: row.get::<_, i64>(5)? as u64,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Append a record to the on-disk SQLite database.
    ///
    /// Silently ignores all I/O and SQL errors.
    pub fn append(record: TrackingRecord) {
        let Some(path) = db_path() else { return };
        let Ok(conn) = open_db(&path) else { return };
        let _ = conn.execute(
            "INSERT INTO records \
             (command_family, original_bytes, filtered_bytes, exit_code, duration_ms, timestamp) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                record.command_family,
                record.original_bytes as i64,
                record.filtered_bytes as i64,
                record.exit_code,
                record.duration_ms as i64,
                record.timestamp as i64,
            ],
        );
    }

    /// Delete all records from the database.
    pub fn reset() -> bool {
        let Some(path) = db_path() else { return false };
        let Ok(conn) = open_db(&path) else {
            return false;
        };
        conn.execute("DELETE FROM records", []).is_ok()
    }

    /// Compute aggregate statistics over the provided records.
    pub fn summary() -> StatsSummary {
        Self::summary_of(Self::load())
    }

    /// Compute aggregate statistics over the last N records.
    pub fn summary_last(n: usize) -> StatsSummary {
        Self::summary_of(Self::load_last(Some(n)))
    }

    fn summary_of(records: Vec<TrackingRecord>) -> StatsSummary {
        let total = records.len();
        let total_original_bytes: usize = records.iter().map(|r| r.original_bytes).sum();
        let total_filtered_bytes: usize = records.iter().map(|r| r.filtered_bytes).sum();
        let mut by_family: BTreeMap<String, usize> = BTreeMap::new();
        for r in &records {
            *by_family.entry(r.command_family.clone()).or_insert(0) += 1;
        }
        StatsSummary {
            total,
            total_original_bytes,
            total_filtered_bytes,
            by_family,
        }
    }

    /// Export all records as a JSON string.
    pub fn to_json() -> String {
        let records = Self::load();
        serde_json::to_string_pretty(&records).unwrap_or_else(|_| "[]".to_string())
    }
}

// ── StatsSummary ───────────────────────────────────────────────────────────────

/// Aggregated view over tracking records for display.
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

// ── Database helpers ───────────────────────────────────────────────────────────

/// Open (or create) the SQLite database, running migrations if needed.
///
/// Also triggers TOML migration on first open.
fn open_db(path: &PathBuf) -> SqlResult<Connection> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS records (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            command_family TEXT    NOT NULL,
            original_bytes INTEGER NOT NULL,
            filtered_bytes INTEGER NOT NULL,
            exit_code      INTEGER NOT NULL,
            duration_ms    INTEGER NOT NULL,
            timestamp      INTEGER NOT NULL
        );",
    )?;
    // Migrate legacy TOML data on first open
    migrate_from_toml(&conn, path);
    Ok(conn)
}

/// Path to `~/.local/share/sift/stats.db` (XDG-aware).
fn db_path() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("sift").join("stats.db"));
        }
    }
    let home = std::env::var("HOME").ok()?;
    Some(
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("sift")
            .join("stats.db"),
    )
}

/// Read the legacy TOML stats file and insert its records into SQLite.
///
/// On success, renames the TOML file to `stats.toml.bak` so migration
/// is not repeated on subsequent runs.
fn migrate_from_toml(conn: &Connection, db_path: &std::path::Path) {
    // Only migrate if the DB was just created (empty records table)
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM records", [], |r| r.get(0))
        .unwrap_or(1); // if query fails, assume non-empty → skip
    if count > 0 {
        return;
    }

    // Look for stats.toml in the same directory as the database
    let toml_path = db_path.with_extension("toml");
    if !toml_path.exists() {
        return;
    }

    let Ok(content) = std::fs::read_to_string(&toml_path) else {
        return;
    };

    #[derive(Deserialize)]
    struct LegacyStore {
        #[serde(default)]
        records: Vec<TrackingRecord>,
    }

    let Ok(store) = toml::from_str::<LegacyStore>(&content) else {
        return;
    };

    for rec in store.records {
        let _ = conn.execute(
            "INSERT INTO records \
             (command_family, original_bytes, filtered_bytes, exit_code, duration_ms, timestamp) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                rec.command_family,
                rec.original_bytes as i64,
                rec.filtered_bytes as i64,
                rec.exit_code,
                rec.duration_ms as i64,
                rec.timestamp as i64,
            ],
        );
    }

    // Rename the TOML file so we don't migrate again
    let bak = db_path
        .parent()
        .map(|p| p.join("stats.toml.bak"))
        .unwrap_or_else(|| toml_path.with_extension("toml.bak"));
    let _ = std::fs::rename(&toml_path, bak);
}

// ── Tests ──────────────────────────────────────────────────────────────────────

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
    fn summary_aggregates_correctly() {
        let records = vec![
            TrackingRecord::new("git", 1000, 100, 0, 5),
            TrackingRecord::new("git", 500, 50, 0, 3),
            TrackingRecord::new("grep", 200, 20, 0, 2),
        ];
        let summary = StatsFile::summary_of(records);
        assert_eq!(summary.total, 3);
        assert_eq!(summary.total_original_bytes, 1700);
        assert_eq!(summary.total_filtered_bytes, 170);
        assert_eq!(summary.savings_bytes(), 1530);
        assert!((summary.savings_percent() - 90.0).abs() < 0.01);
        assert_eq!(*summary.by_family.get("git").unwrap(), 2);
        assert_eq!(*summary.by_family.get("grep").unwrap(), 1);
    }

    #[test]
    fn summary_savings_percent_zero_when_no_data() {
        let summary = StatsFile::summary_of(vec![]);
        assert_eq!(summary.savings_percent(), 0.0);
    }

    #[test]
    fn db_roundtrip_in_temp_dir() {
        let dir = std::env::temp_dir().join(format!("sift_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("stats.db");
        let conn = open_db(&db).unwrap();

        conn.execute(
            "INSERT INTO records \
             (command_family, original_bytes, filtered_bytes, exit_code, duration_ms, timestamp) \
             VALUES ('git', 1000, 100, 0, 5, 1700000000)",
            [],
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM records", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn toml_migration_imports_records() {
        let dir = std::env::temp_dir().join(format!("sift_migrate_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let toml_content = "[[records]]\n\
            command_family = \"git\"\n\
            original_bytes = 500\n\
            filtered_bytes = 50\n\
            exit_code = 0\n\
            duration_ms = 5\n\
            timestamp = 1712410000\n";
        std::fs::write(dir.join("stats.toml"), toml_content).unwrap();

        let db_path = dir.join("stats.db");
        let conn = open_db(&db_path).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM records", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "migrated record should exist in SQLite");

        // Legacy TOML should be renamed to .bak
        assert!(!dir.join("stats.toml").exists());
        assert!(dir.join("stats.toml.bak").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stats_file_reset_clears_data() {
        let dir = std::env::temp_dir().join(format!("sift_reset_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("stats.db");
        let conn = open_db(&db_path).unwrap();
        conn.execute(
            "INSERT INTO records \
             (command_family, original_bytes, filtered_bytes, exit_code, duration_ms, timestamp) \
             VALUES ('xcodebuild', 50000, 500, 0, 120, 1700000001)",
            [],
        )
        .unwrap();

        let count_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM records", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count_before, 1);

        conn.execute("DELETE FROM records", []).unwrap();

        let count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM records", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count_after, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
