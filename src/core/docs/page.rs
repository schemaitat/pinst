//! One tool's page: what it is, when to reach for it, and what to type.
//!
//! A page is written for an agent that has a task and does not yet know which
//! tool does it — so it leads with intent (`what`, `when`, `keywords`) and
//! pays off in literal command lines (`recipes`). It is deliberately not a
//! reference manual: `--help` already exists and answers a different question.
//!
//! `pinst schema docs` emits the JSON Schema derived from these types, so a
//! page can be authored without reading this file.

use color_eyre::eyre::{Context, Result, bail};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How much trust a page has earned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PageStatus {
    /// Seeded from the tool's own `--help`, or written but not yet verified.
    /// Travels with the page through search, show and dump: a consumer that
    /// acts on a recipe should know nobody has run it.
    #[default]
    Draft,
    /// Written by hand, and every recipe in it was executed on this machine.
    Authored,
}

impl PageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PageStatus::Draft => "draft",
            PageStatus::Authored => "authored",
        }
    }
}

/// One thing you might want to do with the tool, and the exact invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// The literal command line, ready to run.
    pub cmd: String,
    /// What it does, in one clause.
    pub does: String,
}

/// A tool page, as authored in `docs/tools/<name>.toml`.
///
/// Unknown fields are rejected rather than ignored: these files are written by
/// hand, and a page whose `recipe` table was meant to be `recipes` would
/// otherwise load clean and silently answer nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolDoc {
    /// The tool this page is about. Taken from the file stem, never authored
    /// in the file — two spellings of the same fact is one of them being wrong.
    #[serde(default, skip_deserializing)]
    pub name: String,
    /// One line: what the tool is and what it does.
    pub what: String,
    /// When to reach for this one rather than something else.
    #[serde(default)]
    pub when: Option<String>,
    /// Extra search terms — the words someone would use for this task without
    /// knowing the tool's name.
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub status: PageStatus,
    /// The tool version the recipes were verified against.
    #[serde(default)]
    pub verified_with: Option<String>,
    #[serde(default)]
    pub recipes: Vec<Recipe>,
    /// Traps worth knowing before the first surprise.
    #[serde(default)]
    pub gotchas: Option<String>,
    /// Sibling tools worth knowing about — what this composes with, and what
    /// it competes with.
    #[serde(default)]
    pub see_also: Vec<String>,
    /// Raw captured `--help`, carried by a page that `docs adopt` seeded.
    #[serde(default)]
    pub help: Option<String>,
}

impl ToolDoc {
    /// Parses a page, naming it from its file stem.
    pub fn parse(name: &str, text: &str) -> Result<Self> {
        let mut doc: ToolDoc =
            toml::from_str(text).with_context(|| format!("parsing docs page for {name}"))?;
        doc.name = name.to_string();
        doc.validate()?;
        Ok(doc)
    }

    /// The one field with no sensible empty value: a page whose `what` is
    /// blank answers nothing, and would still rank in search.
    fn validate(&self) -> Result<()> {
        if self.what.trim().is_empty() {
            bail!("docs page for {} has an empty `what`", self.name);
        }
        Ok(())
    }

    pub fn is_draft(&self) -> bool {
        self.status == PageStatus::Draft
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"
what = "Recursive regex search."
when = "Any content search over a repo."
keywords = ["grep", "search"]
status = "authored"
verified_with = "14.1.0"
gotchas = "Skips ignored files."
see_also = ["fd"]

[[recipes]]
cmd = "rg -n pattern"
does = "Search with line numbers."
"#;

    #[test]
    fn status_renders_as_the_word_that_is_written_in_the_file() {
        assert_eq!(PageStatus::Draft.as_str(), "draft");
        assert_eq!(PageStatus::Authored.as_str(), "authored");
    }

    #[test]
    fn a_full_page_round_trips() {
        let doc = ToolDoc::parse("ripgrep", FULL).unwrap();
        assert_eq!(doc.name, "ripgrep");
        assert_eq!(doc.status, PageStatus::Authored);
        assert_eq!(doc.recipes.len(), 1);
        assert_eq!(doc.recipes[0].cmd, "rg -n pattern");
        assert_eq!(doc.see_also, vec!["fd"]);
        assert!(!doc.is_draft());
    }

    #[test]
    fn everything_but_what_is_optional_and_a_page_defaults_to_draft() {
        let doc = ToolDoc::parse("fd", r#"what = "Fast find.""#).unwrap();
        assert_eq!(doc.status, PageStatus::Draft);
        assert!(doc.recipes.is_empty());
        assert!(doc.when.is_none());
    }

    #[test]
    fn a_name_authored_in_the_file_is_rejected_rather_than_silently_ignored() {
        // Two spellings of the same fact is one of them being wrong; the file
        // name is the one the loader can trust, so writing the other one is an
        // error rather than a no-op.
        let err =
            ToolDoc::parse("fd", "name = \"something-else\"\nwhat = \"Fast find.\"").unwrap_err();
        assert!(err.to_string().contains("fd"), "{err}");
    }

    #[test]
    fn a_misspelled_table_is_rejected_rather_than_dropped() {
        // `[[recipe]]` instead of `[[recipes]]` would otherwise parse clean
        // and produce a page with no recipes at all.
        let err = ToolDoc::parse(
            "fd",
            "what = \"Fast find.\"\n[[recipe]]\ncmd = \"fd x\"\ndoes = \"find x\"\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("fd"), "{err}");
    }

    #[test]
    fn an_empty_what_is_rejected() {
        let err = ToolDoc::parse("fd", r#"what = "  ""#).unwrap_err();
        assert!(err.to_string().contains("empty `what`"), "{err}");
    }

    #[test]
    fn a_missing_what_is_rejected() {
        let err = ToolDoc::parse("fd", r#"when = "sometimes""#).unwrap_err();
        assert!(err.to_string().contains("fd"), "{err}");
    }

    #[test]
    fn an_unknown_status_is_rejected() {
        let err = ToolDoc::parse("fd", "what = \"Fast find.\"\nstatus = \"blessed\"").unwrap_err();
        assert!(err.to_string().contains("fd"), "{err}");
    }

    #[test]
    fn a_recipe_missing_its_explanation_is_rejected() {
        let err = ToolDoc::parse(
            "fd",
            "what = \"Fast find.\"\n[[recipes]]\ncmd = \"fd foo\"\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("fd"), "{err}");
    }
}
