//! Doctor: the join of tool state and config state, expressed as findings an
//! agent can act on.
//!
//! Every finding carries a stable `id`, a severity, a human `message`, and the
//! exact `remediation` command — the difference between a report that is
//! merely readable and one that is actionable.

use std::collections::BTreeMap;

use color_eyre::eyre::Result;
use schemars::JsonSchema;
use serde::Serialize;

use super::configs::{ConfigSet, FileState};
use super::exec;
use super::manifest::{Install, Tool};
use super::probe::ProbeResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Finding {
    /// Stable across runs and releases, e.g. `tool.missing.zsh`. Safe for an
    /// agent to match on.
    pub id: String,
    pub severity: Severity,
    pub message: String,
    /// The command that resolves this finding.
    pub remediation: String,
    /// Whether `pinst doctor --fix` will act on it. Anything needing a human
    /// decision (which side of a drift wins) stays false on purpose.
    pub fixable: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema, Default)]
pub struct DoctorSummary {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub fixable: usize,
}

pub fn summarize(findings: &[Finding]) -> DoctorSummary {
    let mut summary = DoctorSummary::default();
    for finding in findings {
        match finding.severity {
            Severity::Error => summary.errors += 1,
            Severity::Warning => summary.warnings += 1,
            Severity::Info => summary.infos += 1,
        }
        if finding.fixable {
            summary.fixable += 1;
        }
    }
    summary
}

/// Runs every check and returns the findings, most severe first.
pub fn diagnose(
    tools: &[&Tool],
    probes: &BTreeMap<String, ProbeResult>,
    configs: &ConfigSet,
) -> Result<Vec<Finding>> {
    let mut findings = Vec::new();

    check_tools(tools, probes, &mut findings);
    check_configs(configs, &mut findings)?;
    check_shell(&mut findings);

    findings.sort_by(|a, b| a.severity.cmp(&b.severity).then(a.id.cmp(&b.id)));
    Ok(findings)
}

fn check_tools(tools: &[&Tool], probes: &BTreeMap<String, ProbeResult>, out: &mut Vec<Finding>) {
    for tool in tools {
        let installed = probes.get(&tool.name).map(|p| p.installed).unwrap_or(false);

        if !installed {
            match &tool.install {
                Install::Manual { note } => out.push(Finding {
                    id: format!("tool.manual.{}", tool.name),
                    severity: Severity::Warning,
                    message: format!("{} is not installed and has no automated install", tool.name),
                    remediation: note.clone(),
                    fixable: false,
                }),
                _ => out.push(Finding {
                    id: format!("tool.missing.{}", tool.name),
                    severity: Severity::Error,
                    message: format!("{} is not installed", tool.name),
                    remediation: format!("pinst install {}", tool.name),
                    fixable: true,
                }),
            }
            continue;
        }

        // Installed, but pinst cannot verify what it got: a piped installer
        // has no version pin and no signature to check (RISK-006).
        if tool.is_unpinnable() {
            out.push(Finding {
                id: format!("tool.unpinnable.{}", tool.name),
                severity: Severity::Info,
                message: format!(
                    "{} was installed from an unpinned remote script; its version cannot be verified",
                    tool.name
                ),
                remediation: format!("pinst update {} to re-run the installer", tool.name),
                fixable: false,
            });
        }
    }
}

fn check_configs(configs: &ConfigSet, out: &mut Vec<Finding>) -> Result<()> {
    for file in &configs.files {
        let state = configs.classify(file)?;
        let path = file.relative.display();
        match state {
            FileState::Linked | FileState::Materialized => {}
            FileState::Missing => out.push(Finding {
                id: format!("config.missing.{path}"),
                severity: Severity::Error,
                message: format!("~/{path} is not installed"),
                remediation: "pinst config apply".to_string(),
                fixable: true,
            }),
            FileState::Foreign => out.push(Finding {
                id: format!("config.foreign.{path}"),
                severity: Severity::Warning,
                message: format!(
                    "~/{path} is a symlink into another tree (pre-cutover ~/dotfiles, most likely)"
                ),
                remediation: "pinst config apply".to_string(),
                fixable: true,
            }),
            // A templated file whose values are missing is a finding, not a
            // crash: that is exactly the state a fresh machine is in.
            FileState::Unrenderable => out.push(Finding {
                id: format!("config.template.{path}"),
                severity: Severity::Error,
                message: format!(
                    "~/{path} cannot be rendered: {}",
                    configs
                        .render_error(file)
                        .unwrap_or_else(|| "missing values".to_string())
                ),
                remediation: format!(
                    "set the missing values in {}",
                    super::template::Values::path_hint()
                ),
                fixable: false,
            }),
            FileState::Drifted => out.push(Finding {
                id: format!("config.drifted.{path}"),
                severity: Severity::Warning,
                message: format!("~/{path} was edited in place and differs from what pinst carries"),
                // Not auto-fixable: only a human knows whether the edit should
                // win (adopt) or be discarded (apply).
                remediation: "pinst config adopt (keep the edit) or pinst config apply (discard it)"
                    .to_string(),
                fixable: false,
            }),
        }
    }
    Ok(())
}

fn check_shell(out: &mut Vec<Finding>) {
    // The configs assume zsh is the login shell; if it is not, none of them
    // are actually in effect.
    if exec::check("command -v zsh") && !exec::check("[ \"${SHELL##*/}\" = zsh ]") {
        out.push(Finding {
            id: "shell.not-zsh".to_string(),
            severity: Severity::Warning,
            message: "zsh is installed but is not the login shell, so these configs are inert"
                .to_string(),
            remediation: "chsh -s \"$(command -v zsh)\"".to_string(),
            fixable: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::graph::{Selection, select};
    use crate::core::manifest;

    fn probe_map(tools: &[&Tool], installed: &[&str]) -> BTreeMap<String, ProbeResult> {
        tools
            .iter()
            .map(|tool| {
                (
                    tool.name.clone(),
                    ProbeResult {
                        tool: tool.name.clone(),
                        installed: installed.contains(&tool.name.as_str()),
                        version: None,
                        path: None,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn missing_tools_are_fixable_errors_and_manual_ones_are_not() {
        let manifest = manifest::embedded().unwrap();
        let tools = select(&manifest, &Selection::default()).unwrap();
        let mut findings = Vec::new();
        check_tools(&tools, &probe_map(&tools, &[]), &mut findings);

        let zsh = findings.iter().find(|f| f.id == "tool.missing.zsh").unwrap();
        assert_eq!(zsh.severity, Severity::Error);
        assert!(zsh.fixable);
        assert_eq!(zsh.remediation, "pinst install zsh");

        let gcm = findings
            .iter()
            .find(|f| f.id == "tool.manual.git-credential-manager")
            .unwrap();
        assert_eq!(gcm.severity, Severity::Warning);
        assert!(!gcm.fixable, "a manual install cannot be auto-fixed");
    }

    #[test]
    fn unpinnable_installers_are_reported_when_installed() {
        let manifest = manifest::embedded().unwrap();
        let tools = select(
            &manifest,
            &Selection {
                names: vec!["claude".to_string()],
                ..Default::default()
            },
        )
        .unwrap();
        let mut findings = Vec::new();
        check_tools(&tools, &probe_map(&tools, &["claude", "curl"]), &mut findings);

        assert!(findings.iter().any(|f| f.id == "tool.unpinnable.claude"));
        assert!(
            findings
                .iter()
                .all(|f| f.severity != Severity::Error),
            "an unverifiable install is informational, not a failure"
        );
    }

    #[test]
    fn summary_counts_by_severity_and_fixability() {
        let findings = vec![
            Finding {
                id: "a".into(),
                severity: Severity::Error,
                message: String::new(),
                remediation: String::new(),
                fixable: true,
            },
            Finding {
                id: "b".into(),
                severity: Severity::Warning,
                message: String::new(),
                remediation: String::new(),
                fixable: false,
            },
            Finding {
                id: "c".into(),
                severity: Severity::Info,
                message: String::new(),
                remediation: String::new(),
                fixable: false,
            },
        ];
        let summary = summarize(&findings);
        assert_eq!(summary.errors, 1);
        assert_eq!(summary.warnings, 1);
        assert_eq!(summary.infos, 1);
        assert_eq!(summary.fixable, 1);
    }
}
