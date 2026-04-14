use std::collections::BTreeMap;

use crate::filters::types::{PeripheryFileGroup, PeripheryResult, PeripherySymbol};
use crate::filters::{FilterOutput, Verbosity};

/// Filter `periphery scan` output.
///
/// Compact: unused symbols grouped by file, symbol kind + name + line.
///          Summary: "N unused symbols in M files".
/// Verbose: same with full file paths.
/// VeryVerbose+: raw passthrough.
///
/// Periphery line format:
/// `/path/to/File.swift:45:18: warning: Function 'validateLegacyCard()' is unused`
/// `/path/to/File.swift:23:1: warning: Class 'LegacyManager' is unused`
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let result = parse(raw);
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

/// Parse raw `periphery scan` output into grouped result.
pub fn parse(raw: &str) -> PeripheryResult {
    // Map from file path → list of symbols
    let mut file_map: BTreeMap<String, Vec<PeripherySymbol>> = BTreeMap::new();

    for line in raw.lines() {
        if !line.contains(": warning:") && !line.contains(": note:") {
            continue;
        }

        let marker = if line.contains(": warning:") {
            ": warning:"
        } else {
            ": note:"
        };

        let (location_part, rest) = match line.split_once(marker) {
            Some(pair) => pair,
            None => continue,
        };

        if !rest.to_lowercase().contains("is unused") && !rest.to_lowercase().contains("unused") {
            continue;
        }

        // location_part: "/path/file.swift:LINE:COL"
        let mut loc_parts = location_part.rsplitn(3, ':');
        let _col = loc_parts.next().unwrap_or("0");
        let line_num: u32 = loc_parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let file_path = loc_parts.next().unwrap_or("").to_string();

        if file_path.is_empty() {
            continue;
        }

        let (kind, name) = extract_kind_and_name(rest.trim());

        file_map
            .entry(file_path)
            .or_default()
            .push(PeripherySymbol {
                kind,
                name,
                line: line_num,
            });
    }

    let total_symbols: usize = file_map.values().map(|v| v.len()).sum();
    let total_files = file_map.len();

    let files = file_map
        .into_iter()
        .map(|(path, symbols)| PeripheryFileGroup { path, symbols })
        .collect();

    PeripheryResult {
        files,
        total_symbols,
        total_files,
    }
}

/// Extract symbol kind and name from the message part.
///
/// Examples:
///   "Function 'validateLegacyCard()' is unused" → ("func", "validateLegacyCard()")
///   "Class 'LegacyManager' is unused"            → ("class", "LegacyManager")
///   "Variable 'debugMode' is unused"             → ("var", "debugMode")
fn extract_kind_and_name(msg: &str) -> (String, String) {
    let kind_map = [
        ("Function", "func"),
        ("Method", "func"),
        ("Class", "class"),
        ("Struct", "struct"),
        ("Enum", "enum"),
        ("Protocol", "protocol"),
        ("Actor", "actor"),
        ("Variable", "var"),
        ("Property", "var"),
        ("Initializer", "init"),
        ("TypeAlias", "typealias"),
        ("Extension", "extension"),
        ("Import", "import"),
        ("Parameter", "param"),
    ];

    let kind = kind_map
        .iter()
        .find(|(k, _)| msg.starts_with(k))
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| "symbol".to_string());

    // Extract name between single quotes
    let name = msg.split('\'').nth(1).unwrap_or("").to_string();

    (kind, name)
}

fn render(result: &PeripheryResult, verbosity: Verbosity) -> String {
    if result.total_symbols == 0 {
        return "No unused symbols found.\n".to_string();
    }

    let mut out = String::new();

    let show_full_path = matches!(verbosity, Verbosity::Verbose);
    const COMPACT_FILE_LIMIT: usize = 10;

    let files_to_show = if matches!(verbosity, Verbosity::Compact) {
        result
            .files
            .iter()
            .take(COMPACT_FILE_LIMIT)
            .collect::<Vec<_>>()
    } else {
        result.files.iter().collect::<Vec<_>>()
    };

    for group in &files_to_show {
        let display_path = if show_full_path {
            group.path.clone()
        } else {
            shorten_path(&group.path)
        };

        out.push_str(&format!("{} ({})\n", display_path, group.symbols.len()));
        for sym in &group.symbols {
            out.push_str(&format!(
                "  {}  {}  line {}\n",
                sym.kind, sym.name, sym.line
            ));
        }
    }

    let hidden_files = result.total_files.saturating_sub(files_to_show.len());
    if hidden_files > 0 {
        let hidden_symbols: usize = result
            .files
            .iter()
            .skip(files_to_show.len())
            .map(|g| g.symbols.len())
            .sum();
        out.push_str(&format!(
            "[+{} symbols in {} more files — use -v for full list]\n",
            hidden_symbols, hidden_files
        ));
    }

    out.push('\n');
    out.push_str(&format!(
        "{} unused symbol{} in {} file{}\n",
        result.total_symbols,
        if result.total_symbols == 1 { "" } else { "s" },
        result.total_files,
        if result.total_files == 1 { "" } else { "s" },
    ));

    out
}

fn shorten_path(path: &str) -> String {
    // Show last 2 path components: "Features/Auth/AuthViewModel.swift"
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() > 2 {
        parts[parts.len() - 3..].join("/")
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"/Users/dev/MyApp/Features/Checkout/CheckoutViewModel.swift:45:18: warning: Function 'validateLegacyCard()' is unused
/Users/dev/MyApp/Features/Checkout/CheckoutViewModel.swift:89:5: warning: Variable 'debugMode' is unused
/Users/dev/MyApp/Features/Auth/AuthViewModel.swift:23:1: warning: Class 'LegacyAuthManager' is unused
/Users/dev/MyApp/Features/Auth/AuthViewModel.swift:67:5: warning: Function 'legacyLogin(user:password:)' is unused
/Users/dev/MyApp/Core/Network/NetworkClient.swift:12:1: warning: Protocol 'LegacyNetworkProtocol' is unused
"#;

    #[test]
    fn parses_symbols_grouped_by_file() {
        let result = parse(SAMPLE);
        assert_eq!(result.total_symbols, 5);
        assert_eq!(result.total_files, 3);
    }

    #[test]
    fn checkout_file_has_two_symbols() {
        let result = parse(SAMPLE);
        let checkout = result
            .files
            .iter()
            .find(|g| g.path.contains("CheckoutViewModel"))
            .expect("CheckoutViewModel group missing");
        assert_eq!(checkout.symbols.len(), 2);
    }

    #[test]
    fn extracts_kind_and_name_correctly() {
        let (kind, name) = extract_kind_and_name("Function 'validateLegacyCard()' is unused");
        assert_eq!(kind, "func");
        assert_eq!(name, "validateLegacyCard()");

        let (kind, name) = extract_kind_and_name("Class 'LegacyAuthManager' is unused");
        assert_eq!(kind, "class");
        assert_eq!(name, "LegacyAuthManager");

        let (kind, name) = extract_kind_and_name("Protocol 'LegacyNetworkProtocol' is unused");
        assert_eq!(kind, "protocol");
        assert_eq!(name, "LegacyNetworkProtocol");
    }

    #[test]
    fn empty_input_returns_no_symbols_message() {
        let output = filter("", Verbosity::Compact);
        assert!(output.content.contains("No unused symbols"));
    }

    #[test]
    fn compact_output_contains_summary_line() {
        let output = filter(SAMPLE, Verbosity::Compact);
        assert!(output.content.contains("5 unused symbols in 3 files"));
    }

    #[test]
    fn compact_output_shows_symbol_kind_and_name() {
        let output = filter(SAMPLE, Verbosity::Compact);
        assert!(output.content.contains("func"));
        assert!(output.content.contains("validateLegacyCard()"));
    }

    #[test]
    fn reduces_bytes() {
        let output = filter(SAMPLE, Verbosity::Compact);
        assert!(output.filtered_bytes < output.original_bytes);
    }
}
