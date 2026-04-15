//! Filter for `sift pbxproj` — parses Xcode project.pbxproj files.
//!
//! Extracts targets, bundle IDs, signing configuration, build phases,
//! and inter-target dependencies from the pbxproj property list format.

use crate::filters::types::{PbxprojResult, PbxprojTarget};
use crate::filters::{FilterOutput, Verbosity};

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

pub fn parse(raw: &str) -> PbxprojResult {
    let project_name = extract_project_name(raw);
    let configurations = extract_configurations(raw);
    let targets = extract_targets(raw);

    PbxprojResult {
        project_name,
        configurations,
        targets,
    }
}

fn format_result(result: &PbxprojResult, verbosity: Verbosity) -> String {
    let mut out = String::new();

    let name = if result.project_name.is_empty() {
        "Unknown"
    } else {
        &result.project_name
    };
    out.push_str(&format!("Project: {}\n", name));

    if !result.configurations.is_empty() {
        out.push_str(&format!(
            "Configurations: {}\n",
            result.configurations.join(", ")
        ));
    }

    out.push_str(&format!("\nTargets ({}):\n", result.targets.len()));

    for t in &result.targets {
        out.push_str(&format!("  {} [{}]\n", t.name, t.kind));
        if !t.bundle_id.is_empty() {
            out.push_str(&format!("    Bundle ID:  {}\n", t.bundle_id));
        }
        if !t.min_os.is_empty() {
            out.push_str(&format!("    Min iOS:    {}\n", t.min_os));
        }
        if !t.signing_team.is_empty() {
            out.push_str(&format!("    Team:       {}\n", t.signing_team));
        }
        if matches!(verbosity, Verbosity::Verbose) {
            if !t.build_phases.is_empty() {
                out.push_str(&format!("    Phases:     {}\n", t.build_phases.join(", ")));
            }
            if !t.dependencies.is_empty() {
                out.push_str(&format!("    Deps:       {}\n", t.dependencies.join(", ")));
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Parsers
// ---------------------------------------------------------------------------

/// Extract project name from the /* <Name> */ comment after archiveVersion or
/// from the first /* <Name>.xcodeproj */ comment in the file header.
fn extract_project_name(raw: &str) -> String {
    for line in raw.lines() {
        let t = line.trim();
        // Header line: // !$*UTF8*$!  or /* Begin ... section */
        if t.starts_with("// !$") {
            continue;
        }
        // Look for: rootObject = <UUID> /* <Name> */;
        if t.contains("rootObject") {
            if let Some(name) = extract_comment(t) {
                return name.trim_end_matches(" Project").to_string();
            }
        }
    }
    String::new()
}

fn extract_configurations(raw: &str) -> Vec<String> {
    let mut configs: Vec<String> = Vec::new();
    let mut in_build_config_list = false;

    for line in raw.lines() {
        let t = line.trim();
        if t.contains("XCConfigurationList") && t.contains("/* Build configuration list") {
            in_build_config_list = true;
        }
        if in_build_config_list {
            // buildConfigurations = ( ... );
            // Each entry: <UUID> /* Debug */,
            if t.ends_with("*/,") || t.ends_with("*/") {
                if let Some(name) = extract_comment(t) {
                    if !configs.contains(&name) {
                        configs.push(name);
                    }
                }
            }
            if t.starts_with(");") {
                break;
            }
        }
    }

    // Fallback: scan for XCBuildConfiguration blocks and grab their names
    if configs.is_empty() {
        let mut in_config_section = false;
        for line in raw.lines() {
            let t = line.trim();
            if t == "/* Begin XCBuildConfiguration section */" {
                in_config_section = true;
                continue;
            }
            if t == "/* End XCBuildConfiguration section */" {
                break;
            }
            if in_config_section && t.contains("name =") {
                if let Some(name) = extract_value(t, "name") {
                    if !configs.contains(&name) {
                        configs.push(name);
                    }
                }
            }
        }
    }

    configs
}

fn extract_targets(raw: &str) -> Vec<PbxprojTarget> {
    let mut targets: Vec<PbxprojTarget> = Vec::new();
    let mut in_target_section = false;
    let mut current: Option<PbxprojTarget> = None;
    let mut brace_depth = 0i32;
    let mut in_phases = false;
    let mut in_deps = false;

    for line in raw.lines() {
        let t = line.trim();

        if t == "/* Begin PBXNativeTarget section */" {
            in_target_section = true;
            continue;
        }
        if t == "/* End PBXNativeTarget section */" {
            if let Some(tgt) = current.take() {
                targets.push(tgt);
            }
            in_target_section = false;
            continue;
        }

        if !in_target_section {
            continue;
        }

        // New target block: <UUID> /* <name> */ = {
        if t.ends_with("= {") && brace_depth == 0 {
            if let Some(prev) = current.take() {
                targets.push(prev);
            }
            let mut tgt = PbxprojTarget::default();
            if let Some(name) = extract_comment(t) {
                tgt.name = name;
            }
            current = Some(tgt);
            brace_depth = 1;
            continue;
        }

        if let Some(ref mut tgt) = current {
            if t.contains('{') {
                brace_depth += t.chars().filter(|&c| c == '{').count() as i32;
            }
            if t.contains('}') {
                brace_depth -= t.chars().filter(|&c| c == '}').count() as i32;
            }

            if t == "buildPhases = (" {
                in_phases = true;
                in_deps = false;
                continue;
            }
            if t == "dependencies = (" {
                in_deps = true;
                in_phases = false;
                continue;
            }
            if t.starts_with(");") {
                in_phases = false;
                in_deps = false;
            }

            if in_phases {
                if let Some(phase_name) = extract_comment(t) {
                    tgt.build_phases.push(phase_name);
                }
            }
            if in_deps {
                if let Some(dep_name) = extract_comment(t) {
                    tgt.dependencies.push(dep_name);
                }
            }

            if t.contains("productType") {
                if let Some(pt) = extract_string_value(t) {
                    tgt.kind = product_type_label(&pt);
                }
            }
            if t.contains("PRODUCT_BUNDLE_IDENTIFIER") {
                if let Some(v) = extract_value(t, "PRODUCT_BUNDLE_IDENTIFIER") {
                    if tgt.bundle_id.is_empty() {
                        tgt.bundle_id = v;
                    }
                }
            }
            if t.contains("IPHONEOS_DEPLOYMENT_TARGET") {
                if let Some(v) = extract_value(t, "IPHONEOS_DEPLOYMENT_TARGET") {
                    if tgt.min_os.is_empty() {
                        tgt.min_os = v;
                    }
                }
            }
            if t.contains("DEVELOPMENT_TEAM") {
                if let Some(v) = extract_value(t, "DEVELOPMENT_TEAM") {
                    if tgt.signing_team.is_empty() {
                        tgt.signing_team = v;
                    }
                }
            }
        }
    }

    if let Some(tgt) = current {
        targets.push(tgt);
    }

    targets
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the comment text from `<UUID> /* Comment */ = ...` or trailing `/* Comment */`.
fn extract_comment(line: &str) -> Option<String> {
    let start = line.find("/* ")?;
    let end = line[start..].find(" */")?;
    Some(line[start + 3..start + end].to_string())
}

/// Extract `key = value;` where value is unquoted.
fn extract_value(line: &str, key: &str) -> Option<String> {
    let pattern = format!("{} = ", key);
    let pos = line.find(&pattern)?;
    let rest = line[pos + pattern.len()..].trim_end_matches(';').trim();
    Some(rest.trim_matches('"').to_string())
}

/// Extract `key = "value";` string value.
fn extract_string_value(line: &str) -> Option<String> {
    let eq = line.find("= ")?;
    let rest = line[eq + 2..].trim().trim_end_matches(';').trim();
    Some(rest.trim_matches('"').to_string())
}

fn product_type_label(pt: &str) -> String {
    match pt {
        "com.apple.product-type.application" => "App",
        "com.apple.product-type.app-extension" => "Extension",
        "com.apple.product-type.bundle.unit-test" => "Unit Tests",
        "com.apple.product-type.bundle.ui-testing" => "UI Tests",
        "com.apple.product-type.framework" => "Framework",
        "com.apple.product-type.library.static" => "Static Lib",
        "com.apple.product-type.library.dynamic" => "Dynamic Lib",
        "com.apple.product-type.bundle" => "Bundle",
        "com.apple.product-type.tool" => "CLI Tool",
        _ => pt.split('.').next_back().unwrap_or(pt),
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"// !$*UTF8*$!
{
	archiveVersion = 1;
	classes = {
	};
	objectVersion = 56;
	objects = {

/* Begin PBXNativeTarget section */
		AAAAAAAAAAAAAAAAAAAAAAAA /* MyApp */ = {
			isa = PBXNativeTarget;
			buildConfigurationList = BBBBBBBBBBBBBBBBBBBBBBBB /* Build configuration list for PBXNativeTarget "MyApp" */;
			buildPhases = (
				CCCCCCCCCCCCCCCCCCCCCCCC /* Sources */,
				DDDDDDDDDDDDDDDDDDDDDDDD /* Frameworks */,
				EEEEEEEEEEEEEEEEEEEEEEEE /* Resources */,
			);
			dependencies = (
			);
			name = MyApp;
			productName = MyApp;
			productType = "com.apple.product-type.application";
		};
		FFFFFFFFFFFFFFFFFFFFFFFFFF /* MyAppTests */ = {
			isa = PBXNativeTarget;
			buildConfigurationList = GGGGGGGGGGGGGGGGGGGGGGGG /* Build configuration list for PBXNativeTarget "MyAppTests" */;
			buildPhases = (
				HHHHHHHHHHHHHHHHHHHHHHHH /* Sources */,
			);
			dependencies = (
				IIIIIIIIIIIIIIIIIIIIIIII /* PBXTargetDependency */,
			);
			name = MyAppTests;
			productType = "com.apple.product-type.bundle.unit-test";
		};
/* End PBXNativeTarget section */

/* Begin XCBuildConfiguration section */
		JJJJJJJJJJJJJJJJJJJJJJJJ /* Debug */ = {
			isa = XCBuildConfiguration;
			buildSettings = {
				IPHONEOS_DEPLOYMENT_TARGET = 17.0;
				PRODUCT_BUNDLE_IDENTIFIER = com.example.myapp;
				DEVELOPMENT_TEAM = TEAM1234AB;
			};
			name = Debug;
		};
		KKKKKKKKKKKKKKKKKKKKKKKKKK /* Release */ = {
			isa = XCBuildConfiguration;
			name = Release;
		};
/* End XCBuildConfiguration section */

	};
	rootObject = LLLLLLLLLLLLLLLLLLLLLLLL /* Project object */;
}
"#;

    #[test]
    fn parses_target_names() {
        let result = parse(SAMPLE);
        let names: Vec<&str> = result.targets.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"MyApp"));
        assert!(names.contains(&"MyAppTests"));
    }

    #[test]
    fn parses_product_types() {
        let result = parse(SAMPLE);
        let app = result.targets.iter().find(|t| t.name == "MyApp").unwrap();
        assert_eq!(app.kind, "App");
        let tests = result
            .targets
            .iter()
            .find(|t| t.name == "MyAppTests")
            .unwrap();
        assert_eq!(tests.kind, "Unit Tests");
    }

    #[test]
    fn parses_configurations() {
        let result = parse(SAMPLE);
        assert!(result.configurations.contains(&"Debug".to_string()));
        assert!(result.configurations.contains(&"Release".to_string()));
    }

    #[test]
    fn compact_output_contains_targets() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.content.contains("MyApp"));
        assert!(out.content.contains("App"));
    }

    #[test]
    fn reduces_bytes_significantly() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn very_verbose_passthrough() {
        let out = filter(SAMPLE, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE);
    }

    #[test]
    fn structured_is_some() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.structured.is_some());
    }

    #[test]
    fn verbose_shows_build_phases() {
        let out = filter(SAMPLE, Verbosity::Verbose);
        assert!(out.content.contains("Sources"));
    }
}
