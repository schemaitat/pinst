//! Finding the right tool from an intention rather than a name.
//!
//! The query this serves is "search a tree", not "ripgrep" — so the catalogue
//! is matched term by term, and a page's `keywords` exist precisely to catch
//! the words someone would use without knowing the tool.
//!
//! The ranking is a table of literal rules rather than a similarity score.
//! Twenty-six entries and a few hundred keywords are matched exactly in well
//! under a millisecond, every result can be explained by pointing at the rule
//! that produced it, and nothing new has to be linked into a binary tuned for
//! size.

use crate::core::manifest::Tool;

use super::page::{Recipe, ToolDoc};

/// Which part of the entry the best match came from. Reported so a caller can
/// see *why* something ranked where it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Field {
    Name,
    Keyword,
    Recipe,
    Prose,
    Tag,
    Help,
}

impl Field {
    pub fn as_str(&self) -> &'static str {
        match self {
            Field::Name => "name",
            Field::Keyword => "keyword",
            Field::Recipe => "recipe",
            Field::Prose => "prose",
            Field::Tag => "tag",
            Field::Help => "help",
        }
    }
}

/// The score a single term earns against one field. Ordered so that knowing
/// the tool's name beats describing what it does, and a recipe — which is the
/// thing the caller is going to run — beats prose about it.
const NAME_EXACT: u32 = 100;
const NAME_PART: u32 = 80;
const KEYWORD_EXACT: u32 = 70;
const KEYWORD_PART: u32 = 55;
const RECIPE_CMD: u32 = 50;
const RECIPE_DOES: u32 = 45;
const PROSE: u32 = 40;
const TAG_EXACT: u32 = 30;
/// Captured `--help` is matched last and cheaply: it is long, it is not
/// curated, and a hit in it says much less than a hit in a written line.
const HELP: u32 = 10;

/// Words that carry no intent. Dropped so "how do I search a tree" ranks the
/// same as "search tree" — unless the query is nothing but these, in which
/// case they are all we have.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "by", "can", "do", "for", "from", "how", "i", "in", "is",
    "it", "me", "my", "of", "on", "or", "the", "to", "what", "with",
];

/// One tool, with its page when it has one.
#[derive(Debug, Clone, Copy)]
pub struct Entry<'a> {
    pub tool: &'a Tool,
    pub page: Option<&'a ToolDoc>,
}

#[derive(Debug)]
pub struct Hit<'a> {
    pub tool: &'a Tool,
    pub page: Option<&'a ToolDoc>,
    pub score: u32,
    pub field: Field,
    /// The recipes that matched, so one search is enough to act on.
    pub recipes: Vec<&'a Recipe>,
}

struct Ranked<'a> {
    entry: Entry<'a>,
    score: u32,
    field: Field,
}

/// Ranks entries against a query, best first. Entries that match nothing are
/// left out entirely; an empty result is a correct answer, not an error.
pub fn search<'a>(entries: &[Entry<'a>], query: &str) -> Vec<Hit<'a>> {
    let terms = terms(query);
    rank(entries, &terms)
        .into_iter()
        .map(|ranked| Hit {
            tool: ranked.entry.tool,
            page: ranked.entry.page,
            score: ranked.score,
            field: ranked.field,
            recipes: matching_recipes(&ranked.entry, &terms),
        })
        .collect()
}

/// The same ranking as [`search`], without collecting matching recipes.
/// The TUI only needs ordered tools and should not lowercase every recipe
/// merely because a frame was drawn.
pub fn search_tools<'a>(entries: &[Entry<'a>], query: &str) -> Vec<&'a Tool> {
    let terms = terms(query);
    rank(entries, &terms)
        .into_iter()
        .map(|ranked| ranked.entry.tool)
        .collect()
}

fn rank<'a>(entries: &[Entry<'a>], terms: &[String]) -> Vec<Ranked<'a>> {
    if terms.is_empty() {
        return Vec::new();
    }

    let mut hits: Vec<Ranked<'a>> = entries
        .iter()
        .filter_map(|entry| score(entry, terms))
        .collect();

    // Ties break on name so the order is stable across runs — a result set
    // that reshuffles is one nobody can write a test or a habit around.
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.entry.tool.name.cmp(&b.entry.tool.name))
    });
    hits
}

fn terms(query: &str) -> Vec<String> {
    let all: Vec<String> = query
        .split_whitespace()
        .map(|term| term.trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_'))
        .filter(|term| !term.is_empty())
        .map(|term| term.to_lowercase())
        .collect();

    let meaningful: Vec<String> = all
        .iter()
        .filter(|term| !STOPWORDS.contains(&term.as_str()))
        .cloned()
        .collect();

    if meaningful.is_empty() {
        all
    } else {
        meaningful
    }
}

/// Sums each term's best field score. Summing rather than taking the maximum
/// is what makes a two-word query prefer the entry that answers both halves.
fn score<'a>(entry: &Entry<'a>, terms: &[String]) -> Option<Ranked<'a>> {
    let mut total = 0;
    let mut best: Option<(u32, Field)> = None;

    for term in terms {
        if let Some((points, field)) = best_field(entry, term) {
            total += points;
            if best.is_none_or(|(existing, _)| points > existing) {
                best = Some((points, field));
            }
        }
    }

    let (_, field) = best?;
    Some(Ranked {
        entry: *entry,
        score: total,
        field,
    })
}

fn best_field(entry: &Entry<'_>, term: &str) -> Option<(u32, Field)> {
    let name = entry.tool.name.to_lowercase();
    if name == term {
        return Some((NAME_EXACT, Field::Name));
    }
    // The binary is as good a name as the manifest entry: someone searching
    // for "rg" means ripgrep.
    if entry
        .tool
        .detect
        .bin
        .as_deref()
        .is_some_and(|bin| bin.eq_ignore_ascii_case(term))
    {
        return Some((NAME_EXACT, Field::Name));
    }
    if name.contains(term) {
        return Some((NAME_PART, Field::Name));
    }

    if let Some(page) = entry.page {
        for keyword in &page.keywords {
            let keyword = keyword.to_lowercase();
            if keyword == term {
                return Some((KEYWORD_EXACT, Field::Keyword));
            }
            if keyword.contains(term) {
                return Some((KEYWORD_PART, Field::Keyword));
            }
        }
        for recipe in &page.recipes {
            if recipe.cmd.to_lowercase().contains(term) {
                return Some((RECIPE_CMD, Field::Recipe));
            }
            if recipe.does.to_lowercase().contains(term) {
                return Some((RECIPE_DOES, Field::Recipe));
            }
        }
        if page.what.to_lowercase().contains(term)
            || page
                .when
                .as_deref()
                .is_some_and(|when| when.to_lowercase().contains(term))
            || page
                .gotchas
                .as_deref()
                .is_some_and(|gotchas| gotchas.to_lowercase().contains(term))
        {
            return Some((PROSE, Field::Prose));
        }
    }

    if entry.tool.summary.to_lowercase().contains(term) {
        return Some((PROSE, Field::Prose));
    }
    if entry
        .tool
        .tags
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case(term))
    {
        return Some((TAG_EXACT, Field::Tag));
    }
    if let Some(page) = entry.page
        && page
            .help
            .as_deref()
            .is_some_and(|help| help.to_lowercase().contains(term))
    {
        return Some((HELP, Field::Help));
    }

    None
}

fn matching_recipes<'a>(entry: &Entry<'a>, terms: &[String]) -> Vec<&'a Recipe> {
    let Some(page) = entry.page else {
        return Vec::new();
    };
    page.recipes
        .iter()
        .filter(|recipe| {
            let cmd = recipe.cmd.to_lowercase();
            let does = recipe.does.to_lowercase();
            terms
                .iter()
                .any(|term| cmd.contains(term) || does.contains(term))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::manifest::{Detect, Install};

    fn tool(name: &str, bin: Option<&str>, summary: &str, tags: &[&str]) -> Tool {
        Tool {
            name: name.to_string(),
            summary: summary.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            requires: Vec::new(),
            detect: Detect {
                command: "true".to_string(),
                bin: bin.map(|b| b.to_string()),
                version_cmd: None,
                version_regex: None,
            },
            help_cmd: None,
            install: Install::Manual {
                note: String::new(),
            },
            upgrade: None,
            post_install: Vec::new(),
            platform_overrides: Default::default(),
        }
    }

    fn page(name: &str, body: &str) -> ToolDoc {
        ToolDoc::parse(name, body).unwrap()
    }

    const RIPGREP: &str = r#"
what = "Recursive regex search across a directory tree."
keywords = ["grep", "search", "find text"]

[[recipes]]
cmd = "rg -n 'pattern' path/"
does = "Search a path, printing file:line."

[[recipes]]
cmd = "rg --files"
does = "List every file it would search."
"#;

    const JUST: &str = r#"
what = "Task runner: runs recipes from a justfile."
keywords = ["task runner", "make", "recipe"]

[[recipes]]
cmd = "just --list"
does = "Show the available recipes."
"#;

    struct Fixture {
        tools: Vec<Tool>,
        pages: Vec<ToolDoc>,
    }

    impl Fixture {
        fn new() -> Self {
            Fixture {
                tools: vec![
                    tool("ripgrep", Some("rg"), "Fast grep", &["dev", "editor"]),
                    tool("just", Some("just"), "Command runner", &["dev"]),
                    tool("curl", Some("curl"), "HTTP client", &["base"]),
                    tool("makeish", None, "Runs recipes from a Makefile", &["dev"]),
                ],
                pages: vec![page("ripgrep", RIPGREP), page("just", JUST)],
            }
        }

        fn entries(&self) -> Vec<Entry<'_>> {
            self.tools
                .iter()
                .map(|tool| Entry {
                    tool,
                    page: self.pages.iter().find(|p| p.name == tool.name),
                })
                .collect()
        }
    }

    fn names<'a>(hits: &'a [Hit<'a>]) -> Vec<&'a str> {
        hits.iter().map(|h| h.tool.name.as_str()).collect()
    }

    #[test]
    fn an_exact_name_outranks_everything_else() {
        let fixture = Fixture::new();
        let hits = search(&fixture.entries(), "just");
        assert_eq!(hits[0].tool.name, "just");
        assert_eq!(hits[0].field, Field::Name);
    }

    #[test]
    fn the_binary_name_finds_the_tool_the_manifest_calls_something_else() {
        // Nobody types "ripgrep" at a prompt.
        let fixture = Fixture::new();
        let hits = search(&fixture.entries(), "rg");
        assert_eq!(hits[0].tool.name, "ripgrep");
        assert_eq!(hits[0].field, Field::Name);
    }

    #[test]
    fn a_keyword_outranks_prose() {
        // "recipe" is a keyword on just's page and prose in makeish's summary.
        let fixture = Fixture::new();
        let hits = search(&fixture.entries(), "recipe");
        assert_eq!(names(&hits), vec!["just", "makeish"]);
        assert_eq!(hits[0].field, Field::Keyword);
        assert_eq!(hits[1].field, Field::Prose);
    }

    #[test]
    fn a_name_that_contains_the_query_outranks_a_keyword_elsewhere() {
        // "ripgrep" contains "grep" — knowing the name is the strongest
        // signal there is, even when another entry lists it as a keyword.
        let fixture = Fixture::new();
        let hits = search(&fixture.entries(), "grep");
        assert_eq!(hits[0].tool.name, "ripgrep");
        assert_eq!(hits[0].field, Field::Name);
    }

    #[test]
    fn an_intention_finds_the_tool_without_naming_it() {
        let fixture = Fixture::new();
        let hits = search(&fixture.entries(), "how do I search a tree");
        assert_eq!(hits[0].tool.name, "ripgrep");
    }

    #[test]
    fn a_two_word_query_prefers_the_entry_that_answers_both_halves() {
        // Summing term scores, rather than taking the best single one, is what
        // makes this true.
        let fixture = Fixture::new();
        let hits = search(&fixture.entries(), "task runner");
        assert_eq!(hits[0].tool.name, "just");
    }

    #[test]
    fn matching_recipes_come_back_with_the_hit() {
        let fixture = Fixture::new();
        let hits = search(&fixture.entries(), "pattern");
        assert_eq!(hits[0].tool.name, "ripgrep");
        assert_eq!(hits[0].recipes.len(), 1);
        assert!(hits[0].recipes[0].cmd.contains("rg -n"));
    }

    #[test]
    fn a_tool_with_no_page_is_still_findable_by_its_manifest_summary() {
        let fixture = Fixture::new();
        let hits = search(&fixture.entries(), "http");
        assert_eq!(names(&hits), vec!["curl"]);
        assert!(hits[0].page.is_none());
        assert!(hits[0].recipes.is_empty());
    }

    #[test]
    fn a_tag_matches_but_ranks_below_everything_written() {
        let fixture = Fixture::new();
        let hits = search(&fixture.entries(), "base");
        assert_eq!(hits[0].tool.name, "curl");
        assert_eq!(hits[0].field, Field::Tag);
    }

    #[test]
    fn nothing_matching_is_an_empty_result_not_an_error() {
        let fixture = Fixture::new();
        assert!(search(&fixture.entries(), "kubernetes").is_empty());
    }

    #[test]
    fn an_empty_query_matches_nothing() {
        let fixture = Fixture::new();
        assert!(search(&fixture.entries(), "   ").is_empty());
    }

    #[test]
    fn a_query_of_nothing_but_stopwords_still_searches() {
        // Dropping every term would turn "it" into an empty query and answer
        // nothing at all; the fallback keeps the terms it would have dropped.
        let fixture = Fixture::new();
        let hits = search(&fixture.entries(), "it");
        assert!(
            !hits.is_empty(),
            "a stopword-only query falls back to matching the stopwords"
        );

        // And the same word inside a longer query is still dropped, so it
        // cannot drag an unrelated entry up the ranking.
        let intent = search(&fixture.entries(), "it search tree");
        assert_eq!(intent[0].tool.name, "ripgrep");
    }

    #[test]
    fn matching_is_case_insensitive_and_ignores_punctuation() {
        let fixture = Fixture::new();
        let hits = search(&fixture.entries(), "GREP,");
        assert_eq!(hits[0].tool.name, "ripgrep");
    }

    #[test]
    fn ties_break_on_the_tool_name_so_the_order_is_stable() {
        let tools = [
            tool("zed", None, "An editor", &["dev"]),
            tool("alpha", None, "An editor", &["dev"]),
        ];
        let entries: Vec<Entry<'_>> = tools
            .iter()
            .map(|tool| Entry { tool, page: None })
            .collect();
        let hits = search(&entries, "editor");
        assert_eq!(names(&hits), vec!["alpha", "zed"]);
    }

    #[test]
    fn captured_help_matches_last() {
        let tools = [tool("t", None, "A tool", &[])];
        let pages = [page(
            "t",
            "what = \"A tool.\"\nhelp = \"--xyzzy  does the thing\"\n",
        )];
        let entries = [Entry {
            tool: &tools[0],
            page: Some(&pages[0]),
        }];
        let hits = search(&entries, "xyzzy");
        assert_eq!(hits[0].field, Field::Help);
        assert_eq!(hits[0].score, HELP);
    }
}
