use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Instant;

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
/// Spawns `program` with `args`, waits for completion, and returns the
/// captured stdout, stderr, exit code, and wall-clock duration in ms.
///
/// # Exit code contract
/// The returned `exit_code` is always the exact code from the subprocess.
/// If the process is killed by a signal, `exit_code` is set to `1` as a
/// safe fallback — never silently swallowed.
///
/// # Errors
/// Returns `SiftError::CommandNotFound` if the binary cannot be found.
/// Returns `SiftError::Io` for other spawn failures.
pub fn execute(program: &str, args: &[String]) -> Result<ExecutorOutput, SiftError> {
    let start = Instant::now();

    let output = Command::new(program).args(args).output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            SiftError::CommandNotFound(program.to_string())
        } else {
            SiftError::Io(e)
        }
    })?;

    let duration_ms = start.elapsed().as_millis() as u64;

    let exit_code = output.status.code().unwrap_or(1);

    Ok(ExecutorOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code,
        duration_ms,
    })
}

/// Execute a command with streaming output.
///
/// Like `execute`, but pipes stdout line-by-line through `on_line` as it arrives.
/// The full stdout is still accumulated and returned in `ExecutorOutput` so
/// the caller can run the normal filter at the end.
///
/// stderr is captured separately and returned in full.
pub fn execute_streaming<F>(
    program: &str,
    args: &[String],
    mut on_line: F,
) -> Result<ExecutorOutput, SiftError>
where
    F: FnMut(&str),
{
    let start = Instant::now();

    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SiftError::CommandNotFound(program.to_string())
            } else {
                SiftError::Io(e)
            }
        })?;

    let stdout_pipe = child.stdout.take().expect("stdout was piped");
    let reader = BufReader::new(stdout_pipe);

    let mut full_output = String::new();

    for line in reader.lines() {
        let line = line.map_err(SiftError::Io)?;
        full_output.push_str(&line);
        full_output.push('\n');
        on_line(&line);
    }

    let status = child.wait().map_err(SiftError::Io)?;

    // Capture stderr after process completes
    let stderr = child
        .stderr
        .take()
        .map(|pipe| {
            let mut buf = String::new();
            let _ = std::io::Read::read_to_string(&mut BufReader::new(pipe), &mut buf);
            buf
        })
        .unwrap_or_default();

    let duration_ms = start.elapsed().as_millis() as u64;
    let exit_code = status.code().unwrap_or(1);

    Ok(ExecutorOutput {
        stdout: full_output,
        stderr,
        exit_code,
        duration_ms,
    })
}
