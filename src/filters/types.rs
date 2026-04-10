//! Shared structured types for filter output.
//!
//! These types serve as the intermediate representation between raw command
//! output and the final rendered text or JSON. Each type derives `Serialize`
//! to enable `--json` output in Phase 2.

#![allow(dead_code)] // Types are defined ahead of filter conversions

use serde::Serialize;

/// A single compiler/linter diagnostic message.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub file: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub severity: Severity,
    pub message: String,
}

/// Severity level for a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// Structured result of an `xcodebuild build` invocation.
#[derive(Debug, Serialize)]
pub struct XcodebuildBuildResult {
    pub succeeded: bool,
    /// Compiler errors grouped by file path.
    pub errors: Vec<Diagnostic>,
    pub warning_count: usize,
    /// Linker errors (ld / undefined symbols).
    pub linker_errors: Vec<String>,
    /// Signing / provisioning errors.
    pub signing_errors: Vec<String>,
}

/// Structured result of an `xcodebuild test` invocation.
#[derive(Debug, Serialize)]
pub struct XcodebuildTestResult {
    pub succeeded: bool,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub failures: Vec<TestFailure>,
}

/// A single failed test case.
#[derive(Debug, Clone, Serialize)]
pub struct TestFailure {
    pub name: String,
    pub message: String,
    pub location: String,
}

/// Structured result of `git status`.
#[derive(Debug, Serialize)]
pub struct GitStatusResult {
    pub branch: Option<String>,
    pub staged: Vec<String>,
    pub modified: Vec<String>,
    pub untracked: Vec<String>,
}

/// Structured result of `swiftlint` output.
#[derive(Debug, Serialize)]
pub struct SwiftlintResult {
    pub total_violations: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub file_count: usize,
    pub rules: Vec<SwiftlintRuleGroup>,
}

/// A group of violations for a single swiftlint rule.
#[derive(Debug, Clone, Serialize)]
pub struct SwiftlintRuleGroup {
    pub rule: String,
    pub severity: Severity,
    pub count: usize,
    pub locations: Vec<String>,
}

/// Structured result of `swift build` output.
#[derive(Debug, Serialize)]
pub struct SwiftBuildResult {
    pub succeeded: bool,
    /// Compiler errors grouped by file.
    pub errors: Vec<Diagnostic>,
    pub warning_count: usize,
    /// Warnings with details (for verbose mode).
    pub warnings: Vec<Diagnostic>,
}
