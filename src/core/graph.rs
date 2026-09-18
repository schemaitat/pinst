//! Dependency ordering and selection over the manifest's tool set.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use color_eyre::eyre::{Result, bail};

use super::manifest::{Manifest, Tool};

/// Returns tool names in dependency order (every tool after everything it
/// requires). Ties break alphabetically so the order is deterministic — a plan
/// an agent diffs between runs should not shuffle.
pub fn topo_order(tools: &[Tool]) -> Result<Vec<String>> {
    let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut indegree: BTreeMap<&str, usize> = BTreeMap::new();

    for tool in tools {
        indegree.entry(tool.name.as_str()).or_insert(0);
    }
    for tool in tools {
        for dep in &tool.requires {
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
            let deg = indegree.get_mut(*dependent).expect("dependent is a known tool");
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
        bail!("dependency cycle among tools: {}", stuck.join(", "));
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
pub fn select<'m>(manifest: &'m Manifest, selection: &Selection) -> Result<Vec<&'m Tool>> {
    let mut wanted: BTreeSet<String> = BTreeSet::new();

    if let Some(profile_name) = &selection.profile {
        let Some(profile) = manifest.profile.get(profile_name) else {
            bail!(
                "unknown profile '{}' (known: {})",
                profile_name,
                manifest
                    .profile
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
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
            bail!("no tool carries tag '{tag}'");
        }
        for tool in matched {
            wanted.insert(tool.name.clone());
        }
    }

    for name in &selection.names {
        if manifest.tool(name).is_none() {
            bail!("unknown tool '{name}'");
        }
        wanted.insert(name.clone());
    }

    if selection.is_empty() {
        wanted.extend(manifest.tools.iter().map(|t| t.name.clone()));
    }

    // Pull in the transitive requires closure.
    let mut queue: VecDeque<String> = wanted.iter().cloned().collect();
    while let Some(name) = queue.pop_front() {
        let Some(tool) = manifest.tool(&name) else {
            continue;
        };
        for dep in &tool.requires {
            if wanted.insert(dep.clone()) {
                queue.push_back(dep.clone());
            }
        }
    }

    let order = topo_order(&manifest.tools)?;
    Ok(order
        .into_iter()
        .filter(|name| wanted.contains(name))
        .filter_map(|name| manifest.tool(&name))
        .collect())
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
        let selected: Vec<&str> = select(&manifest, &selection)
            .unwrap()
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
        let selected: Vec<&str> = select(&manifest, &selection)
            .unwrap()
            .iter()
            .map(|t| t.name.as_str())
            .collect();

        assert!(selected.contains(&"zsh"));
        assert!(!selected.contains(&"neovim"), "editor tag is not in minimal");
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
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown profile"), "{err}");

        let err = select(
            &manifest,
            &Selection {
                names: vec!["nope".into()],
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown tool"), "{err}");
    }
}
