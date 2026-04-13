//! Snapshot tests for filter output regression detection.
//!
//! These tests use `insta` to capture filter output as snapshots.
//! When filter logic changes, run `cargo insta review` to inspect diffs.

use sift_cli::filters::{self, Verbosity};

// ── xcodebuild build ────────────────────────────────────────────────────────

#[test]
fn xcodebuild_build_failed_compact() {
    let raw = include_str!("fixtures/xcodebuild_build_failed.txt");
    let out = filters::xcodebuild_build::filter(raw, Verbosity::Compact);
    insta::assert_snapshot!(out.content);
}

#[test]
fn xcodebuild_build_failed_verbose() {
    let raw = include_str!("fixtures/xcodebuild_build_failed.txt");
    let out = filters::xcodebuild_build::filter(raw, Verbosity::Verbose);
    insta::assert_snapshot!(out.content);
}

#[test]
fn xcodebuild_build_succeeded_compact() {
    let raw = include_str!("fixtures/xcodebuild_build_succeeded.txt");
    let out = filters::xcodebuild_build::filter(raw, Verbosity::Compact);
    insta::assert_snapshot!(out.content);
}

// ── xcodebuild test ─────────────────────────────────────────────────────────

#[test]
fn xcodebuild_test_failed_compact() {
    let raw = include_str!("fixtures/xcodebuild_test_failed.txt");
    let out = filters::xcodebuild_test::filter(raw, Verbosity::Compact);
    insta::assert_snapshot!(out.content);
}

#[test]
fn xcodebuild_test_succeeded_compact() {
    let raw = include_str!("fixtures/xcodebuild_test_succeeded.txt");
    let out = filters::xcodebuild_test::filter(raw, Verbosity::Compact);
    insta::assert_snapshot!(out.content);
}

// ── git status ──────────────────────────────────────────────────────────────

#[test]
fn git_status_complex_compact() {
    let raw = include_str!("fixtures/git_status_complex.txt");
    let out = filters::git_status::filter(raw, Verbosity::Compact);
    insta::assert_snapshot!(out.content);
}

// ── swiftlint ───────────────────────────────────────────────────────────────

#[test]
fn swiftlint_violations_compact() {
    let raw = include_str!("fixtures/swiftlint_violations.txt");
    let out = filters::swiftlint::filter(raw, Verbosity::Compact);
    insta::assert_snapshot!(out.content);
}

#[test]
fn swiftlint_violations_verbose() {
    let raw = include_str!("fixtures/swiftlint_violations.txt");
    let out = filters::swiftlint::filter(raw, Verbosity::Verbose);
    insta::assert_snapshot!(out.content);
}

// ── swift build ─────────────────────────────────────────────────────────────

#[test]
fn swift_build_errors_compact() {
    let raw = include_str!("fixtures/swift_build_errors.txt");
    let out = filters::swift_build::filter(raw, Verbosity::Compact);
    insta::assert_snapshot!(out.content);
}

#[test]
fn swift_build_errors_verbose() {
    let raw = include_str!("fixtures/swift_build_errors.txt");
    let out = filters::swift_build::filter(raw, Verbosity::Verbose);
    insta::assert_snapshot!(out.content);
}
