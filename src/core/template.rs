//! `${VAR}` substitution for configs carrying machine-specific values.
//!
//! Deliberately not a template engine: the need is a git identity and the odd
//! host path, and an unresolved variable must be a loud failure rather than an
//! empty string silently written into someone's `.gitconfig`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use color_eyre::eyre::{Context, Result, bail};

/// Values available to templates: the values file first, then the
/// environment as a fallback.
#[derive(Debug, Clone, Default)]
pub struct Values {
    map: BTreeMap<String, String>,
}

impl Values {
    pub fn new(map: BTreeMap<String, String>) -> Self {
        Self { map }
    }

    /// Loads `~/.config/pinst/values.toml` (or `$PINST_VALUES`). A missing
    /// file is fine — the environment may still supply everything.
    pub fn load() -> Result<Self> {
        let Some(path) = values_path() else {
            return Ok(Self::default());
        };
        if !path.is_file() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let parsed: BTreeMap<String, toml::Value> =
            toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;

        let map = parsed
            .into_iter()
            .map(|(k, v)| match v {
                toml::Value::String(s) => (k, s),
                other => (k, other.to_string()),
            })
            .collect();
        Ok(Self::new(map))
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.map
            .get(key)
            .cloned()
            .or_else(|| std::env::var(key).ok())
    }

    pub fn path_hint() -> String {
        values_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "~/.config/pinst/values.toml".to_string())
    }
}

fn values_path() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("PINST_VALUES") {
        return Some(PathBuf::from(explicit));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/pinst/values.toml"))
}

/// Substitutes every `${VAR}`. Fails listing all missing variables at once,
/// so an operator fixes their values file in one pass.
pub fn render(input: &str, values: &Values) -> Result<String> {
    let mut out = String::with_capacity(input.len());
    let mut missing: Vec<String> = Vec::new();
    let mut rest = input;

    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            // An unterminated `${` is literal text, not a broken template.
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let name = &after[..end];
        match values.get(name) {
            Some(value) => out.push_str(&value),
            None => {
                if !missing.contains(&name.to_string()) {
                    missing.push(name.to_string());
                }
                // Keep the placeholder so a partial render is still readable
                // in an error message.
                out.push_str(&format!("${{{name}}}"));
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);

    if !missing.is_empty() {
        bail!(
            "unresolved template variable(s): {}. Set them in {} or in the environment",
            missing.join(", "),
            Values::path_hint()
        );
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> Values {
        Values::new(BTreeMap::from([
            ("GIT_USER_NAME".to_string(), "Ada".to_string()),
            ("GIT_USER_EMAIL".to_string(), "ada@example.com".to_string()),
        ]))
    }

    #[test]
    fn substitutes_known_variables() {
        let rendered = render(
            "name = ${GIT_USER_NAME}\nemail = ${GIT_USER_EMAIL}",
            &values(),
        )
        .unwrap();
        assert_eq!(rendered, "name = Ada\nemail = ada@example.com");
    }

    #[test]
    fn fails_loudly_on_missing_variables() {
        let err = render("a = ${NOPE}\nb = ${ALSO_NOPE}", &values()).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("NOPE"), "{message}");
        assert!(message.contains("ALSO_NOPE"), "{message}");
        assert!(message.contains("values.toml"), "{message}");
    }

    #[test]
    fn leaves_shell_syntax_alone() {
        // `.zshrc` is full of `$HOME` and `${SHELL##*/}`; only `${...}` with a
        // value we know is substituted, and an unterminated `${` is literal.
        let rendered = render("export PATH=\"$HOME/bin:$PATH\"", &values()).unwrap();
        assert_eq!(rendered, "export PATH=\"$HOME/bin:$PATH\"");

        let rendered = render("case ${ unterminated", &values()).unwrap();
        assert_eq!(rendered, "case ${ unterminated");
    }
}
