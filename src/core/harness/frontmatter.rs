//! The YAML frontmatter block, parsed strictly.
//!
//! "Strictly" is the whole design. The bash harness read this with `awk`, one
//! line at a time, picking out the key it wanted and ignoring everything else
//! — and LESSON-008 records what that cost: six `SKILL.md` files "had never
//! been valid YAML because only hand-written parsers had ever read them". A
//! reader that skips what it does not understand cannot report a malformed
//! document; it reports a *missing key*, or nothing at all.
//!
//! So this parser accepts a deliberately small subset — the one every
//! document in the corpus actually uses — and **refuses** anything else with
//! the line number:
//!
//! ```text
//! key: plain scalar
//! key: 'single quoted, with '' for a literal quote'
//! key: "double quoted"
//! key: [inline, list]
//! ```
//!
//! Nested mappings, block sequences, block scalars (`|`, `>`), duplicate keys
//! and trailing `#` comments on a plain scalar are all errors. Refusing is the
//! safe direction: a document outside the subset becomes a finding naming the
//! line, never a silently wrong value.

// This module lands one phase ahead of its callers, and `-D warnings` makes
// that a build failure. Scoped to non-test builds so the tests still have to
// exercise everything, and dated rather than left as a bare `allow`:
// every consumer arrives with `index` (phase 2) and `check` (phase 3).
// Delete this attribute when that phase lands.
#![cfg_attr(not(test), allow(dead_code))]

use std::fmt;

/// A frontmatter value. Only two shapes exist in this corpus, and collapsing
/// them would lose the distinction `areas: [a, b]` needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Scalar(String),
    List(Vec<String>),
}

impl Value {
    /// The value as the index renders it: a list joins with `, `, which is
    /// what the bash `delist()` produced by stripping the brackets.
    pub fn text(&self) -> String {
        match self {
            Value::Scalar(s) => s.clone(),
            Value::List(items) => items.join(", "),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// 1-based, counted over the whole file so it matches an editor.
    pub line: usize,
    pub reason: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.reason)
    }
}

impl std::error::Error for ParseError {}

/// Key order is preserved: it is the order the document declares, and a
/// report that reshuffles a document's own keys is harder to diff.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frontmatter {
    entries: Vec<(String, Value)>,
}

impl Frontmatter {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// The value as text, whatever shape it has. `None` means the key is
    /// absent — which is a finding, not a parse error.
    pub fn text(&self, key: &str) -> Option<String> {
        self.get(key).map(Value::text)
    }

    /// The value only if it is a scalar.
    pub fn scalar(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(Value::Scalar(s)) => Some(s),
            _ => None,
        }
    }

    /// The value only if it is an inline list.
    pub fn list(&self, key: &str) -> Option<&[String]> {
        match self.get(key) {
            Some(Value::List(items)) => Some(items),
            _ => None,
        }
    }

    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Parses the block between the first two `---` lines.
///
/// `Ok(None)` means the file has no frontmatter at all — a distinct state
/// from a malformed one, and the callers report them as different findings.
pub fn parse(text: &str) -> Result<Option<Frontmatter>, ParseError> {
    let mut lines = text.lines().enumerate();

    match lines.next() {
        Some((_, first)) if is_fence(first) => {}
        _ => return Ok(None),
    }

    let mut fm = Frontmatter::default();
    for (index, raw) in lines {
        let number = index + 1;
        if is_fence(raw) {
            return Ok(Some(fm));
        }
        let line = raw.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let (key, value) = split_entry(line, number)?;
        if fm.has(&key) {
            return Err(ParseError {
                line: number,
                reason: format!("'{key}' is declared twice — the second wins silently in YAML"),
            });
        }
        fm.entries.push((key, value));
    }

    Err(ParseError {
        line: text.lines().count(),
        reason: "the frontmatter block is opened by '---' but never closed".to_string(),
    })
}

fn is_fence(line: &str) -> bool {
    line.trim_end() == "---"
}

fn split_entry(line: &str, number: usize) -> Result<(String, Value), ParseError> {
    let reject = |reason: &str| ParseError {
        line: number,
        reason: reason.to_string(),
    };

    if line.starts_with([' ', '\t']) {
        return Err(reject(
            "indented — nested mappings and multi-line values are not supported here; \
             keep the block flat",
        ));
    }
    if line.starts_with("- ") {
        return Err(reject(
            "a block sequence — write the list inline as [a, b, c]",
        ));
    }

    let colon = line
        .find(':')
        .ok_or_else(|| reject("expected 'key: value'"))?;
    let key = &line[..colon];
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(reject(&format!("'{key}' is not a plain key")));
    }
    let rest = &line[colon + 1..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return Err(reject("a key and its value must be separated by ': '"));
    }

    Ok((key.to_string(), parse_value(rest.trim(), number)?))
}

fn parse_value(value: &str, number: usize) -> Result<Value, ParseError> {
    let reject = |reason: &str| ParseError {
        line: number,
        reason: reason.to_string(),
    };

    if value.is_empty() {
        return Ok(Value::Scalar(String::new()));
    }
    if value.starts_with('|') || value.starts_with('>') {
        return Err(reject(
            "a block scalar — put the value on one line, quoting it if needed",
        ));
    }

    if let Some(inner) = value.strip_prefix('[') {
        let inner = inner.strip_suffix(']').ok_or_else(|| {
            reject("an inline list that is never closed — expected a trailing ']'")
        })?;
        if inner.trim().is_empty() {
            return Ok(Value::List(Vec::new()));
        }
        return Ok(Value::List(
            inner
                .split(',')
                .map(|item| item.trim().to_string())
                .collect(),
        ));
    }

    if value.starts_with('\'') {
        let inner = value
            .strip_prefix('\'')
            .and_then(|v| v.strip_suffix('\''))
            .filter(|_| value.len() >= 2)
            .ok_or_else(|| reject("a single-quoted scalar that is never closed"))?;
        // In a single-quoted YAML scalar a literal quote is written twice.
        // Checking that no *odd* run of quotes survives is what catches a
        // value that ended early and left trailing text outside the quotes.
        if odd_quote_run(inner) {
            return Err(reject(
                "a single-quoted scalar with an unescaped quote — write a literal ' as ''",
            ));
        }
        return Ok(Value::Scalar(inner.replace("''", "'")));
    }

    if value.starts_with('"') {
        let inner = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .filter(|_| value.len() >= 2)
            .ok_or_else(|| reject("a double-quoted scalar that is never closed"))?;
        // YAML gives backslash escapes meaning inside double quotes and not
        // inside single ones, and guessing wrong changes the value. Refuse
        // rather than pick.
        if inner.contains('\\') || inner.contains('"') {
            return Err(reject(
                "a double-quoted scalar with an escape — use a single-quoted scalar instead",
            ));
        }
        return Ok(Value::Scalar(inner.to_string()));
    }

    // A plain scalar runs to end of line, except that YAML would treat " #"
    // as starting a comment. Nothing in this corpus relies on either reading,
    // so the ambiguity is refused rather than resolved.
    if value.contains(" #") {
        return Err(reject(
            "a plain scalar containing ' #', which YAML reads as a comment — quote the value",
        ));
    }
    Ok(Value::Scalar(value.to_string()))
}

/// True when the string contains a run of `'` of odd length, i.e. a quote
/// that is not part of a doubled pair.
fn odd_quote_run(value: &str) -> bool {
    let mut run = 0usize;
    for ch in value.chars() {
        if ch == '\'' {
            run += 1;
        } else {
            if run % 2 == 1 {
                return true;
            }
            run = 0;
        }
    }
    run % 2 == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn parsed(text: &str) -> Frontmatter {
        parse(text)
            .expect("should parse")
            .expect("should have frontmatter")
    }

    fn error(text: &str) -> ParseError {
        parse(text).expect_err("should be rejected")
    }

    #[test]
    fn a_file_with_no_fence_has_no_frontmatter() {
        assert_eq!(parse("# Just a heading\n").unwrap(), None);
    }

    #[test]
    fn reads_the_shapes_the_corpus_uses() {
        let fm = parsed(concat!(
            "---\n",
            "id: 260919-qwerty\n",
            "status: In Progress\n",
            "areas: [cli, harness, agents]\n",
            "empty: []\n",
            "produces: 'every commit in this repo''s history parses'\n",
            "quoted: \"plain enough\"\n",
            "blank:\n",
            "---\n",
            "# Body\n",
        ));

        assert_eq!(fm.scalar("id"), Some("260919-qwerty"));
        assert_eq!(fm.scalar("status"), Some("In Progress"));
        assert_eq!(fm.list("areas").unwrap(), ["cli", "harness", "agents"]);
        assert_eq!(fm.list("empty").unwrap(), [] as [String; 0]);
        assert_eq!(
            fm.scalar("produces"),
            Some("every commit in this repo's history parses")
        );
        assert_eq!(fm.scalar("quoted"), Some("plain enough"));
        assert_eq!(fm.scalar("blank"), Some(""));
        assert!(!fm.has("nothing"));
    }

    /// A list renders the way `delist()` did, because the index's Areas column
    /// is generated from it and must not change shape across the port.
    #[test]
    fn a_list_renders_as_the_index_prints_it() {
        let fm = parsed("---\nareas: [cli, harness]\n---\n");
        assert_eq!(fm.text("areas").as_deref(), Some("cli, harness"));
    }

    /// A plain scalar may contain a colon — `summary:` routinely does — so the
    /// split is on the *first* colon only.
    #[test]
    fn a_plain_scalar_may_contain_a_colon() {
        let fm = parsed("---\nproduces: the full ADR: Context, Decision\n---\n");
        assert_eq!(
            fm.scalar("produces"),
            Some("the full ADR: Context, Decision")
        );
    }

    #[test]
    fn an_indented_line_is_rejected_with_its_line_number() {
        let err = error("---\nmeta:\n  nested: true\n---\n");
        assert_eq!(err.line, 3);
        assert!(err.reason.contains("nested mappings"), "{}", err.reason);
    }

    #[test]
    fn the_other_constructs_outside_the_subset_are_rejected() {
        for (text, line) in [
            ("---\nitems:\n- one\n---\n", 3),
            ("---\nbody: |\n---\n", 2),
            ("---\nareas: [cli, harness\n---\n", 2),
            ("---\nproduces: 'never closed\n---\n", 2),
            ("---\nsummary: a value # with a comment\n---\n", 2),
            ("---\nid: one\nid: two\n---\n", 3),
            ("---\nnot a key at all\n---\n", 2),
        ] {
            let err = error(text);
            assert_eq!(err.line, line, "wrong line for {text:?}: {err}");
        }
    }

    #[test]
    fn an_unclosed_block_is_an_error_not_an_empty_frontmatter() {
        let err = error("---\nid: 260919-qwerty\n");
        assert!(err.reason.contains("never closed"), "{}", err.reason);
    }

    fn repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn documents() -> Vec<PathBuf> {
        let mut found = Vec::new();
        let plans = repo().join(".ash/plans");
        for entry in std::fs::read_dir(&plans).expect("the corpus must exist") {
            let dir = entry.unwrap().path();
            if !dir.is_dir() {
                continue;
            }
            for file in std::fs::read_dir(&dir).unwrap() {
                let path = file.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                if name == "README.md" || name.starts_with("phase-") {
                    found.push(path);
                }
            }
        }
        for entry in std::fs::read_dir(repo().join(".agents/skills")).unwrap() {
            let skill = entry.unwrap().path().join("SKILL.md");
            if skill.is_file() {
                found.push(skill);
            }
        }
        found
    }

    /// This is what pins the accepted subset to reality rather than to an
    /// author's guess, and it is the test that would have caught LESSON-008's
    /// six invalid `SKILL.md` files. Every document the harness reads must
    /// parse — if one does not, either the document or the subset is wrong,
    /// and the failure says which file to look at.
    #[test]
    fn every_document_in_the_real_corpus_parses() {
        let documents = documents();
        assert!(
            documents.len() > 20,
            "expected a real corpus, found {}",
            documents.len()
        );

        for path in documents {
            let text = std::fs::read_to_string(&path).unwrap();
            let shown = display(&path);
            match parse(&text) {
                Ok(Some(fm)) => assert!(!fm.is_empty(), "{shown} has an empty frontmatter block"),
                Ok(None) => panic!("{shown} has no frontmatter block at all"),
                Err(err) => panic!("{shown} does not parse: {err}"),
            }
        }
    }

    fn display(path: &Path) -> String {
        path.strip_prefix(repo())
            .unwrap_or(path)
            .display()
            .to_string()
    }
}
