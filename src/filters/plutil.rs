//! Filter for `sift plutil` — parses iOS/macOS .plist and .entitlements files.
//!
//! Shows bundle ID, display name, version, device families, privacy permission
//! keys, and capabilities/entitlements in a compact, human-readable format.

use crate::filters::types::PlutilResult;
use crate::filters::{FilterOutput, Verbosity};

/// Known privacy permission keys from Apple's Info.plist reference.
static PRIVACY_PREFIXES: &[&str] = &[
    "NSCameraUsageDescription",
    "NSMicrophoneUsageDescription",
    "NSLocationWhenInUseUsageDescription",
    "NSLocationAlwaysAndWhenInUseUsageDescription",
    "NSLocationAlwaysUsageDescription",
    "NSPhotoLibraryUsageDescription",
    "NSPhotoLibraryAddUsageDescription",
    "NSContactsUsageDescription",
    "NSCalendarsUsageDescription",
    "NSRemindersUsageDescription",
    "NSMotionUsageDescription",
    "NSHealthShareUsageDescription",
    "NSHealthUpdateUsageDescription",
    "NSBluetoothAlwaysUsageDescription",
    "NSBluetoothPeripheralUsageDescription",
    "NSFaceIDUsageDescription",
    "NSSpeechRecognitionUsageDescription",
    "NSUserTrackingUsageDescription",
    "NSLocalNetworkUsageDescription",
    "NSNearbyInteractionUsageDescription",
    "NFCReaderUsageDescription",
    "NSAppleMusicUsageDescription",
];

/// Known entitlement / capability keys.
static CAPABILITY_PREFIXES: &[&str] = &[
    "com.apple.developer.",
    "com.apple.security.",
    "aps-environment",
];

pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let result = parse(raw);
    let content = format_result(&result, verbosity);
    let filtered_bytes = content.len();

    FilterOutput {
        content,
        original_bytes,
        filtered_bytes,
        structured: serde_json::to_value(&result).ok(),
    }
}

pub fn parse(raw: &str) -> PlutilResult {
    let mut r = PlutilResult::default();

    let pairs = collect_plist_pairs(raw);
    for (key, value) in &pairs {
        match key.as_str() {
            "CFBundleIdentifier" => r.bundle_id = value.clone(),
            "CFBundleDisplayName" | "CFBundleName" if r.display_name.is_empty() => {
                r.display_name = value.clone()
            }
            "CFBundleShortVersionString" => r.version = value.clone(),
            "CFBundleVersion" => r.build = value.clone(),
            "MinimumOSVersion" | "LSMinimumSystemVersion" | "MinimumSystemVersion" => {
                if r.min_os.is_empty() {
                    r.min_os = value.clone()
                }
            }
            "UIDeviceFamily" => {
                r.device_families = parse_device_families(value);
            }
            k if PRIVACY_PREFIXES.contains(&k) => {
                r.privacy_keys.push(k.to_string());
            }
            k if CAPABILITY_PREFIXES.iter().any(|p| k.starts_with(p)) => {
                r.capabilities.push(k.to_string());
            }
            _ => {}
        }
    }

    r
}

fn collect_plist_pairs(raw: &str) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut pending_key: Option<String> = None;

    for line in raw.lines() {
        let t = line.trim();

        // XML: <key>SomeName</key>
        if t.starts_with("<key>") && t.ends_with("</key>") {
            let key = t
                .trim_start_matches("<key>")
                .trim_end_matches("</key>")
                .to_string();
            pending_key = Some(key);
            continue;
        }

        if let Some(key) = pending_key.take() {
            let value = extract_xml_value(t);
            pairs.push((key, value));
            continue;
        }

        // JSON plist: "key" : value
        if t.starts_with('"') {
            if let Some(colon) = t.find("\" :") {
                let key = t[1..colon].to_string();
                let rest = t[colon + 3..].trim();
                let value = rest.trim_matches('"').trim_end_matches(',').to_string();
                pairs.push((key, value));
                continue;
            }
            // JSON "key": value
            if let Some(colon) = t.find("\": ") {
                let key = t[1..colon].to_string();
                let rest = t[colon + 3..].trim();
                let value = rest.trim_matches('"').trim_end_matches(',').to_string();
                pairs.push((key, value));
                continue;
            }
        }
    }

    pairs
}

fn extract_xml_value(t: &str) -> String {
    for tag in &[
        "string", "integer", "real", "true", "false", "array", "dict",
    ] {
        let open = format!("<{}>", tag);
        let close = format!("</{}>", tag);
        if t.starts_with(&open) {
            if *tag == "true" {
                return "true".to_string();
            }
            if *tag == "false" {
                return "false".to_string();
            }
            if *tag == "array" {
                return "[array]".to_string();
            }
            if *tag == "dict" {
                return "[dict]".to_string();
            }
            return t
                .trim_start_matches(&*open)
                .trim_end_matches(&*close)
                .to_string();
        }
    }
    // Self-closing <true/> <false/>
    if t == "<true/>" {
        return "true".to_string();
    }
    if t == "<false/>" {
        return "false".to_string();
    }
    t.to_string()
}

fn parse_device_families(raw: &str) -> Vec<String> {
    // Values: 1=iPhone, 2=iPad, 3=TV, 4=Watch, 7=Vision
    let mut families = Vec::new();
    for ch in raw.chars() {
        match ch {
            '1' => families.push("iPhone".to_string()),
            '2' => families.push("iPad".to_string()),
            '3' => families.push("AppleTV".to_string()),
            '4' => families.push("Apple Watch".to_string()),
            '7' => families.push("Apple Vision".to_string()),
            _ => {}
        }
    }
    families
}

fn format_result(result: &PlutilResult, verbosity: Verbosity) -> String {
    let mut out = String::new();

    if !result.bundle_id.is_empty() {
        out.push_str(&format!("Bundle ID:   {}\n", result.bundle_id));
    }
    if !result.display_name.is_empty() {
        out.push_str(&format!("Name:        {}\n", result.display_name));
    }
    if !result.version.is_empty() {
        let build = if result.build.is_empty() {
            String::new()
        } else {
            format!(" ({})", result.build)
        };
        out.push_str(&format!("Version:     {}{}\n", result.version, build));
    }
    if !result.min_os.is_empty() {
        out.push_str(&format!("Min OS:      {}\n", result.min_os));
    }
    if !result.device_families.is_empty() {
        out.push_str(&format!(
            "Devices:     {}\n",
            result.device_families.join(", ")
        ));
    }

    if !result.privacy_keys.is_empty() {
        out.push_str(&format!("\nPrivacy ({}):\n", result.privacy_keys.len()));
        for k in &result.privacy_keys {
            out.push_str(&format!("  {}\n", k));
        }
    }

    if !result.capabilities.is_empty() {
        out.push_str(&format!(
            "\nCapabilities ({}):\n",
            result.capabilities.len()
        ));
        for c in &result.capabilities {
            out.push_str(&format!("  {}\n", c));
        }
    }

    if matches!(verbosity, Verbosity::Verbose) && !result.extra.is_empty() {
        out.push_str("\nOther:\n");
        for (k, v) in &result.extra {
            out.push_str(&format!("  {} = {}\n", k, v));
        }
    }

    if out.is_empty() {
        out.push_str("(no recognized plist keys found)\n");
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleIdentifier</key>
	<string>com.example.myapp</string>
	<key>CFBundleDisplayName</key>
	<string>My App</string>
	<key>CFBundleShortVersionString</key>
	<string>2.1.0</string>
	<key>CFBundleVersion</key>
	<string>42</string>
	<key>MinimumOSVersion</key>
	<string>17.0</string>
	<key>UIDeviceFamily</key>
	<array>
		<integer>1</integer>
		<integer>2</integer>
	</array>
	<key>NSCameraUsageDescription</key>
	<string>We need camera access to scan QR codes.</string>
	<key>NSMicrophoneUsageDescription</key>
	<string>We need microphone for voice notes.</string>
	<key>NSPhotoLibraryUsageDescription</key>
	<string>Select photos to attach.</string>
	<key>com.apple.developer.push-notifications</key>
	<true/>
	<key>com.apple.developer.associated-domains</key>
	<array/>
</dict>
</plist>
"#;

    #[test]
    fn parses_bundle_id() {
        let r = parse(SAMPLE_PLIST);
        assert_eq!(r.bundle_id, "com.example.myapp");
    }

    #[test]
    fn parses_display_name() {
        let r = parse(SAMPLE_PLIST);
        assert_eq!(r.display_name, "My App");
    }

    #[test]
    fn parses_version_and_build() {
        let r = parse(SAMPLE_PLIST);
        assert_eq!(r.version, "2.1.0");
        assert_eq!(r.build, "42");
    }

    #[test]
    fn parses_privacy_keys() {
        let r = parse(SAMPLE_PLIST);
        assert!(r
            .privacy_keys
            .contains(&"NSCameraUsageDescription".to_string()));
        assert!(r
            .privacy_keys
            .contains(&"NSMicrophoneUsageDescription".to_string()));
        assert_eq!(r.privacy_keys.len(), 3);
    }

    #[test]
    fn parses_capabilities() {
        let r = parse(SAMPLE_PLIST);
        assert!(r
            .capabilities
            .iter()
            .any(|c| c.contains("push-notifications")));
    }

    #[test]
    fn compact_output_contains_bundle_id() {
        let out = filter(SAMPLE_PLIST, Verbosity::Compact);
        assert!(out.content.contains("com.example.myapp"));
    }

    #[test]
    fn reduces_bytes_significantly() {
        let out = filter(SAMPLE_PLIST, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn very_verbose_passthrough() {
        let out = filter(SAMPLE_PLIST, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_PLIST);
    }
}
