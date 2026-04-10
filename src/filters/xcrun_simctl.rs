use crate::filters::{FilterOutput, Verbosity};

/// Filter `xcrun simctl list` output.
///
/// Compact: iOS simulators only, one line per device, Booted first, short UDID.
/// Verbose: iOS simulators, full UDID, sorted Booted first per OS version.
/// VeryVerbose+: raw passthrough (all platforms).
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let devices = parse_devices(raw);

    // Compact/Verbose: iOS only; Raw handled above
    let ios_devices: Vec<&Device> = devices
        .iter()
        .filter(|d| d.platform.starts_with("iOS"))
        .collect();

    if ios_devices.is_empty() {
        // Nothing recognised — passthrough so we never hide useful output
        return FilterOutput::passthrough(raw);
    }

    let mut out = String::new();

    // Group by OS version, Booted devices first within each group
    let mut versions: Vec<&str> = ios_devices.iter().map(|d| d.platform.as_str()).collect();
    versions.dedup();
    // preserve insertion order but deduplicate
    let mut seen_versions: Vec<&str> = Vec::new();
    for v in &versions {
        if !seen_versions.contains(v) {
            seen_versions.push(v);
        }
    }

    let booted_count = ios_devices.iter().filter(|d| d.state == "Booted").count();
    if booted_count > 0 {
        out.push_str(&format!(
            "Simulators (iOS) — \x1b[32m{booted_count} booted\x1b[0m\n"
        ));
    } else {
        out.push_str("Simulators (iOS) — all shutdown\n");
    }

    for version in seen_versions {
        let group: Vec<&&Device> = ios_devices
            .iter()
            .filter(|d| d.platform == version)
            .collect();

        // Booted first, then alphabetical
        let mut sorted = group.clone();
        sorted.sort_by_key(|d| (d.state != "Booted", d.name.clone()));

        for device in sorted {
            let state_str = if device.state == "Booted" {
                "\x1b[32mBooted\x1b[0m  "
            } else {
                "Shutdown"
            };

            let udid = if verbosity == Verbosity::Verbose {
                device.udid.clone()
            } else {
                // Short UDID: first 8 chars
                device.udid.chars().take(8).collect()
            };

            let name = compact_device_name(&device.name);
            out.push_str(&format!(
                "  {version:<9}  {name:<28}  {state_str}  [{udid}]\n"
            ));
        }
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
        structured: None,
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

struct Device {
    platform: String,
    name: String,
    udid: String,
    state: String,
}

/// Parse `xcrun simctl list` text output into a flat list of devices.
///
/// The format is:
/// ```text
/// == Devices ==
/// -- iOS 18.0 --
///     iPhone 16 Pro (UDID-HERE) (Booted)
///     iPhone SE (3rd generation) (UDID-HERE) (Shutdown)
/// -- watchOS 11.0 --
///     ...
/// ```
fn parse_devices(raw: &str) -> Vec<Device> {
    let mut devices = Vec::new();
    let mut current_platform = String::new();
    let mut in_devices_section = false;

    for line in raw.lines() {
        let trimmed = line.trim();

        if trimmed == "== Devices ==" {
            in_devices_section = true;
            continue;
        }
        if trimmed.starts_with("== ") && trimmed.ends_with(" ==") {
            // New section (Runtimes, Device Types, etc.) — stop
            if in_devices_section {
                break;
            }
            continue;
        }

        if !in_devices_section {
            continue;
        }

        // Platform header: "-- iOS 18.0 --"
        if trimmed.starts_with("-- ") && trimmed.ends_with(" --") {
            current_platform = trimmed
                .trim_start_matches("-- ")
                .trim_end_matches(" --")
                .to_string();
            continue;
        }

        if current_platform.is_empty() || trimmed.is_empty() {
            continue;
        }

        // Device line: "  iPhone 16 Pro (UDID) (Booted)"
        if let Some(device) = parse_device_line(trimmed, &current_platform) {
            devices.push(device);
        }
    }

    devices
}

/// Parse a single device line into a `Device`.
///
/// Format: `Name (UDID) (State)` — where UDID is a UUID and State is
/// `Booted`, `Shutdown`, or `Creating`.
fn parse_device_line(line: &str, platform: &str) -> Option<Device> {
    // Find the last two parenthesised groups: (...state) and before it (...udid)
    let last_close = line.rfind(')')?;
    let last_open = line[..last_close].rfind('(')?;
    let state = line[last_open + 1..last_close].trim().to_string();

    let before_state = line[..last_open].trim_end();
    let udid_close = before_state.rfind(')')?;
    let udid_open = before_state[..udid_close].rfind('(')?;
    let udid = before_state[udid_open + 1..udid_close].trim().to_string();

    let name = before_state[..udid_open].trim().to_string();

    if name.is_empty() || udid.is_empty() {
        return None;
    }

    Some(Device {
        platform: platform.to_string(),
        name,
        udid,
        state,
    })
}

/// Shorten verbose device names for compact display.
fn compact_device_name(name: &str) -> String {
    name.replace("(3rd generation)", "(3rd gen)")
        .replace("(2nd generation)", "(2nd gen)")
        .replace("(1st generation)", "(1st gen)")
        .replace("(4th generation)", "(4th gen)")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
== Device Types ==
iPhone 16 Pro (com.apple.CoreSimulator.SimDeviceType.iPhone-16-Pro)
== Runtimes ==
iOS 18.0 (18.0 - 22A3351) - com.apple.CoreSimulator.SimRuntime.iOS-18-0
== Devices ==
-- iOS 17.5 --
    iPhone SE (3rd generation) (AABBCCDD-1122-3344-5566-778899AABBCC) (Shutdown)
    iPhone 15 Pro (DEADBEEF-1234-5678-9ABC-DEF012345678) (Booted)
-- iOS 18.0 --
    iPhone 16 (11111111-2222-3333-4444-555555555555) (Shutdown)
    iPhone 16 Pro (22222222-3333-4444-5555-666666666666) (Booted)
-- watchOS 11.0 --
    Apple Watch Series 9 - 41mm (FFFFFFFF-0000-1111-2222-333333333333) (Shutdown)
-- tvOS 18.0 --
    Apple TV 4K (3rd generation) (CCCCCCCC-DDDD-EEEE-FFFF-000000000000) (Shutdown)
";

    #[test]
    fn compact_shows_only_ios_devices() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.content.contains("iOS 17.5"));
        assert!(out.content.contains("iOS 18.0"));
        assert!(!out.content.contains("watchOS"));
        assert!(!out.content.contains("tvOS"));
    }

    #[test]
    fn compact_shows_booted_count_in_header() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.content.contains("2 booted"));
    }

    #[test]
    fn compact_uses_short_udid() {
        let out = filter(SAMPLE, Verbosity::Compact);
        // Short UDID = first 8 chars of DEADBEEF-...
        assert!(out.content.contains("DEADBEEF"));
        // Full UDID should NOT appear
        assert!(!out.content.contains("DEADBEEF-1234-5678-9ABC-DEF012345678"));
    }

    #[test]
    fn verbose_uses_full_udid() {
        let out = filter(SAMPLE, Verbosity::Verbose);
        assert!(out.content.contains("DEADBEEF-1234-5678-9ABC-DEF012345678"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE);
    }

    #[test]
    fn compact_shortens_generation_suffix() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.content.contains("3rd gen"));
        assert!(!out.content.contains("3rd generation"));
    }

    #[test]
    fn bytes_reduced_vs_original() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }
}

/// Filter simctl action commands: boot, install, launch, erase, delete.
///
/// These commands typically produce minimal output (empty on success, or a PID
/// for launch). The filter strips noise and shows a compact result.
/// VeryVerbose+: raw passthrough.
pub fn filter_simctl_action(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    // If empty output — success (simctl commands are silent on success)
    if raw.trim().is_empty() {
        let content = "\x1b[32m✓\x1b[0m Done\n".to_string();
        let filtered_bytes = content.len();
        return FilterOutput {
            content,
            original_bytes,
            filtered_bytes,
            structured: None,
        };
    }

    let mut out = String::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Already booted message
        if trimmed.contains("Unable to boot device") && trimmed.contains("current state: Booted") {
            out.push_str("\x1b[33mAlready booted\x1b[0m\n");
            continue;
        }

        // PID from launch: "com.example.app: 12345"
        if trimmed.contains(": ") {
            let parts: Vec<&str> = trimmed.splitn(2, ": ").collect();
            if parts.len() == 2 && parts[1].chars().all(|c| c.is_ascii_digit()) {
                out.push_str(&format!("Launched: PID {}\n", parts[1]));
                continue;
            }
        }

        // Error lines
        if trimmed.starts_with("An error") || trimmed.starts_with("error:") {
            out.push_str(&format!("\x1b[31m{trimmed}\x1b[0m\n"));
            continue;
        }

        // Pass other lines through
        out.push_str(&format!("{trimmed}\n"));
    }

    if out.is_empty() {
        return FilterOutput::passthrough(raw);
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
        structured: None,
    }
}

#[cfg(test)]
mod simctl_action_tests {
    use super::*;

    #[test]
    fn empty_output_shows_done() {
        let out = filter_simctl_action("", Verbosity::Compact);
        assert!(out.content.contains("Done"));
    }

    #[test]
    fn whitespace_only_shows_done() {
        let out = filter_simctl_action("   \n  ", Verbosity::Compact);
        assert!(out.content.contains("Done"));
    }

    #[test]
    fn launch_pid_shown() {
        let out = filter_simctl_action("com.example.app: 12345\n", Verbosity::Compact);
        assert!(out.content.contains("PID 12345"));
    }

    #[test]
    fn already_booted_message_shown() {
        let raw = "Unable to boot device in current state: Booted\n";
        let out = filter_simctl_action(raw, Verbosity::Compact);
        assert!(out.content.contains("Already booted"));
    }

    #[test]
    fn error_line_colored() {
        let raw = "An error was encountered processing the command (domain=NSPOSIXErrorDomain, code=1).\n";
        let out = filter_simctl_action(raw, Verbosity::Compact);
        assert!(out.content.contains("An error"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let raw = "com.example.app: 99\n";
        let out = filter_simctl_action(raw, Verbosity::VeryVerbose);
        assert_eq!(out.content, raw);
    }
}
