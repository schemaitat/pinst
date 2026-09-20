//! Turning a capture into something a human can promote.
//!
//! `docs adopt` writes what the machine said into the repo as a `draft` page.
//! It is a starting point, never an answer: the whole value of the catalogue
//! is the part a person adds — which invocation is the right one here, and
//! which tool to reach for instead.

use crate::core::manifest::Tool;

use super::capture::Capture;
use super::page::{PageStatus, ToolDoc};

/// The page a capture stands in for when nothing has been written yet.
///
/// Returning a real `ToolDoc` rather than a second shape means `docs show`
/// answers with one schema whatever the source — the `source` field, not the
/// shape, is what says how much to trust it.
pub fn synthesize(tool: &Tool, capture: &Capture) -> ToolDoc {
    ToolDoc {
        name: tool.name.clone(),
        what: if tool.summary.trim().is_empty() {
            format!("{} (no summary in the manifest)", tool.name)
        } else {
            tool.summary.clone()
        },
        when: None,
        keywords: Vec::new(),
        status: PageStatus::Draft,
        verified_with: None,
        recipes: Vec::new(),
        gotchas: None,
        see_also: Vec::new(),
        help: Some(capture.text.clone()),
    }
}

/// The TOML `docs adopt` writes.
///
/// Rendered by hand rather than serialized: the file is going to be edited by
/// a person, so it gets comments, a deliberate key order, and the captured
/// help as a raw literal block instead of one escaped line.
pub fn render(tool: &Tool, capture: &Capture) -> String {
    let doc = synthesize(tool, capture);
    let mut out = String::new();

    out.push_str(&format!(
        "# Seeded by `pinst docs adopt {}` from `{}`.\n",
        tool.name, capture.command
    ));
    out.push_str("# To promote it: rewrite `what` and `when` for a reader who does not know\n");
    out.push_str("# this tool, add recipes you have actually run, drop `help`, then set\n");
    out.push_str("# status = \"authored\" and verified_with = \"<version>\".\n");
    out.push_str("#\n");
    out.push_str("# Every top-level key must stay above the first [[recipes]] table: in TOML a\n");
    out.push_str("# bare key after an array-of-tables belongs to that table, not to the page.\n");
    out.push_str(&format!("what = {}\n", basic_string(&doc.what)));
    out.push_str("status = \"draft\"\n");
    out.push_str("keywords = []\n");
    out.push_str(&format!("help = {}\n", literal_block(&capture.text)));
    out
}

/// A TOML basic string, escaped.
fn basic_string(value: &str) -> String {
    // serde's TOML writer already knows every escape this needs; borrowing it
    // beats maintaining a second escaping table.
    toml::Value::String(value.to_string()).to_string()
}

/// Captured help as a multi-line literal string — no escaping, so backslashes
/// and quotes in usage text survive exactly as the tool printed them.
///
/// Literal strings cannot contain `'''`, and cannot end in `'`. In those rare
/// cases the text falls back to an escaped basic string, which is uglier but
/// always correct.
fn literal_block(text: &str) -> String {
    let body = text.trim_end_matches('\n');
    if body.contains("'''") || body.ends_with('\'') {
        return basic_string(text);
    }
    format!("'''\n{body}\n'''")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::manifest::{Detect, Install};

    fn tool(name: &str, summary: &str) -> Tool {
        Tool {
            name: name.to_string(),
            summary: summary.to_string(),
            tags: Vec::new(),
            requires: Vec::new(),
            detect: Detect {
                command: "true".to_string(),
                bin: Some(name.to_string()),
                version_cmd: None,
                version_regex: None,
            },
            help_cmd: None,
            install: Install::Manual {
                note: String::new(),
            },
            upgrade: None,
            post_install: Vec::new(),
        }
    }

    fn capture(text: &str) -> Capture {
        Capture {
            command: "fd --help".to_string(),
            version: Some("10.1.0".to_string()),
            text: text.to_string(),
            truncated: false,
        }
    }

    #[test]
    fn a_seeded_page_is_a_draft_carrying_the_captured_help() {
        let doc = synthesize(&tool("fd", "Fast find"), &capture("usage: fd"));
        assert_eq!(doc.status, PageStatus::Draft);
        assert_eq!(doc.what, "Fast find");
        assert_eq!(doc.help.as_deref(), Some("usage: fd"));
        assert!(doc.recipes.is_empty());
    }

    #[test]
    fn a_tool_with_no_summary_still_gets_a_what() {
        // `what` is the one field a page cannot omit, so the seed cannot
        // produce a file that will not load.
        let doc = synthesize(&tool("fd", "  "), &capture("usage"));
        assert!(!doc.what.trim().is_empty());
    }

    #[test]
    fn what_is_rendered_and_parses_back() {
        let rendered = render(&tool("fd", "Fast find"), &capture("usage: fd [OPTIONS]"));
        let parsed = ToolDoc::parse("fd", &rendered).unwrap();
        assert_eq!(parsed.what, "Fast find");
        assert_eq!(parsed.status, PageStatus::Draft);
        assert_eq!(
            parsed.help.as_deref().unwrap().trim(),
            "usage: fd [OPTIONS]"
        );
    }

    #[test]
    fn help_full_of_quotes_and_backslashes_survives_the_round_trip() {
        let help = "usage: rg \"pattern\"\n  --regexp \\d+  a digit\n\ttabbed";
        let rendered = render(&tool("ripgrep", "Fast grep"), &capture(help));
        let parsed = ToolDoc::parse("ripgrep", &rendered).unwrap();
        assert_eq!(parsed.help.as_deref().unwrap().trim(), help);
    }

    #[test]
    fn help_containing_a_literal_terminator_falls_back_to_an_escaped_string() {
        let help = "here is ''' a terminator";
        let rendered = render(&tool("odd", "Odd"), &capture(help));
        let parsed = ToolDoc::parse("odd", &rendered).unwrap();
        assert_eq!(parsed.help.as_deref().unwrap(), help);
    }

    #[test]
    fn a_summary_with_quotes_does_not_break_the_page() {
        let rendered = render(&tool("odd", "the \"odd\" one"), &capture("usage"));
        let parsed = ToolDoc::parse("odd", &rendered).unwrap();
        assert_eq!(parsed.what, "the \"odd\" one");
    }

    #[test]
    fn the_seed_names_the_command_it_came_from() {
        let rendered = render(&tool("fd", "Fast find"), &capture("usage"));
        assert!(rendered.contains("from `fd --help`"), "{rendered}");
    }
}
