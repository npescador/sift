#![allow(dead_code)] // Stub: implemented in Phase 3

use crate::error::SiftError;

/// The captured result of running an underlying command.
///
/// The `exit_code` is always the exact code from the subprocess.
/// Sift never modifies or suppresses exit codes.
#[derive(Debug)]
pub struct ExecutorOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

/// Execute a command and capture its full output.
///
/// Spawns `program` with `args`, waits for completion, and returns
/// the captured stdout, stderr, exit code, and wall-clock duration.
///
/// # Exit code contract
/// The returned `exit_code` is always the exact code from the subprocess.
/// This function never alters it.
pub fn execute(program: &str, args: &[String]) -> Result<ExecutorOutput, SiftError> {
    // Phase 3: real implementation using std::process::Command
    Err(SiftError::CommandNotFound(format!(
        "{program} — executor not yet implemented"
    )))
}
