//! Shared structured types for filter output.
//!
//! These types serve as the intermediate representation between raw command
//! output and the final rendered text or JSON. Each type derives `Serialize`
//! to enable `--json` output.

use serde::Serialize;

// ── Shared primitives ──────────────────────────────────────────────────────────

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

/// A single failed test case.
#[derive(Debug, Clone, Serialize)]
pub struct TestFailure {
    pub name: String,
    pub message: String,
    pub location: String,
}

// ── xcodebuild ────────────────────────────────────────────────────────────────

/// Structured result of an `xcodebuild build` invocation.
#[derive(Debug, Serialize)]
pub struct XcodebuildBuildResult {
    pub succeeded: bool,
    pub errors: Vec<Diagnostic>,
    pub warning_count: usize,
    pub linker_errors: Vec<String>,
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

/// Structured result of `xcodebuild archive`.
#[derive(Debug, Serialize)]
pub struct XcodebuildArchiveResult {
    pub succeeded: bool,
    pub archive_path: Option<String>,
    pub scheme: Option<String>,
    pub configuration: Option<String>,
    pub team: Option<String>,
    pub identity: Option<String>,
    pub errors: Vec<Diagnostic>,
    pub warnings_count: usize,
}

/// Structured result of `xcodebuild -list`.
#[derive(Debug, Serialize)]
pub struct XcodebuildListResult {
    pub project: Option<String>,
    pub schemes: Vec<String>,
    pub targets: Vec<String>,
    pub configurations: Vec<String>,
    pub default_scheme: Option<String>,
}

/// Structured result of `xcodebuild -showBuildSettings`.
#[derive(Debug, Serialize)]
pub struct XcodebuildSettingsResult {
    pub targets: Vec<TargetBuildSettings>,
}

/// Build settings for a single target.
#[derive(Debug, Serialize)]
pub struct TargetBuildSettings {
    pub name: String,
    pub settings: std::collections::HashMap<String, String>,
}

// ── swift ─────────────────────────────────────────────────────────────────────

/// Structured result of `swift build`.
#[derive(Debug, Serialize)]
pub struct SwiftBuildResult {
    pub succeeded: bool,
    pub errors: Vec<Diagnostic>,
    pub warning_count: usize,
    pub warnings: Vec<Diagnostic>,
}

/// Structured result of `swift test`.
#[derive(Debug, Serialize)]
pub struct SwiftTestResult {
    pub succeeded: bool,
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<TestFailure>,
}

/// Structured result of `swift package resolve/update/show-dependencies`.
#[derive(Debug, Serialize)]
pub struct SwiftPackageResult {
    pub operation: String,
    pub packages: Vec<PackageEntry>,
    pub errors: Vec<String>,
}

/// A single resolved/updated Swift package.
#[derive(Debug, Clone, Serialize)]
pub struct PackageEntry {
    pub name: String,
    pub version: String,
    pub url: String,
}

// ── git ───────────────────────────────────────────────────────────────────────

/// Structured result of `git status`.
#[derive(Debug, Serialize)]
pub struct GitStatusResult {
    pub branch: Option<String>,
    pub staged: Vec<String>,
    pub modified: Vec<String>,
    pub untracked: Vec<String>,
}

/// Structured result of `git diff`.
#[derive(Debug, Serialize)]
pub struct GitDiffResult {
    pub files: Vec<DiffFile>,
    pub total_additions: i32,
    pub total_deletions: i32,
    pub file_count: usize,
}

/// Per-file diff statistics.
#[derive(Debug, Clone, Serialize)]
pub struct DiffFile {
    pub path: String,
    pub additions: i32,
    pub deletions: i32,
}

/// Structured result of `git log`.
#[derive(Debug, Serialize)]
pub struct GitLogResult {
    pub commits: Vec<CommitEntry>,
}

/// A single commit in a git log.
#[derive(Debug, Clone, Serialize)]
pub struct CommitEntry {
    pub hash: String,
    pub short_hash: String,
    pub subject: String,
    pub author: String,
    pub date: String,
    pub body_preview: Option<String>,
}

// ── linters / formatters ──────────────────────────────────────────────────────

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

/// Structured result of `swiftformat` output.
#[derive(Debug, Serialize)]
pub struct SwiftFormatResult {
    pub succeeded: bool,
    pub completed_line: Option<String>,
    pub changed_files: Vec<String>,
    pub lint_errors: Vec<String>,
}

// ── xcrun / simctl ────────────────────────────────────────────────────────────

/// Structured result of `xcrun simctl list`.
#[derive(Debug, Serialize)]
pub struct SimctlListResult {
    pub booted_count: usize,
    pub devices: Vec<SimDevice>,
}

/// A single iOS simulator device.
#[derive(Debug, Clone, Serialize)]
pub struct SimDevice {
    pub platform: String,
    pub name: String,
    pub udid: String,
    pub state: String,
}

// ── xcresulttool ──────────────────────────────────────────────────────────────

/// Structured result of `xcresulttool get`.
#[derive(Debug, Serialize)]
pub struct XcresultResult {
    pub status: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub warnings: usize,
}

// ── CocoaPods ────────────────────────────────────────────────────────────────

/// Structured result of `pod install` / `pod update`.
#[derive(Debug, Serialize)]
pub struct PodResult {
    pub succeeded: bool,
    pub installed_pods: Vec<String>,
    pub using_pods: Vec<String>,
    pub notices: Vec<String>,
    pub total_pods: usize,
}

// ── fastlane ─────────────────────────────────────────────────────────────────

/// Structured result of a `fastlane` invocation.
#[derive(Debug, Serialize)]
pub struct FastlaneResult {
    pub lane: Option<String>,
    pub succeeded: bool,
    pub steps: Vec<String>,
    pub issues: Vec<String>,
    pub total_time: Option<String>,
}

// ── codesign / security ───────────────────────────────────────────────────────

/// Structured result of `codesign --verify` or `codesign -d`.
#[derive(Debug, Serialize)]
pub struct CodesignResult {
    pub valid: Option<bool>,
    pub identifier: Option<String>,
    pub team: Option<String>,
    pub format: Option<String>,
    pub signature_size: Option<String>,
    pub errors: Vec<String>,
}

/// Structured result of `security find-identity`.
#[derive(Debug, Serialize)]
pub struct SecurityIdentityResult {
    pub identities: Vec<SecurityIdentity>,
    pub count: usize,
}

/// A single code signing identity.
#[derive(Debug, Clone, Serialize)]
pub struct SecurityIdentity {
    pub hash: String,
    pub name: String,
}

// ── curl ──────────────────────────────────────────────────────────────────────

/// Structured result of a `curl` invocation.
#[derive(Debug, Serialize)]
pub struct CurlResult {
    pub status_line: Option<String>,
    pub status_code: Option<u16>,
    pub headers: Vec<CurlHeader>,
    pub body_lines: usize,
    pub is_error: bool,
}

/// A single HTTP response header.
#[derive(Debug, Clone, Serialize)]
pub struct CurlHeader {
    pub name: String,
    pub value: String,
}

// ── grep ──────────────────────────────────────────────────────────────────────

/// Structured result of `grep` / `rg` output.
#[derive(Debug, Serialize)]
pub struct GrepResult {
    pub files: Vec<FileMatches>,
    pub total_matches: usize,
    pub file_count: usize,
}

/// Matches for a single file in grep output.
#[derive(Debug, Clone, Serialize)]
pub struct FileMatches {
    pub file: String,
    pub matches: Vec<String>,
    pub count: usize,
}

// ── ls / find ─────────────────────────────────────────────────────────────────

/// Structured result of `ls` / `find` filtered output.
#[derive(Debug, Serialize)]
pub struct LsResult {
    pub entries: Vec<String>,
    pub total_shown: usize,
}

// ── docc ──────────────────────────────────────────────────────────────────────

/// Structured result of `docc convert`.
#[derive(Debug, Serialize)]
pub struct DoccResult {
    pub succeeded: bool,
    pub symbols_line: Option<String>,
    pub warnings: Vec<String>,
    pub result_line: Option<String>,
}

// ── tuist ─────────────────────────────────────────────────────────────────────

/// Structured result of `tuist generate` / `tuist fetch`.
#[derive(Debug, Serialize)]
pub struct TuistResult {
    pub succeeded: bool,
    pub targets: Vec<String>,
    pub errors: Vec<String>,
    pub result: Option<String>,
}

// ── agvtool ───────────────────────────────────────────────────────────────────

/// Structured result of `agvtool` invocations.
#[derive(Debug, Serialize)]
pub struct AgvtoolResult {
    pub version: Option<String>,
    pub files_updated: usize,
}

// ── xcode-select ─────────────────────────────────────────────────────────────

/// Structured result of `xcode-select` invocations.
#[derive(Debug, Serialize)]
pub struct XcodeSelectResult {
    pub version: Option<String>,
    pub path: Option<String>,
}

// ── read (cat) ────────────────────────────────────────────────────────────────

/// Structured result of `cat` / file read output.
#[derive(Debug, Serialize)]
pub struct ReadResult {
    pub total_lines: usize,
    pub shown_lines: usize,
    pub is_binary: bool,
}

// ---------------------------------------------------------------------------
// periphery
// ---------------------------------------------------------------------------

#[derive(Debug, Default, serde::Serialize)]
pub struct PeripheryResult {
    pub files: Vec<PeripheryFileGroup>,
    pub total_symbols: usize,
    pub total_files: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct PeripheryFileGroup {
    pub path: String,
    pub symbols: Vec<PeripherySymbol>,
}

#[derive(Debug, serde::Serialize)]
pub struct PeripherySymbol {
    pub kind: String,
    pub name: String,
    pub line: u32,
}

// ---------------------------------------------------------------------------
// crashlog
// ---------------------------------------------------------------------------

#[derive(Debug, Default, serde::Serialize)]
pub struct CrashlogResult {
    pub exception_type: String,
    pub exception_subtype: String,
    pub app_name: String,
    pub app_version: String,
    pub device: String,
    pub os_version: String,
    pub crashed_thread: Vec<CrashFrame>,
    pub diagnosis: String,
}

#[derive(Debug, serde::Serialize)]
pub struct CrashFrame {
    pub index: u32,
    pub module: String,
    pub symbol: String,
    pub offset: String,
}

// ---------------------------------------------------------------------------
// project
// ---------------------------------------------------------------------------

#[derive(Debug, Default, serde::Serialize)]
pub struct ProjectResult {
    pub name: String,
    pub targets: Vec<ProjectTarget>,
    pub min_ios: String,
    pub swift_version: String,
    pub dependencies: Vec<ProjectDependency>,
    pub source_counts: ProjectSourceCounts,
    pub configurations: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ProjectTarget {
    pub name: String,
    pub bundle_id: String,
    pub kind: String,
}

#[derive(Debug, serde::Serialize)]
pub struct ProjectDependency {
    pub name: String,
    pub version: String,
    pub manager: String,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct ProjectSourceCounts {
    pub swift: usize,
    pub objc: usize,
    pub storyboards: usize,
    pub xibs: usize,
    pub resources: usize,
}
