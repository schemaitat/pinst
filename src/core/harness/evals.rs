//! The standard Agent Skills eval manifest.
//!
//! Evals are declarative JSON in `<skill>/evals/evals.json`.  They describe
//! prompts and observable expectations; they are not commands.  This parser
//! therefore validates the authoring contract without executing prompt text,
//! assertion text, or files supplied by an eval.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The file name prescribed by the Agent Skills evaluation convention.
pub const FILE_NAME: &str = "evals/evals.json";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Suite {
    pub skill_name: String,
    pub evals: Vec<Eval>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Eval {
    pub id: u64,
    pub prompt: String,
    pub expected_output: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub assertions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub eval_id: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub passed: usize,
    pub total: usize,
}

impl Summary {
    pub fn label(self) -> String {
        format!("{}/{} passed", self.passed, self.total)
    }
}

/// Read and validate one skill's standard eval manifest.
///
/// A missing manifest is distinct from an invalid one.  That distinction lets
/// the skills report tell authors whether they need to add evals or fix them.
pub fn run(skill_dir: &Path) -> Result<Summary, Vec<Failure>> {
    let path = skill_dir.join(FILE_NAME);
    let text = std::fs::read_to_string(&path).map_err(|error| {
        vec![Failure {
            eval_id: None,
            message: format!("{}: {error}", path.display()),
        }]
    })?;
    let suite: Suite = serde_json::from_str(&text).map_err(|error| {
        vec![Failure {
            eval_id: None,
            message: format!("{}: invalid evals.json: {error}", path.display()),
        }]
    })?;

    let mut failures = Vec::new();
    if suite.skill_name != skill_dir.file_name().unwrap_or_default().to_string_lossy() {
        failures.push(Failure {
            eval_id: None,
            message: format!(
                "skill_name '{}' does not match '{}'",
                suite.skill_name,
                skill_dir.file_name().unwrap_or_default().to_string_lossy()
            ),
        });
    }
    if suite.evals.is_empty() {
        failures.push(Failure {
            eval_id: None,
            message: "evals must contain at least one test case".to_string(),
        });
    }

    let mut ids = BTreeSet::new();
    for eval in &suite.evals {
        let invalid = |message: String| Failure {
            eval_id: Some(eval.id),
            message,
        };
        if !ids.insert(eval.id) {
            failures.push(invalid(format!("duplicate eval id {}", eval.id)));
        }
        if eval.prompt.trim().is_empty() {
            failures.push(invalid("prompt must not be empty".to_string()));
        }
        if eval.expected_output.trim().is_empty() {
            failures.push(invalid("expected_output must not be empty".to_string()));
        }
        for file in &eval.files {
            let relative = PathBuf::from(file);
            if relative.is_absolute()
                || relative
                    .components()
                    .any(|c| c == std::path::Component::ParentDir)
            {
                failures.push(invalid(format!(
                    "file path '{file}' must stay inside the skill"
                )));
            } else if !skill_dir.join(&relative).is_file() {
                failures.push(invalid(format!("input file '{file}' does not exist")));
            }
        }
        if eval
            .assertions
            .iter()
            .any(|assertion| assertion.trim().is_empty())
        {
            failures.push(invalid(
                "assertions must not contain empty strings".to_string(),
            ));
        }
    }

    if failures.is_empty() {
        Ok(Summary {
            passed: suite.evals.len(),
            total: suite.evals.len(),
        })
    } else {
        Err(failures)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_standard_manifest_is_a_real_passing_eval() {
        let dir = tempfile::tempdir().unwrap();
        let skill = dir.path().join("example");
        std::fs::create_dir_all(skill.join("evals/files")).unwrap();
        std::fs::write(skill.join("evals/files/input.txt"), "fixture").unwrap();
        std::fs::write(
            skill.join(FILE_NAME),
            r#"{
              "skill_name": "example",
              "evals": [{
                "id": 1,
                "prompt": "Use the skill.",
                "expected_output": "A useful result.",
                "files": ["evals/files/input.txt"],
                "assertions": ["The result is useful."]
              }]
            }"#,
        )
        .unwrap();

        assert_eq!(
            run(&skill),
            Ok(Summary {
                passed: 1,
                total: 1
            })
        );
    }

    #[test]
    fn evals_never_accept_paths_outside_the_skill() {
        let dir = tempfile::tempdir().unwrap();
        let skill = dir.path().join("example");
        std::fs::create_dir_all(skill.join("evals")).unwrap();
        std::fs::write(
            skill.join(FILE_NAME),
            r#"{"skill_name":"example","evals":[{"id":1,"prompt":"p","expected_output":"o","files":["../secret"]}]}"#,
        )
        .unwrap();

        assert!(
            run(&skill)
                .unwrap_err()
                .iter()
                .any(|failure| { failure.message.contains("must stay inside") })
        );
    }
}
