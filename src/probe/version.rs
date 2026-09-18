//! Extracts a normalized version string from raw `--version`-style output.

use regex::Regex;

const DEFAULT_PATTERN: &str = r"(\d+\.\d+(?:\.\d+)?)";

/// Applies `pattern` (or the generic `\d+\.\d+(\.\d+)?` default) to `output`
/// and returns the first capture group. Unparseable output yields `None`
/// rather than a wrong guess.
pub fn extract(output: &str, pattern: Option<&str>) -> Option<String> {
    let pattern = pattern.unwrap_or(DEFAULT_PATTERN);
    let re = Regex::new(pattern).ok()?;
    re.captures(output)?.get(1).map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_semver_from_typical_output() {
        assert_eq!(
            extract("zsh 5.9 (x86_64-ubuntu-linux-gnu)", None),
            Some("5.9".to_string())
        );
        assert_eq!(
            extract("git version 2.43.0", None),
            Some("2.43.0".to_string())
        );
        assert_eq!(extract("node v20.11.0", None), Some("20.11.0".to_string()));
    }

    #[test]
    fn returns_none_for_unparseable_output() {
        assert_eq!(extract("no version info here", None), None);
    }
}
