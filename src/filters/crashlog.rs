use crate::filters::types::{CrashFrame, CrashlogResult};
use crate::filters::{FilterOutput, Verbosity};

/// Filter Apple crash report files (`.crash` text format and `.ips` JSON format).
///
/// Compact: exception type, device/OS, app version, crashed thread backtrace
///          (first 10 app frames, system frames collapsed), optional diagnosis.
/// Verbose: first 20 frames + all threads summary.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    // Detect format: .ips files start with `{` (JSON)
    let result = if raw.trim_start().starts_with('{') {
        parse_ips(raw)
    } else {
        parse_crash(raw)
    };

    let content = render(&result, verbosity);
    let filtered_bytes = content.len();
    let structured = serde_json::to_value(&result).ok();

    FilterOutput {
        content,
        original_bytes,
        filtered_bytes,
        structured,
    }
}

// ---------------------------------------------------------------------------
// .crash parser (plain text Apple crash format)
// ---------------------------------------------------------------------------

pub fn parse_crash(raw: &str) -> CrashlogResult {
    let mut result = CrashlogResult::default();
    let mut in_crashed_thread = false;
    let mut frame_count = 0u32;

    for line in raw.lines() {
        let trimmed = line.trim();

        // Exception type
        if let Some(rest) = trimmed.strip_prefix("Exception Type:") {
            result.exception_type = rest.trim().to_string();
            continue;
        }

        // Exception subtype
        if let Some(rest) = trimmed.strip_prefix("Exception Subtype:") {
            result.exception_subtype = rest.trim().to_string();
            continue;
        }

        // App version: "Version: 2.1.0 (47)"
        if trimmed.starts_with("Version:") && result.app_version.is_empty() {
            result.app_version = trimmed
                .strip_prefix("Version:")
                .unwrap_or("")
                .trim()
                .to_string();
            continue;
        }

        // Process name
        if trimmed.starts_with("Process:") && result.app_name.is_empty() {
            result.app_name = trimmed
                .strip_prefix("Process:")
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            continue;
        }

        // Hardware model
        if let Some(rest) = trimmed.strip_prefix("Hardware Model:") {
            result.device = rest.trim().to_string();
            continue;
        }

        // OS version
        if trimmed.starts_with("OS Version:") && result.os_version.is_empty() {
            result.os_version = trimmed
                .strip_prefix("OS Version:")
                .unwrap_or("")
                .trim()
                .to_string();
            continue;
        }

        // Detect crashed thread header: "Thread N Crashed:" or "Thread N name:  Dispatch queue: com.apple.main-thread"
        if trimmed.contains("Crashed:") && trimmed.starts_with("Thread") {
            in_crashed_thread = true;
            frame_count = 0;
            continue;
        }

        // Detect end of crashed thread (empty line or new Thread section)
        if in_crashed_thread {
            if trimmed.is_empty()
                || (trimmed.starts_with("Thread") && !trimmed.contains("Crashed:"))
            {
                in_crashed_thread = false;
                continue;
            }

            if let Some(frame) = parse_crash_frame(trimmed) {
                frame_count += 1;
                result.crashed_thread.push(frame);
                // Keep first 10 frames in compact
                if frame_count >= 10 {
                    in_crashed_thread = false;
                }
            }
        }
    }

    result.diagnosis = build_diagnosis(&result);
    result
}

fn parse_crash_frame(line: &str) -> Option<CrashFrame> {
    // Format: "0   MyApp   0x00000001045abc12 CheckoutViewModel.checkout(with:) + 48"
    let mut parts = line.split_whitespace();
    let index: u32 = parts.next()?.parse().ok()?;
    let module = parts.next()?.to_string();
    let _address = parts.next()?; // skip address

    let rest: Vec<&str> = parts.collect();
    if rest.is_empty() {
        return None;
    }

    // Split on " + " to separate symbol from offset
    let joined = rest.join(" ");
    let (symbol, offset) = if let Some(pos) = joined.rfind(" + ") {
        (
            joined[..pos].trim().to_string(),
            joined[pos + 3..].trim().to_string(),
        )
    } else {
        (joined, String::new())
    };

    Some(CrashFrame {
        index,
        module,
        symbol,
        offset,
    })
}

// ---------------------------------------------------------------------------
// .ips parser (JSON format, iOS 15+)
// ---------------------------------------------------------------------------

pub fn parse_ips(raw: &str) -> CrashlogResult {
    let mut result = CrashlogResult::default();

    // .ips files have two JSON documents separated by a newline:
    // Line 1: metadata JSON {"app_name":...,"os_version":...}
    // Rest: full crash JSON
    let mut lines = raw.lines();
    let first_line = lines.next().unwrap_or("").trim();
    let rest: String = lines.collect::<Vec<_>>().join("\n");

    // Parse metadata from first line
    result.app_name = extract_json_string(first_line, "app_name");
    result.app_version = extract_json_string(first_line, "app_version");
    if result.app_version.is_empty() {
        result.app_version = extract_json_string(first_line, "bundle_version");
    }
    result.os_version = extract_json_string(first_line, "os_version");
    result.device = extract_json_string(first_line, "modelCode");
    if result.device.is_empty() {
        result.device = extract_json_string(first_line, "model");
    }

    // Parse main crash JSON
    result.exception_type = extract_json_string(&rest, "type");
    result.exception_subtype = extract_json_string(&rest, "subtype");

    if result.app_name.is_empty() {
        result.app_name = extract_json_string(&rest, "procName");
    }
    if result.os_version.is_empty() {
        result.os_version = extract_json_string(&rest, "osVersion");
    }

    // Extract crashed thread frames from JSON
    // Look for "\"triggered\":true" thread, then its "frames" array
    result.crashed_thread = extract_ips_frames(&rest);

    result.diagnosis = build_diagnosis(&result);
    result
}

/// Minimal JSON string field extractor — no full JSON parser needed.
fn extract_json_string(json: &str, key: &str) -> String {
    let needle = format!("\"{}\"", key);
    if let Some(pos) = json.find(&needle) {
        let after_key = &json[pos + needle.len()..];
        // Skip whitespace and ':'
        let after_colon = after_key.trim_start().trim_start_matches(':').trim_start();
        if after_colon.starts_with('"') {
            // String value — strip leading quote
            let inner = after_colon.strip_prefix('"').unwrap_or(after_colon);
            if let Some(end) = inner.find('"') {
                return inner[..end].to_string();
            }
        }
    }
    String::new()
}

fn extract_ips_frames(json: &str) -> Vec<CrashFrame> {
    let mut frames = Vec::new();

    // Find triggered thread
    let triggered_marker = "\"triggered\":true";
    let search_from = if let Some(pos) = json.find(triggered_marker) {
        // Back-track to find start of this thread object
        json[..pos].rfind('{').unwrap_or(pos)
    } else {
        // Fallback: first thread
        0
    };

    let thread_section = &json[search_from..];

    // Find "frames" array in this thread
    let frames_marker = "\"frames\"";
    if let Some(pos) = thread_section.find(frames_marker) {
        let after = &thread_section[pos + frames_marker.len()..];
        let after = after.trim_start().trim_start_matches(':').trim_start();

        if after.starts_with('[') {
            // Walk through frame objects
            let mut depth = 0i32;
            let mut frame_start = None;
            let mut index = 0u32;

            for (i, ch) in after.char_indices() {
                match ch {
                    '{' => {
                        depth += 1;
                        if depth == 1 {
                            frame_start = Some(i);
                        }
                    }
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            if let Some(start) = frame_start {
                                let frame_json = &after[start..=i];
                                let symbol = extract_json_string(frame_json, "symbol");
                                let image_offset = extract_json_string(frame_json, "imageOffset");
                                let module = extract_json_string(frame_json, "imageName");

                                let sym_display = if symbol.is_empty() {
                                    format!("0x{}", image_offset)
                                } else {
                                    symbol
                                };

                                frames.push(CrashFrame {
                                    index,
                                    module,
                                    symbol: sym_display,
                                    offset: image_offset,
                                });
                                index += 1;
                                frame_start = None;

                                if index >= 10 {
                                    break;
                                }
                            }
                        }
                    }
                    ']' if depth == 0 => break,
                    _ => {}
                }
            }
        }
    }

    frames
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(result: &CrashlogResult, verbosity: Verbosity) -> String {
    let mut out = String::new();

    // Header
    let exception = if result.exception_subtype.is_empty() {
        result.exception_type.clone()
    } else {
        format!("{} — {}", result.exception_type, result.exception_subtype)
    };

    out.push_str(&format!("CRASH  {}\n", exception));

    let mut meta = Vec::new();
    if !result.device.is_empty() {
        meta.push(result.device.clone());
    }
    if !result.os_version.is_empty() {
        meta.push(result.os_version.clone());
    }
    if !result.app_name.is_empty() && !result.app_version.is_empty() {
        meta.push(format!("{} {}", result.app_name, result.app_version));
    } else if !result.app_name.is_empty() {
        meta.push(result.app_name.clone());
    }
    if !meta.is_empty() {
        out.push_str(&format!("Device: {}\n", meta.join("  |  ")));
    }

    out.push('\n');

    // Backtrace
    let frame_limit = if matches!(verbosity, Verbosity::Verbose) {
        20
    } else {
        10
    };

    if !result.crashed_thread.is_empty() {
        out.push_str("Thread (crashed):\n");
        let mut system_run = 0usize;

        for frame in result.crashed_thread.iter().take(frame_limit) {
            let is_system = is_system_module(&frame.module);
            if is_system && matches!(verbosity, Verbosity::Compact) {
                system_run += 1;
                continue;
            }

            if system_run > 0 {
                out.push_str(&format!(
                    "  [{} system frame{}]\n",
                    system_run,
                    if system_run > 1 { "s" } else { "" }
                ));
                system_run = 0;
            }

            let offset_str = if frame.offset.is_empty() {
                String::new()
            } else {
                format!(" + {}", frame.offset)
            };
            out.push_str(&format!(
                "  {}  {}  {}{}\n",
                frame.index, frame.module, frame.symbol, offset_str
            ));
        }

        if system_run > 0 {
            out.push_str(&format!(
                "  [{} system frame{}]\n",
                system_run,
                if system_run > 1 { "s" } else { "" }
            ));
        }
    } else {
        out.push_str("Thread (crashed): [no frames extracted]\n");
    }

    // Diagnosis
    if !result.diagnosis.is_empty() {
        out.push('\n');
        out.push_str(&format!("Diagnosis: {}\n", result.diagnosis));
    }

    out
}

fn is_system_module(module: &str) -> bool {
    const SYSTEM_PREFIXES: &[&str] = &[
        "libsystem",
        "libobjc",
        "CoreFoundation",
        "UIKit",
        "Foundation",
        "libdispatch",
        "libc++",
        "libswift",
        "SwiftCore",
        "dyld",
        "CFNetwork",
        "libxpc",
        "IOKit",
        "Security",
        "CoreData",
    ];
    SYSTEM_PREFIXES.iter().any(|p| module.starts_with(p))
}

fn build_diagnosis(result: &CrashlogResult) -> String {
    let et = result.exception_type.to_lowercase();
    let es = result.exception_subtype.to_lowercase();

    if et.contains("exc_bad_access") || es.contains("kern_invalid_address") {
        if let Some(first) = result.crashed_thread.first() {
            return format!(
                "Null pointer dereference in {} — likely force unwrap or dangling reference",
                first.module
            );
        }
        return "Null pointer dereference — likely force unwrap or dangling reference".to_string();
    }

    if et.contains("exc_crash") && es.contains("sigabrt") {
        return "Process aborted — likely assertion failure or uncaught exception".to_string();
    }

    if et.contains("exc_breakpoint") {
        return "Swift runtime error — check for forced unwrap (!) or out-of-bounds array access"
            .to_string();
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CRASH: &str = r#"Incident Identifier: 12AB34CD-FAKE-0000-0000-000000000000
Hardware Model:      iPhone15,2
Process:             MyApp [1234]
Path:                /private/var/containers/Bundle/Application/ABCD/MyApp.app/MyApp
Identifier:          com.example.myapp
Version:             2.1.0 (47)
AppStoreTools:       14A309
Code Type:           ARM-64 (Native)
Role:                Foreground
Parent Process:      launchd [1]
Coalition:           com.example.myapp [1234]
Date/Time:           2024-01-15 12:34:56.789 +0000
Launch Time:         2024-01-15 12:30:00.000 +0000
OS Version:          iPhone OS 17.2 (21C62)
Release Type:        User
Baseband Version:    2.50.00
Report Version:      104

Exception Type:  EXC_BAD_ACCESS (SIGSEGV)
Exception Subtype: KERN_INVALID_ADDRESS at 0x0000000000000010
VM Region Info: 0x10 is not in any region.  Bytes before following region: 4367638528
      REGION TYPE                    START - END         [ VSIZE] PRT/MAX SHRMOD  REGION DETAIL
      UNUSED SPACE AT START
--->  
      __TEXT                      104000000-104800000    [ 8192K] r-x/r-x SM=COW  ...MyApp

Termination Reason: SIGNAL 11 Segmentation fault: 11
Terminating Process: exc handler [1234]

Triggered by Thread:  0

Thread 0 name:  Dispatch queue: com.apple.main-thread
Thread 0 Crashed:
0   MyApp                    0x00000001045abc12 CheckoutViewModel.checkout(with:) + 48
1   MyApp                    0x00000001045abc45 closure #1 in CheckoutViewModel.loadCart() + 124
2   libswiftCore.dylib        0x0000000180000000 swift_task_run + 92
3   libdispatch.dylib         0x0000000181000000 _dispatch_main_queue_callback_4CF + 44
4   CoreFoundation             0x0000000182000000 __CFRunLoopRun + 1996
5   CoreFoundation             0x0000000182000100 CFRunLoopRunSpecific + 608
6   UIKit                      0x0000000183000000 UIApplicationMain + 800
7   MyApp                      0x00000001045abc99 main + 56

Thread 1:
0   libsystem_kernel.dylib   0x0000000182000000 mach_msg_trap + 8
1   libsystem_kernel.dylib   0x0000000182000010 mach_msg + 72
2   CoreFoundation             0x0000000182100000 __CFRunLoopServiceMachPort + 372
3   CoreFoundation             0x0000000182100100 __CFRunLoopRun + 2048
4   CoreFoundation             0x0000000182100200 CFRunLoopRunSpecific + 608
5   Foundation                 0x0000000183100000 -[NSRunLoop(NSRunLoop) runMode:beforeDate:] + 228

Thread 2:
0   libsystem_pthread.dylib  0x0000000182200000 start_wqthread + 8
1   libsystem_pthread.dylib  0x0000000182200100 _pthread_wqthread + 312

Thread 3 name:  com.apple.uikit.eventfetch-thread
Thread 3:
0   libsystem_kernel.dylib   0x0000000182000000 mach_msg_trap + 8
1   libsystem_kernel.dylib   0x0000000182000010 mach_msg + 72
2   CoreFoundation             0x0000000182100000 __CFRunLoopServiceMachPort + 372

Binary Images:
0x100000000 - 0x1047fffff MyApp arm64 <AAAAAAAABBBBCCCCDDDDEEEEFFFFGGGG> /private/var/containers/Bundle/Application/ABCD/MyApp.app/MyApp
0x180000000 - 0x18001ffff libswiftCore.dylib arm64 <1111222233334444> /usr/lib/swift/libswiftCore.dylib
0x181000000 - 0x1811fffff libdispatch.dylib arm64 <5555666677778888> /usr/lib/libdispatch.dylib
0x182000000 - 0x1829fffff CoreFoundation arm64 <9999AAAABBBBCCCC> /System/Library/Frameworks/CoreFoundation.framework/CoreFoundation
0x183000000 - 0x1839fffff UIKit arm64 <DDDDEEEEFFFFGGGG> /System/Library/Frameworks/UIKit.framework/UIKit
0x184000000 - 0x1849fffff Foundation arm64 <HHHHIIIIJJJJKKKK> /System/Library/Frameworks/Foundation.framework/Foundation
0x185000000 - 0x1850fffff libsystem_kernel.dylib arm64 <LLLLMMMMNNNNOOO0> /usr/lib/system/libsystem_kernel.dylib
0x186000000 - 0x1860fffff libsystem_pthread.dylib arm64 <PPPPQQQQRRRRSSSS> /usr/lib/system/libsystem_pthread.dylib
"#;

    #[test]
    fn parses_exception_type() {
        let result = parse_crash(SAMPLE_CRASH);
        assert!(result.exception_type.contains("EXC_BAD_ACCESS"));
    }

    #[test]
    fn parses_device_and_os() {
        let result = parse_crash(SAMPLE_CRASH);
        assert_eq!(result.device, "iPhone15,2");
        assert!(result.os_version.contains("17.2"));
    }

    #[test]
    fn parses_app_version() {
        let result = parse_crash(SAMPLE_CRASH);
        assert!(result.app_version.contains("2.1.0"));
    }

    #[test]
    fn extracts_crashed_thread_frames() {
        let result = parse_crash(SAMPLE_CRASH);
        assert!(!result.crashed_thread.is_empty());
        assert_eq!(
            result.crashed_thread[0].symbol,
            "CheckoutViewModel.checkout(with:)"
        );
    }

    #[test]
    fn compact_output_contains_exception() {
        let output = filter(SAMPLE_CRASH, Verbosity::Compact);
        assert!(output.content.contains("EXC_BAD_ACCESS"));
    }

    #[test]
    fn compact_output_contains_app_frame() {
        let output = filter(SAMPLE_CRASH, Verbosity::Compact);
        assert!(output.content.contains("CheckoutViewModel"));
    }

    #[test]
    fn reduces_bytes_significantly() {
        let output = filter(SAMPLE_CRASH, Verbosity::Compact);
        assert!(output.filtered_bytes < output.original_bytes / 2);
    }

    #[test]
    fn generates_diagnosis_for_sigsegv() {
        let result = parse_crash(SAMPLE_CRASH);
        assert!(!result.diagnosis.is_empty());
        assert!(
            result.diagnosis.to_lowercase().contains("null pointer")
                || result.diagnosis.to_lowercase().contains("force unwrap")
        );
    }
}
