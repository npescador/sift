use crate::filters::types::{CurlHeader, CurlResult};
use crate::filters::{FilterOutput, Verbosity};

pub fn parse(raw: &str) -> CurlResult {
    if is_curl_error(raw) {
        return CurlResult {
            status_line: None,
            status_code: None,
            headers: Vec::new(),
            body_lines: 0,
            is_error: true,
        };
    }

    let mut in_headers = false;
    let mut in_body = false;
    let mut body_count = 0usize;
    let mut status_line: Option<String> = None;
    let mut status_code: Option<u16> = None;
    let mut headers: Vec<CurlHeader> = Vec::new();

    for line in raw.lines() {
        if is_progress_line(line) {
            continue;
        }

        if !in_body && (line.starts_with("HTTP/1") || line.starts_with("HTTP/2")) {
            in_headers = true;
            let raw_status = line.to_string();
            status_code = raw_status
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok());
            status_line = Some(format_status_line(line));
            continue;
        }

        if in_headers && line.trim().is_empty() {
            in_headers = false;
            in_body = true;
            continue;
        }

        if in_headers {
            if let Some((name, value)) = line.split_once(':') {
                headers.push(CurlHeader {
                    name: name.trim().to_lowercase(),
                    value: value.trim().to_string(),
                });
            }
            continue;
        }

        if in_body || (!in_headers && status_line.is_none()) {
            body_count += 1;
        }
    }

    CurlResult {
        status_line,
        status_code,
        headers,
        body_lines: body_count,
        is_error: false,
    }
}

/// Filter `curl` output.
///
/// Compact:
/// - Show HTTP status line
/// - Show key headers: content-type, content-length, location, x-request-id
/// - Truncate body to 20 lines
/// - Strip progress meter lines
///
/// Verbose: 40 lines of body + all response headers.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    if is_curl_error(raw) {
        let result = parse(raw);
        return FilterOutput {
            content: raw.to_string(),
            original_bytes,
            filtered_bytes: raw.len(),
            structured: serde_json::to_value(&result).ok(),
        };
    }

    let body_limit = if verbosity == Verbosity::Verbose {
        40
    } else {
        20
    };

    let mut out = String::new();
    let mut in_headers = false;
    let mut in_body = false;
    let mut body_lines: Vec<&str> = Vec::new();
    let mut status_line: Option<String> = None;
    let mut response_headers: Vec<(String, String)> = Vec::new();

    for line in raw.lines() {
        if is_progress_line(line) {
            continue;
        }

        if !in_body && (line.starts_with("HTTP/1") || line.starts_with("HTTP/2")) {
            in_headers = true;
            status_line = Some(format_status_line(line));
            continue;
        }

        if in_headers && line.trim().is_empty() {
            in_headers = false;
            in_body = true;
            continue;
        }

        if in_headers {
            if let Some((name, value)) = line.split_once(':') {
                response_headers.push((name.trim().to_lowercase(), value.trim().to_string()));
            }
            continue;
        }

        if in_body || (!in_headers && status_line.is_none()) {
            body_lines.push(line);
        }
    }

    let result = parse(raw);

    if status_line.is_none() && !body_lines.is_empty() {
        let truncated = body_lines.len() > body_limit;
        let shown = &body_lines[..body_lines.len().min(body_limit)];
        for l in shown {
            out.push_str(l);
            out.push('\n');
        }
        if truncated {
            out.push_str(&format!(
                "… ({} more lines)\n",
                body_lines.len() - body_limit
            ));
        }
        let filtered_bytes = out.len();
        return FilterOutput {
            content: out,
            original_bytes,
            filtered_bytes,
            structured: serde_json::to_value(&result).ok(),
        };
    }

    if let Some(ref s) = status_line {
        out.push_str(s);
        out.push('\n');
    }

    let key_headers = ["content-type", "content-length", "location", "x-request-id"];
    let headers_to_show: Vec<&(String, String)> = if verbosity == Verbosity::Verbose {
        response_headers.iter().collect()
    } else {
        response_headers
            .iter()
            .filter(|(k, _)| key_headers.contains(&k.as_str()))
            .collect()
    };

    if !headers_to_show.is_empty() {
        for (name, value) in &headers_to_show {
            out.push_str(&format!("{name}: {value}\n"));
        }
    }

    if !body_lines.is_empty() {
        out.push('\n');
        let total = body_lines.len();
        let shown_count = total.min(body_limit);
        for l in &body_lines[..shown_count] {
            out.push_str(l);
            out.push('\n');
        }
        if total > body_limit {
            out.push_str(&format!("… ({} more lines)\n", total - body_limit));
        }
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
        structured: serde_json::to_value(&result).ok(),
    }
}

fn is_curl_error(raw: &str) -> bool {
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .all(|l| l.starts_with("curl: ("))
}

fn is_progress_line(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('%')
        || t.contains("Dload")
        || t.contains("--:--:--")
        || (t
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
            && t.contains("--:--:--"))
}

fn format_status_line(line: &str) -> String {
    let status_code = line.split_whitespace().nth(1).unwrap_or("0");
    let code: u16 = status_code.parse().unwrap_or(0);
    let indicator = if (200..300).contains(&code) {
        "\x1b[32m✓\x1b[0m"
    } else {
        "\x1b[31m✗\x1b[0m"
    };
    format!("{line} {indicator}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_WITH_HEADERS: &str = "\
  % Total    % Received % Xferd  Average Speed   Time    Time     Time  Current
                                 Dload  Upload   Total   Spent    Left  Speed
100  1256  100  1256    0     0   8234      0 --:--:-- --:--:-- --:--:--  8311
HTTP/2 200
content-type: application/json; charset=utf-8
content-length: 1256
x-request-id: abc123def456
cache-control: no-cache

{\"id\":1,\"name\":\"John\"}
";

    const SAMPLE_ERROR: &str = "\
curl: (6) Could not resolve host: api.example.com
";

    const SAMPLE_404: &str = "\
HTTP/1.1 404
content-type: text/plain
content-length: 9

Not Found
";

    const SAMPLE_BODY_ONLY: &str = "{\"id\":1,\"name\":\"John\",\"email\":\"test@example.com\"}\n";

    #[test]
    fn compact_shows_status_line() {
        let out = filter(SAMPLE_WITH_HEADERS, Verbosity::Compact);
        assert!(out.content.contains("HTTP/2 200"));
        assert!(out.content.contains('✓'));
    }

    #[test]
    fn compact_shows_key_headers() {
        let out = filter(SAMPLE_WITH_HEADERS, Verbosity::Compact);
        assert!(out.content.contains("content-type"));
        assert!(out.content.contains("x-request-id"));
    }

    #[test]
    fn compact_strips_progress_meter() {
        let out = filter(SAMPLE_WITH_HEADERS, Verbosity::Compact);
        assert!(!out.content.contains("Dload"));
        assert!(!out.content.contains("--:--:--"));
    }

    #[test]
    fn compact_shows_404_with_error_indicator() {
        let out = filter(SAMPLE_404, Verbosity::Compact);
        assert!(out.content.contains("404"));
        assert!(out.content.contains('✗'));
    }

    #[test]
    fn curl_error_shown_as_is() {
        let out = filter(SAMPLE_ERROR, Verbosity::Compact);
        assert!(out.content.contains("Could not resolve host"));
    }

    #[test]
    fn body_only_response_shown() {
        let out = filter(SAMPLE_BODY_ONLY, Verbosity::Compact);
        assert!(out.content.contains("John"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_WITH_HEADERS, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_WITH_HEADERS);
    }

    #[test]
    fn body_truncated_at_20_lines_compact() {
        let mut raw = "HTTP/2 200\ncontent-type: text/plain\n\n".to_string();
        for i in 0..25 {
            raw.push_str(&format!("line {i}\n"));
        }
        let out = filter(&raw, Verbosity::Compact);
        assert!(out.content.contains("… (5 more lines)"));
    }

    #[test]
    fn verbose_shows_all_headers() {
        let out = filter(SAMPLE_WITH_HEADERS, Verbosity::Verbose);
        assert!(out.content.contains("cache-control"));
    }

    #[test]
    fn bytes_reduced_vs_original() {
        let out = filter(SAMPLE_WITH_HEADERS, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn parse_extracts_status_code() {
        let result = parse(SAMPLE_WITH_HEADERS);
        assert_eq!(result.status_code, Some(200));
        assert!(!result.is_error);
    }

    #[test]
    fn parse_curl_error_sets_flag() {
        let result = parse(SAMPLE_ERROR);
        assert!(result.is_error);
    }

    #[test]
    fn structured_is_some_on_filter() {
        let out = filter(SAMPLE_WITH_HEADERS, Verbosity::Compact);
        assert!(out.structured.is_some());
    }
}
