use crate::filters::types::{TargetBuildSettings, XcodebuildSettingsResult};
use crate::filters::{FilterOutput, Verbosity};

/// Keys shown in Compact mode — high-signal iOS project identity.
const COMPACT_KEYS: &[&str] = &[
    "PRODUCT_NAME",
    "PRODUCT_BUNDLE_IDENTIFIER",
    "PRODUCT_MODULE_NAME",
    "SWIFT_VERSION",
    "IPHONEOS_DEPLOYMENT_TARGET",
    "MACOSX_DEPLOYMENT_TARGET",
    "CONFIGURATION",
    "SDKROOT",
    "PLATFORM_NAME",
    "ARCHS",
    "TARGETED_DEVICE_FAMILY",
    "CODE_SIGN_IDENTITY",
    "DEVELOPMENT_TEAM",
    "PROVISIONING_PROFILE_SPECIFIER",
    "INFOPLIST_FILE",
    "SWIFT_OPTIMIZATION_LEVEL",
];

/// Keys added in Verbose mode on top of Compact.
const VERBOSE_EXTRA_KEYS: &[&str] = &[
    "BUILT_PRODUCTS_DIR",
    "OBJROOT",
    "SYMROOT",
    "TARGET_NAME",
    "PROJECT_NAME",
    "WRAPPER_EXTENSION",
    "EXECUTABLE_NAME",
    "FULL_PRODUCT_NAME",
    "SWIFT_ACTIVE_COMPILATION_CONDITIONS",
    "GCC_PREPROCESSOR_DEFINITIONS",
    "OTHER_SWIFT_FLAGS",
    "FRAMEWORK_SEARCH_PATHS",
    "LIBRARY_SEARCH_PATHS",
];

pub fn parse(raw: &str) -> XcodebuildSettingsResult {
    let raw_targets = parse_raw_targets(raw);
    let targets = raw_targets
        .into_iter()
        .map(|t| TargetBuildSettings {
            name: t.name,
            settings: t.settings,
        })
        .collect();
    XcodebuildSettingsResult { targets }
}

/// Filter `xcodebuild -showBuildSettings` output.
///
/// Compact: ~16 high-signal iOS keys per target, grouped with a header.
/// Verbose: Compact + ~13 additional build-path and flag keys.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let allowed: Vec<&str> = if verbosity == Verbosity::Verbose {
        COMPACT_KEYS
            .iter()
            .chain(VERBOSE_EXTRA_KEYS.iter())
            .copied()
            .collect()
    } else {
        COMPACT_KEYS.to_vec()
    };

    let result = parse(raw);

    if result.targets.is_empty() {
        return FilterOutput::passthrough(raw);
    }

    let mut out = String::new();

    for target in &result.targets {
        // Header line: "Build settings — MyApp (Debug · iphonesimulator18.4)"
        let config = target
            .settings
            .get("CONFIGURATION")
            .map(String::as_str)
            .unwrap_or("—");
        let sdk = target
            .settings
            .get("SDKROOT")
            .map(String::as_str)
            .unwrap_or("—");
        out.push_str(&format!(
            "\x1b[1mBuild settings\x1b[0m — {} ({} · {})\n",
            target.name, config, sdk
        ));

        // Key-value lines, aligned
        for key in &allowed {
            if let Some(value) = target.settings.get(*key) {
                if !value.is_empty() {
                    out.push_str(&format!("  {key:<36}{value}\n"));
                }
            }
        }

        if result.targets.len() > 1 {
            out.push('\n');
        }
    }

    if verbosity == Verbosity::Compact {
        let total: usize = COMPACT_KEYS.len();
        out.push_str(&format!(
            "\n\x1b[2m({total} keys shown — use -v for more, --raw for all)\x1b[0m\n"
        ));
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
        structured: serde_json::to_value(&result).ok(),
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

struct RawTargetSettings {
    name: String,
    settings: std::collections::HashMap<String, String>,
}

/// Parse `xcodebuild -showBuildSettings` output into per-target maps.
///
/// The format is:
/// ```text
/// Build settings for action build and target MyApp:
///     KEY = value
///     KEY2 = value2
///
/// Build settings for action build and target MyAppTests:
///     ...
/// ```
fn parse_raw_targets(raw: &str) -> Vec<RawTargetSettings> {
    let mut targets: Vec<RawTargetSettings> = Vec::new();
    let mut current: Option<RawTargetSettings> = None;

    for line in raw.lines() {
        // Target header
        if line.starts_with("Build settings for action") && line.contains("target") {
            if let Some(prev) = current.take() {
                targets.push(prev);
            }
            let name = extract_target_name(line);
            current = Some(RawTargetSettings {
                name,
                settings: std::collections::HashMap::new(),
            });
            continue;
        }

        // KEY = value line (indented with spaces)
        if let Some(ref mut t) = current {
            let trimmed = line.trim();
            if let Some((key, value)) = trimmed.split_once(" = ") {
                t.settings
                    .insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }

    if let Some(last) = current.take() {
        targets.push(last);
    }

    targets
}

/// Extract target name from "Build settings for action build and target MyApp:"
fn extract_target_name(line: &str) -> String {
    if let Some(pos) = line.find("target ") {
        let after = &line[pos + 7..];
        return after.trim_end_matches(':').trim().to_string();
    }
    "Unknown".to_string()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Build settings for action build and target MyApp:
    ACTION = build
    AD_HOC_CODE_SIGNING_ALLOWED = NO
    ALTERNATE_GROUP = staff
    ALTERNATE_OWNER = runner
    ARCHS = arm64
    BUILT_PRODUCTS_DIR = /DerivedData/Build/Products/Debug-iphonesimulator
    CONFIGURATION = Debug
    CODE_SIGN_IDENTITY = iPhone Developer
    DEVELOPMENT_TEAM = ABCDEF1234
    INFOPLIST_FILE = MyApp/Info.plist
    IPHONEOS_DEPLOYMENT_TARGET = 18.0
    PLATFORM_NAME = iphonesimulator
    PRODUCT_BUNDLE_IDENTIFIER = com.example.myapp
    PRODUCT_MODULE_NAME = MyApp
    PRODUCT_NAME = MyApp
    SDKROOT = iphonesimulator18.4
    SWIFT_VERSION = 6.0
    TARGETED_DEVICE_FAMILY = 1,2
    SWIFT_OPTIMIZATION_LEVEL = -Onone
    PROVISIONING_PROFILE_SPECIFIER =

Build settings for action build and target MyAppTests:
    CONFIGURATION = Debug
    PRODUCT_NAME = MyAppTests
    PRODUCT_BUNDLE_IDENTIFIER = com.example.myapptests
    SWIFT_VERSION = 6.0
    SDKROOT = iphonesimulator18.4
    IPHONEOS_DEPLOYMENT_TARGET = 18.0
    DEVELOPMENT_TEAM = ABCDEF1234
";

    #[test]
    fn compact_shows_high_signal_keys() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.content.contains("PRODUCT_NAME"));
        assert!(out.content.contains("MyApp"));
        assert!(out.content.contains("SWIFT_VERSION"));
        assert!(out.content.contains("6.0"));
        assert!(out.content.contains("IPHONEOS_DEPLOYMENT_TARGET"));
        assert!(out.content.contains("18.0"));
    }

    #[test]
    fn compact_strips_noise_keys() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(!out.content.contains("AD_HOC_CODE_SIGNING_ALLOWED"));
        assert!(!out.content.contains("ALTERNATE_GROUP"));
        assert!(!out.content.contains("ALTERNATE_OWNER"));
    }

    #[test]
    fn compact_shows_target_header() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.content.contains("Build settings"));
        assert!(out.content.contains("MyApp"));
        assert!(out.content.contains("Debug"));
        assert!(out.content.contains("iphonesimulator18.4"));
    }

    #[test]
    fn compact_shows_both_targets() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.content.contains("MyApp"));
        assert!(out.content.contains("MyAppTests"));
    }

    #[test]
    fn verbose_adds_built_products_dir() {
        let out = filter(SAMPLE, Verbosity::Verbose);
        assert!(out.content.contains("BUILT_PRODUCTS_DIR"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE);
    }

    #[test]
    fn bytes_significantly_reduced() {
        // Build a realistic sample: many noise keys + a handful of compact keys.
        // Real xcodebuild -showBuildSettings outputs ~400 key-value pairs per target.
        let mut noisy = String::from("Build settings for action build and target MyApp:\n");
        // 50 pure-noise keys that the filter will drop
        let noise_keys = [
            "ACTION",
            "AD_HOC_CODE_SIGNING_ALLOWED",
            "ALTERNATE_GROUP",
            "ALTERNATE_OWNER",
            "ALWAYS_EMBED_SWIFT_STANDARD_LIBRARIES",
            "ALWAYS_SEARCH_USER_PATHS",
            "ALWAYS_USE_SEPARATE_HEADERMAPS",
            "APPLICATION_EXTENSION_API_ONLY",
            "APPLY_RULES_IN_COPY_FILES",
            "APPLY_RULES_IN_COPY_HEADERS",
            "ARCHS_STANDARD",
            "ARCHS_STANDARD_32_BIT",
            "ASSETCATALOG_COMPILER_APPICON_NAME",
            "ASSETCATALOG_COMPILER_GENERATE_SWIFT",
            "AVAILABLE_PLATFORMS",
            "BITCODE_GENERATION_MODE",
            "BUILD_ACTIVE_RESOURCES_ONLY",
            "BUILD_DIR",
            "BUILD_LIBRARY_FOR_DISTRIBUTION",
            "BUILD_ROOT",
            "BUILD_STYLE",
            "BUILD_VARIANTS",
            "CACHE_ROOT",
            "CHMOD",
            "CHOWN",
            "CLANG_ANALYZER_DEADCODE_DEADSTORES",
            "CLANG_ANALYZER_GCD_PERFORMANCE",
            "CLANG_ANALYZER_LOCALIZABILITY",
            "CLANG_CXX_LANGUAGE_STANDARD",
            "CLANG_CXX_LIBRARY",
            "CLANG_ENABLE_MODULES",
            "CLANG_ENABLE_OBJC_ARC",
            "CLANG_ENABLE_OBJC_WEAK",
            "CLANG_MODULES_AUTOLINK",
            "CLANG_MODULES_BUILD_SESSION_FILE",
            "CLANG_WARN_BLOCK_CAPTURE_AUTORELEASING",
            "CLANG_WARN_BOOL_CONVERSION",
            "CLANG_WARN_COMMA",
            "CLANG_WARN_DEPRECATED_OBJC_IMPLEMENTATIONS",
            "CLANG_WARN_DIRECT_OBJC_ISA_USAGE",
            "CLANG_WARN_DOCUMENTATION_COMMENTS",
            "CLANG_WARN_EMPTY_BODY",
            "CLANG_WARN_ENUM_CONVERSION",
            "CLANG_WARN_INFINITE_RECURSION",
            "CLANG_WARN_INT_CONVERSION",
            "CLANG_WARN_NON_LITERAL_NULL_CONVERSION",
            "CLANG_WARN_OBJC_IMPLICIT",
            "CLANG_WARN_OBJC_LITERAL_CONVERSION",
            "CLANG_WARN_OBJC_ROOT_CLASS",
            "CLANG_WARN_RANGE_LOOP_ANALYSIS",
        ];
        for key in &noise_keys {
            noisy.push_str(&format!("    {key} = some_noise_value\n"));
        }
        // Add the compact keys
        noisy.push_str("    PRODUCT_NAME = MyApp\n");
        noisy.push_str("    CONFIGURATION = Debug\n");
        noisy.push_str("    SDKROOT = iphonesimulator18.4\n");
        noisy.push_str("    SWIFT_VERSION = 6.0\n");
        noisy.push_str("    IPHONEOS_DEPLOYMENT_TARGET = 18.0\n");

        let out = filter(&noisy, Verbosity::Compact);
        // Filtered should be well under half the noisy input
        assert!(
            out.filtered_bytes < out.original_bytes / 2,
            "expected savings > 50%, got filtered={} original={}",
            out.filtered_bytes,
            out.original_bytes
        );
    }

    #[test]
    fn empty_value_keys_are_skipped() {
        let out = filter(SAMPLE, Verbosity::Compact);
        // PROVISIONING_PROFILE_SPECIFIER is empty → should not appear
        assert!(!out.content.contains("PROVISIONING_PROFILE_SPECIFIER"));
    }

    #[test]
    fn parse_returns_structured_data() {
        let result = parse(SAMPLE);
        assert_eq!(result.targets.len(), 2);
        assert_eq!(result.targets[0].name, "MyApp");
        assert_eq!(
            result.targets[0].settings.get("SWIFT_VERSION"),
            Some(&"6.0".to_string())
        );
    }

    #[test]
    fn structured_is_some_on_filter() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.structured.is_some());
    }
}
