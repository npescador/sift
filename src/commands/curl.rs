//! `curl` command family — no subcommands needed.

#[cfg(test)]
mod tests {
    use crate::commands::{detect, CommandFamily};

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn detects_curl() {
        assert_eq!(
            detect(&args(&["curl", "https://example.com"])),
            CommandFamily::Curl
        );
    }

    #[test]
    fn detects_curl_with_flags() {
        assert_eq!(
            detect(&args(&["curl", "-v", "https://api.example.com"])),
            CommandFamily::Curl
        );
    }
}
