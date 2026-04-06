/// Integration tests for the `sift` binary.
///
/// Each test spawns the actual compiled binary via `CARGO_BIN_EXE_sift`
/// and validates stdout, stderr, and exit codes end-to-end.
use std::process::Command;

fn sift_bin() -> &'static str {
    env!("CARGO_BIN_EXE_sift")
}

#[test]
fn help_flag_exits_zero_and_shows_description() {
    let out = Command::new(sift_bin())
        .arg("-h")
        .output()
        .expect("failed to run sift");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Smart output reduction"));
}

#[test]
fn no_command_exits_nonzero() {
    let out = Command::new(sift_bin())
        .output()
        .expect("failed to run sift");
    assert!(!out.status.success());
}

#[test]
fn echo_passthrough_exits_zero_with_output() {
    let out = Command::new(sift_bin())
        .args(["echo", "hello_sift_test"])
        .output()
        .expect("failed to run sift");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("hello_sift_test"));
}

#[test]
fn raw_flag_preserves_output() {
    let out = Command::new(sift_bin())
        .args(["--raw", "echo", "raw_output_test"])
        .output()
        .expect("failed to run sift");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("raw_output_test"));
}

#[test]
fn exit_code_propagated_from_subprocess() {
    let out = Command::new(sift_bin())
        .args(["sh", "-c", "exit 42"])
        .output()
        .expect("failed to run sift");
    assert_eq!(out.status.code(), Some(42));
}

#[test]
fn zero_exit_code_propagated() {
    let out = Command::new(sift_bin())
        .args(["sh", "-c", "exit 0"])
        .output()
        .expect("failed to run sift");
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn unknown_command_not_found_exits_nonzero() {
    let out = Command::new(sift_bin())
        .args(["nonexistent_sift_binary_xyz_12345"])
        .output()
        .expect("failed to run sift");
    assert!(!out.status.success());
}

#[test]
fn stats_with_empty_store_shows_no_invocations() {
    let tmp = std::env::temp_dir().join("sift_integration_test_empty_stats");
    let out = Command::new(sift_bin())
        .arg("stats")
        .env("XDG_DATA_HOME", &tmp)
        .output()
        .expect("failed to run sift");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("No sift invocations recorded yet."));
}

#[test]
fn proxy_records_to_stats_then_stats_shows_data() {
    let tmp = std::env::temp_dir().join("sift_integration_test_tracking");
    // Run a proxy command to generate a tracking record
    let _ = Command::new(sift_bin())
        .args(["echo", "track_me"])
        .env("XDG_DATA_HOME", &tmp)
        .output()
        .expect("failed to run sift");
    // sift stats should now show at least 1 invocation
    let out = Command::new(sift_bin())
        .arg("stats")
        .env("XDG_DATA_HOME", &tmp)
        .output()
        .expect("failed to run sift");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Sift Statistics"));
    assert!(stdout.contains("Invocations:"));
}
