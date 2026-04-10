//! Streaming line handlers for long-running commands.
//!
//! Each handler receives lines as they arrive from the subprocess and
//! selectively prints progress to stderr so the final filtered summary
//! on stdout remains clean and machine-parseable.

use crate::commands::CommandFamily;

/// A boxed streaming line handler.
type LineHandler = Box<dyn FnMut(&str)>;

/// Return a streaming line handler for the given command family.
///
/// The handler prints progress lines to stderr. Returns `None` for
/// command families that don't benefit from streaming.
pub fn handler_for(family: &CommandFamily) -> Option<LineHandler> {
    match family {
        CommandFamily::Xcodebuild(sub) => match sub {
            crate::commands::xcodebuild::XcodebuildSubcommand::Build
            | crate::commands::xcodebuild::XcodebuildSubcommand::Archive => {
                Some(Box::new(build_handler()))
            }
            crate::commands::xcodebuild::XcodebuildSubcommand::Test => {
                Some(Box::new(test_handler()))
            }
            _ => None,
        },
        CommandFamily::SwiftBuild(sub) => match sub {
            crate::commands::swift_build::SwiftBuildSubcommand::Build => {
                Some(Box::new(build_handler()))
            }
            crate::commands::swift_build::SwiftBuildSubcommand::Test => {
                Some(Box::new(test_handler()))
            }
            _ => None,
        },
        _ => None,
    }
}

/// Build handler: emits errors and build result as they appear.
fn build_handler() -> impl FnMut(&str) {
    let mut error_count = 0usize;
    move |line: &str| {
        if line.contains(": error:") {
            error_count += 1;
            // Show first 5 errors inline, then just count
            if error_count <= 5 {
                eprintln!("\x1b[31m  error:\x1b[0m {}", truncate(line, 120));
            } else if error_count == 6 {
                eprintln!("\x1b[31m  … more errors (will summarize at end)\x1b[0m");
            }
        } else if line.contains("** BUILD FAILED **") {
            eprintln!("\x1b[31m** BUILD FAILED **\x1b[0m");
        } else if line.contains("** BUILD SUCCEEDED **") {
            eprintln!("\x1b[32m** BUILD SUCCEEDED **\x1b[0m");
        } else if line.contains("Build complete!") {
            eprintln!("\x1b[32mBuild complete!\x1b[0m");
        } else if line.starts_with("Compiling ") || line.starts_with("CompileSwift ") {
            // Show compilation progress as a compact dot or module name
            if let Some(module) = extract_module(line) {
                eprintln!("\x1b[2m  compiling {module}\x1b[0m");
            }
        } else if line.starts_with("Linking ") || line.starts_with("Ld ") {
            eprintln!("\x1b[2m  linking…\x1b[0m");
        }
    }
}

/// Test handler: emits test pass/fail as they appear.
fn test_handler() -> impl FnMut(&str) {
    move |line: &str| {
        let trimmed = line.trim();
        if trimmed.contains("' passed (") {
            if let Some(name) = extract_test_name(trimmed) {
                eprintln!("  \x1b[32m✓\x1b[0m {name}");
            }
        } else if trimmed.contains("' failed (") {
            if let Some(name) = extract_test_name(trimmed) {
                eprintln!("  \x1b[31m✗\x1b[0m {name}");
            }
        } else if trimmed.contains("** TEST FAILED **") {
            eprintln!("\x1b[31m** TEST FAILED **\x1b[0m");
        } else if trimmed.contains("** TEST SUCCEEDED **") {
            eprintln!("\x1b[32m** TEST SUCCEEDED **\x1b[0m");
        } else if trimmed.contains("Test Suite") && trimmed.contains("started") {
            if let Some(suite) = extract_suite_name(trimmed) {
                eprintln!("\x1b[2m  suite: {suite}\x1b[0m");
            }
        }
    }
}

/// Extract module name from a compile line.
fn extract_module(line: &str) -> Option<&str> {
    // "Compiling MyModule ..." or "CompileSwift normal arm64 /path/to/File.swift"
    if let Some(rest) = line.strip_prefix("Compiling ") {
        return Some(rest.split_whitespace().next().unwrap_or(rest));
    }
    if line.starts_with("CompileSwift ") {
        // Extract filename from path at end
        return line.rsplit('/').next().map(|f| f.trim());
    }
    None
}

/// Extract test name from `Test Case '-[Suite testName]' passed/failed`.
fn extract_test_name(line: &str) -> Option<&str> {
    let start = line.find('\'')?;
    let end = line[start + 1..].find('\'')? + start + 1;
    Some(&line[start + 1..end])
}

/// Extract suite name from `Test Suite 'SuiteName' started`.
fn extract_suite_name(line: &str) -> Option<&str> {
    let start = line.find('\'')?;
    let end = line[start + 1..].find('\'')? + start + 1;
    Some(&line[start + 1..end])
}

/// Truncate a string to `max` characters, appending "…" if truncated.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

/// Check if streaming is appropriate for the given command family.
pub fn supports_streaming(family: &CommandFamily) -> bool {
    handler_for(family).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_module_from_compiling() {
        assert_eq!(extract_module("Compiling MyModule ..."), Some("MyModule"));
    }

    #[test]
    fn extract_module_from_compile_swift() {
        assert_eq!(
            extract_module("CompileSwift normal arm64 /Users/dev/App/ContentView.swift"),
            Some("ContentView.swift")
        );
    }

    #[test]
    fn extract_test_name_from_passed() {
        let line = "Test Case '-[MyTests testLogin]' passed (0.1 seconds)";
        assert_eq!(extract_test_name(line), Some("-[MyTests testLogin]"));
    }

    #[test]
    fn extract_test_name_from_failed() {
        let line = "Test Case '-[MyTests testPayment]' failed (0.5 seconds)";
        assert_eq!(extract_test_name(line), Some("-[MyTests testPayment]"));
    }

    #[test]
    fn extract_suite_name_works() {
        let line = "Test Suite 'MyAppTests' started at 2026-04-06 10:00:00";
        assert_eq!(extract_suite_name(line), Some("MyAppTests"));
    }

    #[test]
    fn truncate_short_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_adds_ellipsis() {
        assert_eq!(truncate("hello world", 5), "hello…");
    }

    #[test]
    fn supports_streaming_xcodebuild_build() {
        let family =
            CommandFamily::Xcodebuild(crate::commands::xcodebuild::XcodebuildSubcommand::Build);
        assert!(supports_streaming(&family));
    }

    #[test]
    fn supports_streaming_unknown_is_false() {
        assert!(!supports_streaming(&CommandFamily::Unknown));
    }

    #[test]
    fn supports_streaming_git_is_false() {
        let family = CommandFamily::Git(crate::commands::git::GitSubcommand::Status);
        assert!(!supports_streaming(&family));
    }
}
