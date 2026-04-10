//! Error recall tests — verify that filters never drop critical error signals.
//!
//! Each fixture has a companion `.errors.json` manifest listing text fragments
//! that MUST appear in the filtered output. If a filter drops any of them,
//! the test fails with a clear message about what was lost.

use serde::Deserialize;
use sift_cli::filters::{self, Verbosity};

#[derive(Deserialize)]
struct RecallManifest {
    errors: Vec<ExpectedError>,
}

#[derive(Deserialize, Debug)]
struct ExpectedError {
    text: String,
    severity: String,
}

fn assert_recall(filter_output: &str, manifest_json: &str, fixture_name: &str) {
    let manifest: RecallManifest =
        serde_json::from_str(manifest_json).expect("invalid manifest JSON");

    let missed: Vec<&ExpectedError> = manifest
        .errors
        .iter()
        .filter(|e| !filter_output.contains(&e.text))
        .collect();

    assert!(
        missed.is_empty(),
        "Filter dropped {}/{} expected signals in {fixture_name}:\n{}",
        missed.len(),
        manifest.errors.len(),
        missed
            .iter()
            .map(|e| format!(
                "  [{severity}] \"{text}\"",
                severity = e.severity,
                text = e.text
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ── xcodebuild build ────────────────────────────────────────────────────────

#[test]
fn recall_xcodebuild_build_failed_compact() {
    let raw = include_str!("fixtures/xcodebuild_build_failed.txt");
    let manifest = include_str!("fixtures/xcodebuild_build_failed.errors.json");
    let out = filters::xcodebuild_build::filter(raw, Verbosity::Compact);
    assert_recall(&out.content, manifest, "xcodebuild_build_failed (compact)");
}

#[test]
fn recall_xcodebuild_build_failed_verbose() {
    let raw = include_str!("fixtures/xcodebuild_build_failed.txt");
    let manifest = include_str!("fixtures/xcodebuild_build_failed.errors.json");
    let out = filters::xcodebuild_build::filter(raw, Verbosity::Verbose);
    assert_recall(&out.content, manifest, "xcodebuild_build_failed (verbose)");
}

// ── xcodebuild test ─────────────────────────────────────────────────────────

#[test]
fn recall_xcodebuild_test_failed_compact() {
    let raw = include_str!("fixtures/xcodebuild_test_failed.txt");
    let manifest = include_str!("fixtures/xcodebuild_test_failed.errors.json");
    let out = filters::xcodebuild_test::filter(raw, Verbosity::Compact);
    assert_recall(&out.content, manifest, "xcodebuild_test_failed (compact)");
}

#[test]
fn recall_xcodebuild_test_failed_verbose() {
    let raw = include_str!("fixtures/xcodebuild_test_failed.txt");
    let manifest = include_str!("fixtures/xcodebuild_test_failed.errors.json");
    let out = filters::xcodebuild_test::filter(raw, Verbosity::Verbose);
    assert_recall(&out.content, manifest, "xcodebuild_test_failed (verbose)");
}

// ── swift build ─────────────────────────────────────────────────────────────

#[test]
fn recall_swift_build_errors_compact() {
    let raw = include_str!("fixtures/swift_build_errors.txt");
    let manifest = include_str!("fixtures/swift_build_errors.errors.json");
    let out = filters::swift_build::filter(raw, Verbosity::Compact);
    assert_recall(&out.content, manifest, "swift_build_errors (compact)");
}

#[test]
fn recall_swift_build_errors_verbose() {
    let raw = include_str!("fixtures/swift_build_errors.txt");
    let manifest = include_str!("fixtures/swift_build_errors.errors.json");
    let out = filters::swift_build::filter(raw, Verbosity::Verbose);
    assert_recall(&out.content, manifest, "swift_build_errors (verbose)");
}

// ── swiftlint ───────────────────────────────────────────────────────────────

#[test]
fn recall_swiftlint_violations_compact() {
    let raw = include_str!("fixtures/swiftlint_violations.txt");
    let manifest = include_str!("fixtures/swiftlint_violations.errors.json");
    let out = filters::swiftlint::filter(raw, Verbosity::Compact);
    assert_recall(&out.content, manifest, "swiftlint_violations (compact)");
}

#[test]
fn recall_swiftlint_violations_verbose() {
    let raw = include_str!("fixtures/swiftlint_violations.txt");
    let manifest = include_str!("fixtures/swiftlint_violations.errors.json");
    let out = filters::swiftlint::filter(raw, Verbosity::Verbose);
    assert_recall(&out.content, manifest, "swiftlint_violations (verbose)");
}
