//! Grading the skills — on the standard eval manifests they carry, never on
//! whether anyone invoked them.
//!
//! Counting invocations would have graded the busiest lifecycle skill here as
//! dead: `plan-implement` recorded zero, from either record shape, while
//! having written a complete run log. Slash commands log differently from
//! tool calls, so `/cc` and `/pr` were invisible too, and the count is
//! reflexive — implementing the measurement bumps it. A commit that parses, a
//! plan carrying its ADR sections, a run log with a matching `run_start` and
//! `run_end`: durable, in-repo, vendor-neutral, and none of them changes
//! because you looked (LESSON-012).
//!
//! **A low rate is a conversation, not a failure.** Nothing here may produce
//! a finding about a *number*; the only findings are structural — a skill
//! that says nothing about what it produces, or a measure that would not run.
//! `report()` exits 3 on a finding of any severity, so a rate that got
//! interesting would fail `just qc`, and the check would be deleted within a
//! week. Invariants gate commits; measurements start conversations.

use crate::core::doctor::{Finding, Severity};
use std::collections::BTreeMap;

use super::corpus::{self as model, Corpus};
use super::evals;

/// How a skill's conformance came out over one window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Measure {
    /// Every declared eval has a valid standard shape and its input files exist.
    Evals(evals::Summary),
    /// The eval manifest was invalid or unavailable.
    Unmeasured,
}

impl Measure {
    pub fn label(&self) -> String {
        match self {
            Measure::Evals(summary) => summary.label(),
            Measure::Unmeasured => "unmeasured".to_string(),
        }
    }

    fn summary(&self) -> Option<evals::Summary> {
        match self {
            Measure::Evals(summary) => Some(*summary),
            _ => None,
        }
    }
}

#[cfg(test)]
mod eval_integration_tests {
    use super::*;
    use crate::core::harness::corpus::Corpus;
    use crate::core::harness::root::CorpusRoot;

    #[test]
    fn a_valid_standard_manifest_is_reported_as_passing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".ash/plans")).unwrap();
        let skill = dir.path().join(".agents/skills/example");
        std::fs::create_dir_all(skill.join("evals")).unwrap();
        std::fs::write(
            skill.join("SKILL.md"),
            "---\nname: example\ndescription: Example.\n---\n",
        )
        .unwrap();
        std::fs::write(
            skill.join("evals/evals.json"),
            r#"{"skill_name":"example","evals":[{"id":1,"prompt":"p","expected_output":"o"}]}"#,
        )
        .unwrap();

        let corpus = Corpus::load(CorpusRoot::new(dir.path())).unwrap();
        let report = run(&corpus, &Options::default());
        assert_eq!(report.skills.len(), 1);
        assert_eq!(
            report.skills[0].windowed,
            Measure::Evals(evals::Summary {
                passed: 1,
                total: 1
            })
        );
        assert!(report.findings.is_empty());
    }
}

/// One row of the report.
#[derive(Debug, Clone)]
pub struct SkillRow {
    pub name: String,
    /// Whether it is projected into `.claude/skills`. An unwired skill fails
    /// silently — it simply never appears (LESSON-007).
    pub wired: bool,
    pub evals: Option<String>,
    pub windowed: Measure,
    pub all_time: Measure,
    /// How many recorded issues name this skill. Read against the rate: a
    /// skill at 100% conformance with six issues naming it is producing
    /// perfectly-shaped artifacts by a procedure that keeps going wrong, and
    /// it is the most interesting row in the table.
    pub issues: usize,
    /// Invocation counts, only when `--transcripts` asked for them.
    pub invocations: Option<u64>,
    pub last_invoked: Option<String>,
}

impl SkillRow {
    /// True when the recent rate is below the all-time one — the signal that
    /// something changed lately, which an all-time rate would bury.
    fn regressed(&self) -> bool {
        self.windowed
            .summary()
            .map(|summary| summary.passed < summary.total)
            .unwrap_or(false)
    }
}

/// An area of the codebase with recorded issues that no skill owns.
#[derive(Debug, Clone)]
pub struct Gap {
    pub area: String,
    pub issues: usize,
    pub ids: Vec<String>,
}

/// The whole report.
#[derive(Debug, Default)]
pub struct Report {
    pub skills: Vec<SkillRow>,
    pub gaps: Vec<Gap>,
    pub agenda: Vec<String>,
    pub findings: Vec<Finding>,
    /// Whether invocation counts were asked for and could be read.
    pub counted: bool,
}

/// What the caller wants measured.
pub struct Options<'a> {
    /// Validate eval manifests. The manifests are declarative and validation
    /// never executes prompt, assertion, or fixture contents.
    pub run_evals: bool,
    /// A directory of session transcripts, if invocation counts were asked
    /// for. Opt-in, and nothing in `qc` may ever depend on it.
    pub transcripts: Option<&'a std::path::Path>,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            run_evals: true,
            transcripts: None,
        }
    }
}

pub fn run(corpus: &Corpus, options: &Options<'_>) -> Report {
    let mut report = Report::default();
    let skills_dir = corpus.root.skills_dir();

    if !skills_dir.is_dir() {
        report.findings.push(Finding {
            id: "skills.missing".to_string(),
            severity: Severity::Error,
            message: format!("{} does not exist", corpus.relative(&skills_dir)),
            remediation: "run this from a repo that has an .agents/skills directory".to_string(),
            fixable: false,
        });
        return report;
    }

    let counts = match options.transcripts {
        None => None,
        Some(dir) => match super::transcripts::count(dir, corpus) {
            Some(counts) => {
                report.counted = true;
                Some(counts)
            }
            None => {
                report.findings.push(Finding {
                    id: "skills.transcripts-unreadable".to_string(),
                    severity: Severity::Warning,
                    message: format!("could not read invocation counts from {}", dir.display()),
                    remediation: "check the path, or drop --transcripts — nothing else depends \
                                  on it"
                        .to_string(),
                    fixable: false,
                });
                None
            }
        },
    };

    let tally = issue_tally(corpus);
    // Read the same state `pinst harness status`/`install` compute, rather
    // than a second, narrower "does the path exist" check of its own — two
    // implementations of "is this skill wired" is exactly the drift this
    // harness exists to detect.
    let wired: std::collections::BTreeSet<String> = super::install::state::status(
        &super::asset::resolve_source(),
        super::vendor::Vendor::Claude,
        corpus.root.path(),
    )
    .unwrap_or_default()
    .into_iter()
    .filter(|row| row.kind == super::install::state::AssetKindLabel::Skill && row.state.satisfied())
    .map(|row| row.name)
    .collect();

    for dir in skill_dirs(&skills_dir) {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let file = dir.join("SKILL.md");
        if !file.is_file() {
            continue;
        }
        let row = grade(
            corpus,
            &name,
            &file,
            &tally,
            options,
            &mut report.findings,
            wired.contains(&name),
        );
        let row = match counts.as_ref() {
            None => row,
            Some(counts) => {
                let seen = counts.get(&name);
                SkillRow {
                    invocations: Some(seen.map(|s| s.count).unwrap_or(0)),
                    last_invoked: Some(
                        seen.and_then(|s| s.last.clone())
                            .unwrap_or_else(|| "never".to_string()),
                    ),
                    ..row
                }
            }
        };
        report.skills.push(row);
    }

    report.gaps = gaps(corpus);
    report.agenda = agenda(corpus, &report);
    if !report.agenda.is_empty() {
        report.findings.push(Finding {
            id: "review.agenda".to_string(),
            severity: Severity::Info,
            message: format!("{} item(s) on the review agenda", report.agenda.len()),
            remediation: "run /distil, or read the Agenda section above".to_string(),
            fixable: false,
        });
    }
    report
}

fn skill_dirs(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<_> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn grade(
    corpus: &Corpus,
    name: &str,
    file: &std::path::Path,
    tally: &BTreeMap<String, usize>,
    options: &Options<'_>,
    findings: &mut Vec<Finding>,
    wired: bool,
) -> SkillRow {
    let shown = corpus.relative(file);
    let skill_dir = file.parent().unwrap_or_else(|| std::path::Path::new(""));
    let evals_path = skill_dir.join(evals::FILE_NAME);
    let evals = Some(evals_path.display().to_string());

    let (windowed, all_time) = if !options.run_evals {
        (Measure::Unmeasured, Measure::Unmeasured)
    } else {
        match evals::run(skill_dir) {
            Ok(summary) => (Measure::Evals(summary), Measure::Evals(summary)),
            Err(failures) => {
                findings.push(Finding {
                    id: format!("skill.evals-failed.{name}"),
                    severity: Severity::Warning,
                    message: format!(
                        "{} has an invalid standard eval manifest: {}",
                        shown,
                        failures
                            .iter()
                            .map(|failure| failure.message.as_str())
                            .collect::<Vec<_>>()
                            .join("; ")
                    ),
                    remediation: format!("fix {} to match the evals/evals.json contract", shown),
                    fixable: false,
                });
                (Measure::Unmeasured, Measure::Unmeasured)
            }
        }
    };

    SkillRow {
        wired,
        name: name.to_string(),
        evals,
        windowed,
        all_time,
        issues: tally.get(name).copied().unwrap_or(0),
        invocations: None,
        last_invoked: None,
    }
}

/// How many recorded issues name each skill, from the `**Skill:**` line
/// `plan-learnings` writes.
fn issue_tally(corpus: &Corpus) -> BTreeMap<String, usize> {
    let mut tally = BTreeMap::new();
    for plan in &corpus.plans {
        for issue in model::read_issues(&plan.learnings_file()) {
            if let Some(skill) = issue.skill {
                *tally.entry(skill).or_insert(0) += 1;
            }
        }
    }
    tally
}

/// Which work keeps being done with no skill to do it.
///
/// Reported, never enforced: a missing skill is a judgement about what is
/// worth automating, and a finding would mean `qc` failing over an opinion.
/// An issue counts once per area its plan declares, so an area accumulates
/// everything unowned that touched it — the useful reading, at the cost of
/// one issue appearing under two headings.
fn gaps(corpus: &Corpus) -> Vec<Gap> {
    let mut by_area: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for plan in &corpus.plans {
        let Some(readme) = plan.readme.as_ref() else {
            continue;
        };
        let areas: Vec<String> = readme
            .frontmatter
            .as_ref()
            .and_then(|fm| fm.list("areas"))
            .map(|items| items.to_vec())
            .unwrap_or_default();
        for issue in model::read_issues(&plan.learnings_file()) {
            if issue.skill.as_deref() != Some("none") || issue.gap_answered {
                continue;
            }
            for area in &areas {
                by_area
                    .entry(area.clone())
                    .or_default()
                    .push(issue.id.clone());
            }
        }
    }
    let mut gaps: Vec<Gap> = by_area
        .into_iter()
        .map(|(area, ids)| Gap {
            area,
            issues: ids.len(),
            ids,
        })
        .collect();
    // Biggest first: the numbers order the conversation, they do not settle it.
    gaps.sort_by(|a, b| b.issues.cmp(&a.issues).then(a.area.cmp(&b.area)));
    gaps
}

/// The closing list of things someone could actually do.
fn agenda(corpus: &Corpus, report: &Report) -> Vec<String> {
    let mut agenda = Vec::new();

    let untriaged = untriaged_count(corpus);
    if untriaged > 0 {
        agenda.push(format!(
            "{untriaged} issue(s) waiting on a decision — /distil, or pinst harness check to \
             list them"
        ));
    }

    let regressed: Vec<&str> = report
        .skills
        .iter()
        .filter(|row| row.regressed())
        .map(|row| row.name.as_str())
        .collect();
    if !regressed.is_empty() {
        agenda.push(format!(
            "windowed rate below all-time: {} — look at what changed recently",
            regressed.join(", ")
        ));
    }

    if let Some(biggest) = report.gaps.first() {
        agenda.push(format!(
            "{} has {} issue(s) no skill owns — is there a skill missing here?",
            biggest.area, biggest.issues
        ));
    }

    // A lesson two different plans have hit, still enforced by nothing.
    // Listed rather than failed on: "several" is a threshold nobody can
    // justify, and some lessons are judgement no exit code can carry — which
    // is what `**Mechanize:** declined` says, once and permanently.
    let recurring: Vec<String> = model::read_lessons(&corpus.root.learnings_file())
        .into_iter()
        .filter(|lesson| {
            lesson.status.as_deref() == Some("prose")
                && !lesson.mechanize_declined
                && distinct(&lesson.plans) > 1
        })
        .map(|lesson| lesson.id)
        .collect();
    if !recurring.is_empty() {
        agenda.push(format!(
            "recurring and still unenforced: {} — candidates for a check",
            recurring.join(" ")
        ));
    }

    agenda
}

fn distinct(values: &[String]) -> usize {
    values
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

/// Issues nobody has decided about. Computed the same way `check` does, and
/// reported here only as a count — `check` is where each one is named.
fn untriaged_count(corpus: &Corpus) -> usize {
    let lessons = model::read_lessons(&corpus.root.learnings_file());
    let cited: std::collections::BTreeSet<(String, String)> = lessons
        .iter()
        .flat_map(|lesson| {
            lesson.plans.iter().flat_map(move |plan| {
                lesson
                    .issues
                    .iter()
                    .map(move |issue| (plan.clone(), issue.clone()))
            })
        })
        .collect();

    corpus
        .plans
        .iter()
        .filter_map(|plan| plan.id.as_deref().map(|id| (plan, id)))
        .flat_map(|(plan, id)| {
            model::read_issues(&plan.learnings_file())
                .into_iter()
                .map(move |issue| (id.to_string(), issue))
        })
        .filter(|(id, issue)| {
            !issue.distilled_declined && !cited.contains(&(id.clone(), issue.id.clone()))
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::harness::root::CorpusRoot;
    use std::path::Path;

    fn real() -> Corpus {
        Corpus::load(CorpusRoot::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn row<'r>(report: &'r Report, name: &str) -> &'r SkillRow {
        report
            .skills
            .iter()
            .find(|row| row.name == name)
            .unwrap_or_else(|| panic!("no row for {name}"))
    }

    /// Every skill in this repo is wired and declares a usable contract.
    ///
    /// Eval validation is local and deterministic, so this exercises the real
    /// manifests rather than skipping them.
    #[test]
    fn every_skill_in_this_repo_declares_a_contract_and_is_wired() {
        let report = run(
            &real(),
            &Options {
                ..Options::default()
            },
        );
        assert_eq!(report.skills.len(), 7, "expected seven skills");

        for row in &report.skills {
            assert!(row.wired, "{} is not wired into .claude/skills", row.name);
            assert!(
                row.evals.is_some(),
                "{} declares no eval manifest",
                row.name
            );
            assert!(
                !matches!(row.windowed, Measure::Unmeasured),
                "{} has no usable measure: {}",
                row.name,
                row.windowed.label()
            );
        }

        let structural: Vec<&str> = report
            .findings
            .iter()
            .filter(|f| f.id != "review.agenda")
            .map(|f| f.id.as_str())
            .collect();
        assert!(structural.is_empty(), "unexpected findings: {structural:?}");
    }

    /// `pinst` carries the same standard eval contract as every other skill.
    #[test]
    fn the_pinst_skill_has_a_passing_eval() {
        let report = run(&real(), &Options::default());
        let pinst = row(&report, "pinst");
        assert_eq!(
            pinst.windowed,
            Measure::Evals(evals::Summary {
                passed: 1,
                total: 1
            })
        );
        assert_eq!(pinst.all_time, pinst.windowed);
    }

    /// Read the issues column against the rate: a skill at 100% conformance
    /// with a dozen issues naming it is producing perfectly-shaped artifacts
    /// by a procedure that keeps going wrong.
    #[test]
    fn issues_are_tallied_from_the_skill_lines_plans_record() {
        let report = run(
            &real(),
            &Options {
                ..Options::default()
            },
        );
        assert!(row(&report, "plan-write").issues > 0);
        assert!(row(&report, "conventional-commits").issues > 0);
        assert!(row(&report, "orchestrate").issues > 0);
    }

    // --- fixtures -----------------------------------------------------------

    struct Fixture {
        dir: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join(".ash/plans")).unwrap();
            std::fs::create_dir_all(dir.path().join(".claude/skills")).unwrap();
            Self { dir }
        }

        fn path(&self) -> &Path {
            self.dir.path()
        }

        fn skill(&self, name: &str, frontmatter: &str) {
            let dir = self.path().join(".agents/skills").join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: A skill.\n{frontmatter}---\n"),
            )
            .unwrap();
        }

        fn corpus(&self) -> Corpus {
            Corpus::load(CorpusRoot::new(self.path())).unwrap()
        }

        fn report(&self, options: Options<'_>) -> Report {
            run(&self.corpus(), &options)
        }
    }

    fn quick() -> Options<'static> {
        Options {
            ..Options::default()
        }
    }

    /// The structural findings, which are the only ones this report may
    /// produce. A *number* must never become a finding: any finding exits 3,
    /// so an interesting rate would fail `just qc` and the check would be
    /// deleted.
    #[test]
    fn a_skill_with_no_contract_and_one_with_no_measure_are_both_reported() {
        let fixture = Fixture::new();
        fixture.skill("silent", "");
        fixture.skill("unmeasured", "");

        let report = fixture.report(quick());
        let ids: Vec<&str> = report.findings.iter().map(|f| f.id.as_str()).collect();

        assert!(ids.contains(&"skill.evals-failed.silent"), "{ids:?}");
        assert!(ids.contains(&"skill.evals-failed.unmeasured"), "{ids:?}");
        assert_eq!(row(&report, "silent").windowed, Measure::Unmeasured);
        assert_eq!(row(&report, "unmeasured").windowed, Measure::Unmeasured);
    }

    /// A measure that cannot run reports `unmeasured` and never `0`, because
    /// a zero meaning "offline" is worse than a gap that admits it.
    #[test]
    fn a_broken_measure_is_unmeasured_never_zero() {
        let fixture = Fixture::new();
        fixture.skill("one-number", "");
        fixture.skill("exits-nonzero", "");

        let report = fixture.report(quick());
        for name in ["one-number", "exits-nonzero"] {
            assert_eq!(row(&report, name).windowed, Measure::Unmeasured);
            assert_eq!(row(&report, name).windowed.label(), "unmeasured");
            assert!(
                report
                    .findings
                    .iter()
                    .any(|f| f.id == format!("skill.evals-failed.{name}")),
                "{name} should report a failed measure"
            );
        }
    }

    /// Skipping eval validation must not report missing manifests; enabling it
    /// must report the same manifest as invalid.
    #[test]
    fn skipping_evals_is_explicit_and_does_not_execute_anything() {
        let fixture = Fixture::new();
        fixture.skill("missing-evals", "");

        let report = fixture.report(Options {
            run_evals: false,
            ..quick()
        });
        assert!(report.findings.is_empty());
        assert_eq!(row(&report, "missing-evals").windowed, Measure::Unmeasured);

        let report = fixture.report(quick());
        assert_eq!(row(&report, "missing-evals").windowed, Measure::Unmeasured);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.id == "skill.evals-failed.missing-evals")
        );
    }

    /// A missing standard manifest is reported as invalid rather than inferred
    /// from unrelated skill frontmatter.
    #[test]
    fn a_missing_eval_manifest_is_unmeasured() {
        let fixture = Fixture::new();
        fixture.skill("slipping", "");
        let report = fixture.report(quick());

        assert_eq!(row(&report, "slipping").windowed, Measure::Unmeasured);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.id == "skill.evals-failed.slipping")
        );
    }

    /// An issue attributed to no skill is a gap, once per area its plan
    /// declares — and can be told the question has been settled.
    #[test]
    fn gaps_group_unowned_issues_by_area_and_can_be_answered() {
        let fixture = Fixture::new();
        fixture.skill("a-skill", "");
        let plan = fixture.path().join(".ash/plans/260919-qwerty-thing");
        std::fs::create_dir_all(&plan).unwrap();
        std::fs::write(
            plan.join("README.md"),
            "---\nid: 260919-qwerty\nslug: thing\nstatus: Done\ncreated: 2026-09-19\n\
             updated: 2026-09-19\nareas: [ci, distribution]\nsummary: A plan.\n---\n",
        )
        .unwrap();
        std::fs::write(
            plan.join("learnings.md"),
            "# Learnings\n\n### ISSUE-001: A surprise\n**Skill:** none\n\n\
             ### ISSUE-002: Another\n**Skill:** none\n",
        )
        .unwrap();

        let report = fixture.report(quick());
        assert_eq!(report.gaps.len(), 2, "one per declared area");
        assert_eq!(report.gaps[0].issues, 2);
        assert!(
            report
                .agenda
                .iter()
                .any(|item| item.contains("no skill owns")),
            "{:?}",
            report.agenda
        );

        // Answering it retires the question permanently — a recurring report
        // with no way to record the reply decays into noise (LESSON-014).
        std::fs::write(
            plan.join("learnings.md"),
            "# Learnings\n\n### ISSUE-001: A surprise\n**Skill:** none\n\
             **Gap:** answered — a domain surprise, no skill would have helped\n\n\
             ### ISSUE-002: Another\n**Skill:** none\n\
             **Gap:** answered — likewise\n",
        )
        .unwrap();
        assert!(fixture.report(quick()).gaps.is_empty());
    }

    /// `**Mechanize:** declined` is the same shape of answer, for the other
    /// recurring question.
    #[test]
    fn a_recurring_prose_lesson_is_listed_until_mechanizing_is_declined() {
        let fixture = Fixture::new();
        fixture.skill("a-skill", "");
        let lesson = |extra: &str| {
            format!(
                "# Distilled learnings\n\n### LESSON-001: A thing\n**Status:** prose\n{extra}\
                 **Seen in:** 260918-aaaaaa-one (ISSUE-001); again in\n\
                 260919-bbbbbb-two (ISSUE-002)\n"
            )
        };
        std::fs::write(fixture.path().join(".ash/LEARNINGS.md"), lesson("")).unwrap();
        assert!(
            fixture
                .report(quick())
                .agenda
                .iter()
                .any(|item| item.contains("recurring and still unenforced")),
        );

        std::fs::write(
            fixture.path().join(".ash/LEARNINGS.md"),
            lesson("**Mechanize:** declined — judgement no exit code can carry\n"),
        )
        .unwrap();
        assert!(
            !fixture
                .report(quick())
                .agenda
                .iter()
                .any(|item| item.contains("recurring and still unenforced")),
        );
    }

    #[test]
    fn a_missing_skills_directory_is_a_finding_rather_than_an_empty_report() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".ash/plans")).unwrap();
        let corpus = Corpus::load(CorpusRoot::new(dir.path())).unwrap();
        let report = run(&corpus, &quick());
        assert_eq!(report.findings[0].id, "skills.missing");
        assert!(report.skills.is_empty());
    }
}
