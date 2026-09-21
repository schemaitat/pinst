//! Dependency ordering and selection over the manifest's tool set.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use color_eyre::eyre::Result;

use crate::core::platform::Platform;
use crate::core::usage;

use super::manifest::{Manifest, Resolved, Tool};

/// A tool that was in scope for the selection but that `platform` cannot
/// install — `note` is why, exactly as [`crate::core::manifest::Tool::resolve`]
/// produced it. Carried separately from the selected tools rather than
/// silently dropped, so a caller (`pinst doctor`, in particular) can still
/// report it.
#[derive(Debug, Clone)]
pub struct UnsupportedTool {
    pub name: String,
    pub note: String,
}

/// The result of narrowing the manifest for one platform: the tools to plan
/// for, dependency-ordered, and the ones that were in scope but that the
/// platform cannot install.
#[derive(Debug, Clone, Default)]
pub struct Selected<'m> {
    pub tools: Vec<&'m Tool>,
    pub unsupported: Vec<UnsupportedTool>,
}

/// Returns tool names in dependency order (every tool after everything it
/// requires). Ties break alphabetically so the order is deterministic — a plan
/// an agent diffs between runs should not shuffle.
///
/// Orders on each tool's *base* `requires` — the edges every platform
/// agrees on. Used for [`Manifest::validate`]'s cycle check and by callers
/// that have no platform in hand; a plan that must respect a
/// platform-specific edge (`ripgrep` on macOS requiring `homebrew`, which
/// `ripgrep`'s base definition says nothing about) needs
/// [`topo_order_for`] instead.
pub fn topo_order(tools: &[Tool]) -> Result<Vec<String>> {
    topo_order_with(tools, |tool| tool.requires.as_slice())
}

/// [`topo_order`], but ordering on each tool's *effective* `requires` for
/// `platform` — the edges a `[tool.platform.<key>]` override adds or
/// changes. An unsupported tool contributes its base `requires`, matching
/// `select`'s own closure walk: the tool itself is dropped from the plan,
/// but its position among tools that still depend on it must stay
/// well-defined.
pub fn topo_order_for(tools: &[Tool], platform: Platform) -> Result<Vec<String>> {
    topo_order_with(tools, move |tool| match tool.resolve(platform) {
        Resolved::Supported(effective) => effective.requires,
        Resolved::Unsupported(_) => tool.requires.as_slice(),
    })
}

fn topo_order_with<'a>(
    tools: &'a [Tool],
    requires_of: impl Fn(&'a Tool) -> &'a [String],
) -> Result<Vec<String>> {
    let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut indegree: BTreeMap<&str, usize> = BTreeMap::new();

    for tool in tools {
        indegree.entry(tool.name.as_str()).or_insert(0);
    }
    for tool in tools {
        for dep in requires_of(tool) {
            dependents.entry(dep.as_str()).or_default().push(&tool.name);
            *indegree.entry(tool.name.as_str()).or_insert(0) += 1;
        }
    }

    let mut ready: VecDeque<&str> = indegree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(name, _)| *name)
        .collect();

    let mut ordered = Vec::with_capacity(tools.len());
    while let Some(name) = ready.pop_front() {
        ordered.push(name.to_string());
        for dependent in dependents.get(name).into_iter().flatten() {
            let deg = indegree
                .get_mut(*dependent)
                .expect("dependent is a known tool");
            *deg -= 1;
            if *deg == 0 {
                ready.push_back(dependent);
            }
        }
    }

    if ordered.len() != tools.len() {
        let stuck: Vec<&str> = indegree
            .iter()
            .filter(|(_, deg)| **deg > 0)
            .map(|(name, _)| *name)
            .collect();
        return Err(usage(format!(
            "dependency cycle among tools: {}",
            stuck.join(", ")
        )));
    }

    Ok(ordered)
}

/// How a caller narrowed the tool set.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub profile: Option<String>,
    pub tags: Vec<String>,
    pub names: Vec<String>,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.profile.is_none() && self.tags.is_empty() && self.names.is_empty()
    }
}

/// Resolves a selection into a dependency-ordered list of tools.
///
/// Explicitly named tools pull in their `requires` closure — asking for
/// `node` without `nvm` would otherwise produce a plan that cannot succeed.
pub fn select<'m>(
    manifest: &'m Manifest,
    selection: &Selection,
    platform: Platform,
) -> Result<Selected<'m>> {
    let mut wanted: BTreeSet<String> = BTreeSet::new();

    if let Some(profile_name) = &selection.profile {
        let Some(profile) = manifest.profile.get(profile_name) else {
            return Err(usage(format!(
                "unknown profile '{}' (known: {})",
                profile_name,
                manifest
                    .profile
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        };
        for tool in &manifest.tools {
            if tool.tags.iter().any(|t| profile.tags.contains(t)) {
                wanted.insert(tool.name.clone());
            }
        }
    }

    for tag in &selection.tags {
        let matched: Vec<&Tool> = manifest
            .tools
            .iter()
            .filter(|t| t.tags.contains(tag))
            .collect();
        if matched.is_empty() {
            return Err(usage(format!("no tool carries tag '{tag}'")));
        }
        for tool in matched {
            wanted.insert(tool.name.clone());
        }
    }

    for name in &selection.names {
        if manifest.tool(name).is_none() {
            return Err(usage(format!("unknown tool '{name}'")));
        }
        wanted.insert(name.clone());
    }

    if selection.is_empty() {
        wanted.extend(manifest.tools.iter().map(|t| t.name.clone()));
    }

    // Pull in the transitive requires closure. An unsupported tool's own
    // `requires` edges are still walked here — its dependents stay wanted
    // even though the tool itself will be dropped below, and one tool
    // failing honestly at install time (RISK, engine.rs's existing
    // contract) is preferable to silently narrowing what a dependent
    // chain asked for.
    let mut queue: VecDeque<String> = wanted.iter().cloned().collect();
    while let Some(name) = queue.pop_front() {
        let Some(tool) = manifest.tool(&name) else {
            continue;
        };
        let requires: &[String] = match tool.resolve(platform) {
            Resolved::Supported(effective) => effective.requires,
            Resolved::Unsupported(_) => &tool.requires,
        };
        for dep in requires {
            if wanted.insert(dep.clone()) {
                queue.push_back(dep.clone());
            }
        }
    }

    let order = topo_order_for(&manifest.tools, platform)?;
    let mut selected = Selected::default();
    for name in order {
        if !wanted.contains(&name) {
            continue;
        }
        let Some(tool) = manifest.tool(&name) else {
            continue;
        };
        match tool.resolve(platform) {
            Resolved::Supported(_) => selected.tools.push(tool),
            Resolved::Unsupported(note) => selected.unsupported.push(UnsupportedTool {
                name: tool.name.clone(),
                note,
            }),
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::manifest;

    fn position(order: &[String], name: &str) -> usize {
        order
            .iter()
            .position(|n| n == name)
            .unwrap_or_else(|| panic!("{name} missing from order"))
    }

    #[test]
    fn orders_real_manifest_dependencies() {
        let manifest = manifest::embedded().unwrap();
        let order = topo_order(&manifest.tools).unwrap();

        assert!(position(&order, "zsh") < position(&order, "oh-my-zsh"));
        assert!(position(&order, "oh-my-zsh") < position(&order, "zsh-autosuggestions"));
        assert!(position(&order, "rust") < position(&order, "tree-sitter-cli"));
        assert!(position(&order, "nvm") < position(&order, "node"));
        assert!(position(&order, "rust") < position(&order, "just"));
        assert!(position(&order, "curl") < position(&order, "uv"));
    }

    #[test]
    fn detects_cycles() {
        let src = r#"
            [meta]
            schema_version = 1
            [[tool]]
            name = "a"
            requires = ["b"]
            detect = { command = "true" }
            install = { method = "apt", packages = ["a"] }
            [[tool]]
            name = "b"
            requires = ["a"]
            detect = { command = "true" }
            install = { method = "apt", packages = ["b"] }
        "#;
        let manifest: Manifest = toml::from_str(src).unwrap();
        let err = topo_order(&manifest.tools).unwrap_err();
        assert!(err.to_string().contains("cycle"), "{err}");
    }

    #[test]
    fn selecting_a_tool_pulls_in_its_dependencies() {
        let manifest = manifest::embedded().unwrap();
        let selection = Selection {
            names: vec!["node".to_string()],
            ..Default::default()
        };
        let selected: Vec<&str> = select(&manifest, &selection, Platform::Linux)
            .unwrap()
            .tools
            .iter()
            .map(|t| t.name.as_str())
            .collect();

        assert!(selected.contains(&"node"));
        assert!(selected.contains(&"nvm"), "nvm is required by node");
        assert!(selected.contains(&"curl"), "curl is required by nvm");
        assert!(
            selected.iter().position(|n| *n == "nvm") < selected.iter().position(|n| *n == "node")
        );
    }

    #[test]
    fn profile_narrows_the_set() {
        let manifest = manifest::embedded().unwrap();
        let selection = Selection {
            profile: Some("minimal".to_string()),
            ..Default::default()
        };
        let selected: Vec<&str> = select(&manifest, &selection, Platform::Linux)
            .unwrap()
            .tools
            .iter()
            .map(|t| t.name.as_str())
            .collect();

        assert!(selected.contains(&"zsh"));
        assert!(
            !selected.contains(&"neovim"),
            "editor tag is not in minimal"
        );
    }

    #[test]
    fn rejects_unknown_profile_and_tool() {
        let manifest = manifest::embedded().unwrap();
        let err = select(
            &manifest,
            &Selection {
                profile: Some("nope".into()),
                ..Default::default()
            },
            Platform::Linux,
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown profile"), "{err}");

        let err = select(
            &manifest,
            &Selection {
                names: vec!["nope".into()],
                ..Default::default()
            },
            Platform::Linux,
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown tool"), "{err}");
    }

    // TEST-004: an apt-only tool set plus one entry whose macOS override
    // changes only `post_install` (not `install`) — proving per-field
    // substitution and unsupported-filtering together, on a fixture rather
    // than the real manifest, since Phase 1 introduces no macOS-capable
    // install method for the real apt tools to switch to yet.
    #[test]
    fn platform_selection_drops_unsupported_apt_tools_and_keeps_field_overrides() {
        let manifest: Manifest = toml::from_str(
            r#"
            [meta]
            schema_version = 1

            [[tool]]
            name = "ripgrep"
            detect = { command = "command -v rg" }
            install = { method = "apt", packages = ["ripgrep"] }

            [[tool]]
            name = "direnv"
            detect = { command = "command -v direnv" }
            install = { method = "apt", packages = ["direnv"] }

            [[tool]]
            name = "fd"
            detect = { command = "command -v fd" }
            install = { method = "apt", packages = ["fd-find"] }
            post_install = [
              { description = "symlink fdfind to fd", command = "ln -sf fdfind fd" },
            ]

              [tool.platform.macos]
              post_install = []
            "#,
        )
        .unwrap();
        manifest.validate().unwrap();

        let selected = select(&manifest, &Selection::default(), Platform::MacOS).unwrap();

        let names: Vec<&str> = selected.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["fd"],
            "the two apt-only tools have no macos override and are dropped"
        );

        let unsupported: Vec<&str> = selected
            .unsupported
            .iter()
            .map(|u| u.name.as_str())
            .collect();
        assert!(unsupported.contains(&"ripgrep"));
        assert!(unsupported.contains(&"direnv"));

        let fd = selected.tools[0];
        let Resolved::Supported(effective) = fd.resolve(Platform::MacOS) else {
            panic!("fd has a macos override and resolves as supported");
        };
        assert!(
            effective.post_install.is_empty(),
            "post_install was overridden to empty"
        );
        // install was not overridden, so it is inherited unchanged — this
        // fixture is about proving the substitution mechanism, not about
        // fd actually being installable via apt on macOS.
        assert!(matches!(
            effective.install,
            crate::core::manifest::Install::Apt { .. }
        ));
    }
}
