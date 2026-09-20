//! The tool catalogue: how to *use* what the manifest installs.
//!
//! The manifest says what is on this machine; this says what each entry is
//! for and what to type. One TOML page per tool under `docs/tools/`, embedded
//! into the binary exactly as `configs/` is, so a machine that was provisioned
//! from a downloaded binary can still answer "which tool searches a tree, and
//! how do I call it" with no clone and no network.
//!
//! Scope is the manifest: a page whose name matches no declared tool is a
//! repo bug, and `cargo test` is where it is caught.

pub mod capture;
pub mod page;
pub mod search;
pub mod seed;

use std::collections::BTreeMap;
use std::path::Path;

use color_eyre::eyre::{Context, Result};
use include_dir::{Dir, include_dir};

use super::source::{self, Source};
use page::ToolDoc;

/// Only `docs/tools/` is embedded — `docs/` itself also holds the
/// architecture diagrams, which have no business inside the binary.
static EMBEDDED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/docs/tools");

/// The subdirectory the catalogue lives in, relative to a checkout root.
pub const SUBDIR: &str = "docs/tools";

/// Overrides catalogue resolution, the way `PINST_SOURCE` overrides configs.
pub const SOURCE_ENV: &str = "PINST_DOCS_SOURCE";

/// Every page pinst can see this run, keyed by tool name.
#[derive(Debug)]
pub struct Catalogue {
    pub source: Source,
    pages: BTreeMap<String, ToolDoc>,
}

impl Catalogue {
    /// Loads from the checkout when there is one, else from the embedded copy.
    pub fn load() -> Result<Self> {
        Self::load_from(source::resolve(SUBDIR, SOURCE_ENV))
    }

    pub fn load_from(source: Source) -> Result<Self> {
        let pages = match &source {
            Source::Tree(root) => read_tree(root)?,
            Source::Embedded => read_embedded()?,
        };
        Ok(Self { source, pages })
    }

    pub fn get(&self, tool: &str) -> Option<&ToolDoc> {
        self.pages.get(tool)
    }

    /// Pages in tool-name order, which is what every listing renders in.
    pub fn iter(&self) -> impl Iterator<Item = &ToolDoc> {
        self.pages.values()
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// Where a page would be written. `None` when the catalogue came from the
    /// binary: there is no tree to write into, which is what `docs adopt`
    /// refuses on.
    pub fn page_path(&self, tool: &str) -> Option<std::path::PathBuf> {
        match &self.source {
            Source::Tree(root) => Some(root.join(format!("{tool}.toml"))),
            Source::Embedded => None,
        }
    }
}

fn read_tree(root: &Path) -> Result<BTreeMap<String, ToolDoc>> {
    let mut pages = BTreeMap::new();
    let entries = std::fs::read_dir(root).with_context(|| format!("reading {}", root.display()))?;
    for entry in entries {
        let path = entry
            .with_context(|| format!("reading {}", root.display()))?
            .path();
        let Some(name) = toml_stem(&path) else {
            continue;
        };
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        pages.insert(name.clone(), ToolDoc::parse(&name, &text)?);
    }
    Ok(pages)
}

fn read_embedded() -> Result<BTreeMap<String, ToolDoc>> {
    let mut pages = BTreeMap::new();
    for file in EMBEDDED.files() {
        let Some(name) = toml_stem(file.path()) else {
            continue;
        };
        let text = std::str::from_utf8(file.contents())
            .with_context(|| format!("embedded docs page {name} is not UTF-8"))?;
        pages.insert(name.clone(), ToolDoc::parse(&name, text)?);
    }
    Ok(pages)
}

/// The tool name a page file is about, or `None` for anything that is not a
/// `.toml` — a stray `README.md` in the catalogue is ignored, not an error.
fn toml_stem(path: &Path) -> Option<String> {
    if path.extension().and_then(|e| e.to_str()) != Some("toml") {
        return None;
    }
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::manifest;

    /// The catalogue compiled into the binary is the one a provisioned
    /// machine reads, so it is the one that has to be valid. Every assertion
    /// here is an invariant that can only be true or broken — unlike
    /// coverage, which is a report (`pinst docs status`) precisely because an
    /// unwritten page is a normal state.
    fn embedded() -> Catalogue {
        Catalogue::load_from(Source::Embedded).expect("every embedded page must parse")
    }

    #[test]
    fn every_embedded_page_parses() {
        let _ = embedded();
    }

    #[test]
    fn every_embedded_page_names_a_manifest_tool() {
        let manifest = manifest::load(None).unwrap().manifest;
        let known: Vec<&str> = manifest.tools.iter().map(|t| t.name.as_str()).collect();
        for doc in embedded().iter() {
            assert!(
                known.contains(&doc.name.as_str()),
                "docs/tools/{}.toml names no manifest tool; rename it or add the tool",
                doc.name
            );
        }
    }

    #[test]
    fn every_embedded_page_says_what_the_tool_is() {
        for doc in embedded().iter() {
            assert!(
                !doc.what.trim().is_empty(),
                "{} has an empty what",
                doc.name
            );
        }
    }

    #[test]
    fn every_see_also_link_names_a_real_tool() {
        // A page that points at a tool nobody has heard of sends its reader
        // somewhere that does not exist — and unlike a missing page, this is
        // always a mistake rather than work not yet done.
        let manifest = manifest::load(None).unwrap().manifest;
        for doc in embedded().iter() {
            for link in &doc.see_also {
                assert!(
                    manifest.tools.iter().any(|tool| &tool.name == link),
                    "{}'s see_also points at '{}', which the manifest does not declare",
                    doc.name,
                    link
                );
            }
        }
    }

    #[test]
    fn an_authored_page_with_recipes_says_which_version_it_was_verified_against() {
        // `authored` claims every recipe was run here; the version it was run
        // against is what makes that claim checkable later. A page with no
        // recipes — a framework, a plugin — has nothing to pin.
        for doc in embedded().iter() {
            if doc.status == page::PageStatus::Authored && !doc.recipes.is_empty() {
                assert!(
                    doc.verified_with.is_some(),
                    "{} is authored with recipes but names no verified_with",
                    doc.name
                );
            }
        }
    }

    #[test]
    fn no_recipe_is_missing_its_command_or_its_explanation() {
        for doc in embedded().iter() {
            for recipe in &doc.recipes {
                assert!(!recipe.cmd.trim().is_empty(), "{}: empty cmd", doc.name);
                assert!(!recipe.does.trim().is_empty(), "{}: empty does", doc.name);
            }
        }
    }

    #[test]
    fn a_page_that_does_not_parse_fails_the_load() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("fd.toml"), "what = ").unwrap();
        let err = Catalogue::load_from(Source::Tree(dir.path().to_path_buf())).unwrap_err();
        assert!(err.to_string().contains("fd"), "{err}");
    }

    #[test]
    fn a_page_with_an_empty_what_fails_the_load() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("fd.toml"), r#"what = """#).unwrap();
        let err = Catalogue::load_from(Source::Tree(dir.path().to_path_buf())).unwrap_err();
        assert!(err.to_string().contains("empty `what`"), "{err}");
    }

    #[test]
    fn a_page_with_an_unknown_status_fails_the_load() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("fd.toml"),
            "what = \"Fast find.\"\nstatus = \"blessed\"\n",
        )
        .unwrap();
        assert!(Catalogue::load_from(Source::Tree(dir.path().to_path_buf())).is_err());
    }

    #[test]
    fn non_toml_files_in_the_catalogue_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "not a page").unwrap();
        std::fs::write(dir.path().join("fd.toml"), r#"what = "Fast find.""#).unwrap();
        let catalogue = Catalogue::load_from(Source::Tree(dir.path().to_path_buf())).unwrap();
        assert_eq!(catalogue.len(), 1);
        assert!(catalogue.get("fd").is_some());
    }

    #[test]
    fn the_checkout_tree_and_the_embedded_copy_hold_the_same_catalogue() {
        // The same property the config tests assert: what a dev machine reads
        // off disk and what a provisioned machine reads out of the binary
        // must not be able to disagree.
        let tree = Source::Tree(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SUBDIR));
        let from_tree = Catalogue::load_from(tree).unwrap();
        let from_binary = embedded();
        let tree_names: Vec<&str> = from_tree.iter().map(|d| d.name.as_str()).collect();
        let binary_names: Vec<&str> = from_binary.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(tree_names, binary_names);
    }

    #[test]
    fn the_source_env_var_redirects_the_catalogue_at_another_checkout() {
        // Serialized against the other env-var test in `source`: set_var is
        // process-wide.
        let _guard = crate::core::source::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("manifest.toml"), "").unwrap();
        std::fs::create_dir_all(root.path().join(SUBDIR)).unwrap();
        std::fs::write(
            root.path().join(SUBDIR).join("just.toml"),
            r#"what = "Task runner.""#,
        )
        .unwrap();

        unsafe { std::env::set_var(SOURCE_ENV, root.path()) };
        let catalogue = Catalogue::load().unwrap();
        unsafe { std::env::remove_var(SOURCE_ENV) };

        assert!(catalogue.source.is_tree());
        assert_eq!(catalogue.len(), 1);
        assert!(catalogue.get("just").is_some());
    }

    #[test]
    fn an_embedded_catalogue_has_nowhere_to_write_a_page() {
        assert!(embedded().page_path("fd").is_none());
    }

    #[test]
    fn a_tree_catalogue_writes_pages_beside_the_ones_it_read() {
        let dir = tempfile::tempdir().unwrap();
        let catalogue = Catalogue::load_from(Source::Tree(dir.path().to_path_buf())).unwrap();
        assert_eq!(
            catalogue.page_path("fd").unwrap(),
            dir.path().join("fd.toml")
        );
    }

    #[test]
    fn a_catalogue_with_no_pages_is_empty_rather_than_an_error() {
        // The normal state on the day the feature ships: coverage is a report,
        // never a failure.
        let dir = tempfile::tempdir().unwrap();
        let catalogue = Catalogue::load_from(Source::Tree(dir.path().to_path_buf())).unwrap();
        assert!(catalogue.is_empty());
    }
}
