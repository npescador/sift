use thiserror::Error;

/// Errors owned by Sift itself (distinct from wrapped command failures).
///
/// All messages are prefixed with `[sift error]` when printed in `main`.
/// Wrapped command exit codes are propagated as `i32`, not as `SiftError`.
#[derive(Debug, Error)]
pub enum SiftError {
    /// The underlying command binary could not be found or executed.
    #[error("command not found: {0}")]
    CommandNotFound(String),

    /// Configuration file could not be parsed.
    #[error("configuration error: {0}")]
    Config(String),

    /// An I/O error occurred while spawning or reading from the command.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
