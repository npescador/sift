//! Filter for `sift provisioning` — parses Apple .mobileprovision files.
//!
//! A .mobileprovision file is a DER-encoded CMS blob containing an embedded
//! XML plist. We extract the plist section using byte scanning (no external
//! crypto dependency) and parse the relevant fields.

use crate::filters::types::ProvisioningResult;
use crate::filters::{FilterOutput, Verbosity};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    // raw here is the plist XML extracted from the mobileprovision binary.
    // The caller (main.rs) is responsible for reading the file and extracting
    // the embedded plist. For stdin/text usage we accept raw plist XML directly.
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

pub fn parse(plist_xml: &str) -> ProvisioningResult {
    let mut r = ProvisioningResult::default();

    let pairs = collect_plist_pairs(plist_xml);

    for (key, value) in &pairs {
        match key.as_str() {
            "Name" => r.name = value.clone(),
            "TeamName" => r.team_name = value.clone(),
            "TeamIdentifier" => {
                if r.team_id.is_empty() {
                    r.team_id = value.trim_matches('[').trim_matches(']').to_string();
                }
            }
            "AppIDName" => {} // skip, app ID name often redundant
            "application-identifier" | "com.apple.application-identifier" => {
                if r.app_id.is_empty() {
                    r.app_id = value.clone();
                }
            }
            "ExpirationDate" => {
                r.expiry = value.clone();
                r.expiry_status = expiry_status(value);
            }
            "ProvisionedDevices" => {
                // value from our parser is "[array]" — count items differently
                r.device_count = count_array_items(plist_xml, "ProvisionedDevices");
            }
            "DeveloperCertificates" => {
                r.certificate_count = count_array_items(plist_xml, "DeveloperCertificates");
            }
            k if k.starts_with("com.apple.")
                || k == "aps-environment"
                || k == "beta-reports-active" =>
            {
                r.entitlements.push(k.to_string());
            }
            _ => {}
        }
    }

    // Determine profile type
    r.profile_type = determine_profile_type(plist_xml, &r);

    r
}

fn format_result(r: &ProvisioningResult, verbosity: Verbosity) -> String {
    let mut out = String::new();

    if !r.name.is_empty() {
        out.push_str(&format!("Profile:     {}\n", r.name));
    }
    out.push_str(&format!("Type:        {}\n", r.profile_type));
    if !r.app_id.is_empty() {
        out.push_str(&format!("App ID:      {}\n", r.app_id));
    }
    if !r.team_name.is_empty() {
        let team_id = if r.team_id.is_empty() {
            String::new()
        } else {
            format!(" ({})", r.team_id)
        };
        out.push_str(&format!("Team:        {}{}\n", r.team_name, team_id));
    }
    if !r.expiry.is_empty() {
        out.push_str(&format!(
            "Expires:     {} [{}]\n",
            r.expiry, r.expiry_status
        ));
    }
    if r.device_count > 0 {
        out.push_str(&format!("Devices:     {}\n", r.device_count));
    }
    if r.certificate_count > 0 {
        out.push_str(&format!("Certs:       {}\n", r.certificate_count));
    }
    if !r.entitlements.is_empty() {
        out.push_str(&format!("\nEntitlements ({}):\n", r.entitlements.len()));
        if matches!(verbosity, Verbosity::Compact) {
            // show first 5
            for e in r.entitlements.iter().take(5) {
                out.push_str(&format!("  {}\n", e));
            }
            if r.entitlements.len() > 5 {
                out.push_str(&format!("  (+{} more)\n", r.entitlements.len() - 5));
            }
        } else {
            for e in &r.entitlements {
                out.push_str(&format!("  {}\n", e));
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn collect_plist_pairs(raw: &str) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut pending_key: Option<String> = None;

    for line in raw.lines() {
        let t = line.trim();

        if t.starts_with("<key>") && t.ends_with("</key>") {
            pending_key = Some(
                t.trim_start_matches("<key>")
                    .trim_end_matches("</key>")
                    .to_string(),
            );
            continue;
        }

        if let Some(key) = pending_key.take() {
            let value = extract_xml_value(t);
            pairs.push((key, value));
        }
    }

    pairs
}

fn extract_xml_value(t: &str) -> String {
    for tag in &["string", "integer", "real"] {
        let open = format!("<{}>", tag);
        let close = format!("</{}>", tag);
        if t.starts_with(&open) {
            return t
                .trim_start_matches(&*open)
                .trim_end_matches(&*close)
                .to_string();
        }
    }
    if t == "<true/>" || t == "<true>" {
        return "true".to_string();
    }
    if t == "<false/>" || t == "<false>" {
        return "false".to_string();
    }
    if t.starts_with("<array") {
        return "[array]".to_string();
    }
    if t.starts_with("<dict") {
        return "[dict]".to_string();
    }
    t.to_string()
}

fn count_array_items(raw: &str, key: &str) -> usize {
    let needle = format!("<key>{}</key>", key);
    let mut found = false;
    let mut in_array = false;
    let mut count = 0usize;

    for line in raw.lines() {
        let t = line.trim();
        if !found {
            if t == needle {
                found = true;
            }
            continue;
        }
        if !in_array {
            if t.starts_with("<array") {
                in_array = true;
            }
            continue;
        }
        if t == "</array>" {
            break;
        }
        // Each item in the array starts with a tag
        if t.starts_with('<') && !t.starts_with("</") && !t.starts_with("<!--") {
            count += 1;
        }
    }

    count
}

fn determine_profile_type(raw: &str, r: &ProvisioningResult) -> String {
    let has_provisioned_devices = r.device_count > 0;
    let has_aps_production =
        raw.contains("aps-environment</key>") && raw.contains("<string>production</string>");
    let has_aps_development =
        raw.contains("aps-environment</key>") && raw.contains("<string>development</string>");
    let has_xc_wildcard = r.app_id.contains('*');

    // Heuristic detection
    if raw.contains("ProvisionsAllDevices") && raw.contains("<true/>") {
        return "Enterprise (In-House)".to_string();
    }
    if has_aps_production && !has_provisioned_devices {
        return "App Store".to_string();
    }
    if has_provisioned_devices && has_aps_production {
        return "Ad Hoc".to_string();
    }
    if has_provisioned_devices && (has_aps_development || !has_aps_production) {
        return "Development".to_string();
    }
    if has_xc_wildcard {
        return "Development (wildcard)".to_string();
    }

    "Unknown".to_string()
}

/// Parse ISO 8601 date string and return expiry status.
fn expiry_status(date_str: &str) -> String {
    // Format: 2025-12-31T23:59:59Z
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if let Some(secs) = parse_iso8601(date_str) {
        if secs < now_secs {
            return "EXPIRED".to_string();
        }
        // 30 days = 2592000 seconds
        if secs < now_secs + 2_592_000 {
            return "expiring soon".to_string();
        }
        return "valid".to_string();
    }

    "unknown".to_string()
}

fn parse_iso8601(s: &str) -> Option<u64> {
    // Very minimal: parse YYYY-MM-DDTHH:MM:SSZ
    let parts: Vec<&str> = s.splitn(2, 'T').collect();
    if parts.len() < 2 {
        return None;
    }
    let date_parts: Vec<u32> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
    if date_parts.len() < 3 {
        return None;
    }
    let (y, m, d) = (date_parts[0], date_parts[1], date_parts[2]);
    // Rough days since epoch
    let years_since_epoch = y.saturating_sub(1970);
    let leap_years = years_since_epoch / 4;
    let days =
        years_since_epoch * 365 + leap_years + months_to_days(m, is_leap(y)) + d.saturating_sub(1);
    Some(days as u64 * 86400)
}

fn is_leap(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn months_to_days(month: u32, leap: bool) -> u32 {
    let days = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut total = 0u32;
    for m in 1..month {
        total += days[m as usize];
        if m == 2 && leap {
            total += 1;
        }
    }
    total
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
	<key>Name</key>
	<string>MyApp Development</string>
	<key>TeamName</key>
	<string>Acme Corp</string>
	<key>TeamIdentifier</key>
	<array>
		<string>TEAM1234AB</string>
	</array>
	<key>application-identifier</key>
	<string>TEAM1234AB.com.acme.myapp</string>
	<key>ExpirationDate</key>
	<string>2030-12-31T23:59:59Z</string>
	<key>ProvisionedDevices</key>
	<array>
		<string>abc123def456</string>
		<string>xyz789uvw012</string>
	</array>
	<key>DeveloperCertificates</key>
	<array>
		<data>MIIFkzCCBHugAwIBAgIIW</data>
	</array>
	<key>Entitlements</key>
	<dict>
		<key>aps-environment</key>
		<string>development</string>
		<key>com.apple.developer.associated-domains</key>
		<array/>
		<key>application-identifier</key>
		<string>TEAM1234AB.com.acme.myapp</string>
	</dict>
</dict>
</plist>
"#;

    #[test]
    fn parses_profile_name() {
        let r = parse(SAMPLE_PLIST);
        assert_eq!(r.name, "MyApp Development");
    }

    #[test]
    fn parses_team() {
        let r = parse(SAMPLE_PLIST);
        assert_eq!(r.team_name, "Acme Corp");
    }

    #[test]
    fn parses_app_id() {
        let r = parse(SAMPLE_PLIST);
        assert!(r.app_id.contains("com.acme.myapp"));
    }

    #[test]
    fn parses_expiry_valid() {
        let r = parse(SAMPLE_PLIST);
        assert_eq!(r.expiry_status, "valid");
    }

    #[test]
    fn parses_device_count() {
        let r = parse(SAMPLE_PLIST);
        assert_eq!(r.device_count, 2);
    }

    #[test]
    fn compact_output_contains_profile_name() {
        let out = filter(SAMPLE_PLIST, Verbosity::Compact);
        assert!(out.content.contains("MyApp Development"));
    }

    #[test]
    fn reduces_bytes() {
        let out = filter(SAMPLE_PLIST, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn very_verbose_passthrough() {
        let out = filter(SAMPLE_PLIST, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_PLIST);
    }
}
