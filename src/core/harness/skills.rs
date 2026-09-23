//! Grading the skills — on the artifacts they leave in the repo, never on
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

use std::collections::BTreeMap;
use std::time::Duration;

use crate::core::doctor::{Finding, Severity};

use super::corpus::{self as model, Corpus};
use super::evidence::{self, Rate};
use super::frontmatter;

/// How a skill's conformance came out over one window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Measure {
    /// The measure ran and printed two integers.
    Rate(Rate),
    /// A skill that produces no artifact — `pinst` is the only one, and that
    /// exemption is written down rather than inferred from silence.
    Exempt,
    /// The all-time cell of an exempt skill. Repeating "reference (exempt)"
    /// in both columns says nothing the first one did not.
    NotApplicable,
    /// No `produces:` at all.
    NoContract,
    /// `produces:` but no `evidence:` command to measure it with.
    NoCommand,
    /// The measure would not run, or printed something else. Never `0`: a
    /// zero meaning "offline" is worse than a gap that admits it.
    Unmeasured,
}

impl Measure {
    pub fn label(&self) -> String {
        match self {
            Measure::Rate(rate) => rate.label(),
            Measure::Exempt => "reference (exempt)".to_string(),
            Measure::NotApplicable => "-".to_string(),
            Measure::NoContract => "no contract".to_string(),
            Measure::NoCommand => "no measure".to_string(),
            Measure::Unmeasured => "unmeasured".to_string(),
        }
    }

    fn rate(&self) -> Option<Rate> {
        match self {
            Measure::Rate(rate) => Some(*rate),
            _ => None,
        }
    }
}

/// One row of the report.
#[derive(Debug, Clone)]
pub struct SkillRow {
    pub name: String,
    /// Whether it is projected into `.claude/skills`. An unwired skill fails
    /// silently — it simply never appears (LESSON-007).
    pub wired: bool,
    pub produces: Option<String>,
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
        match (self.windowed.rate(), self.all_time.rate()) {
            (Some(window), Some(all)) => match (window.percent(), all.percent()) {
                (Some(w), Some(a)) => w < a,
                _ => false,
            },
            _ => false,
        }
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
    /// Off skips every `evidence:` subprocess. See `evidence`'s header for
    /// why this exists and why it is not the default.
    pub run_evidence: bool,
    /// A directory of session transcripts, if invocation counts were asked
    /// for. Opt-in, and nothing in `qc` may ever depend on it.
    pub transcripts: Option<&'a std::path::Path>,
    pub timeout: Duration,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            run_evidence: true,
            transcripts: None,
            timeout: evidence::TIMEOUT,
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
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let fm = frontmatter::parse(&text).ok().flatten();
    let field = |key: &str| {
        fm.as_ref()
            .and_then(|f| f.text(key))
            .filter(|v| !v.is_empty())
    };

    let produces = field("produces");
    let evidence_command = field("evidence");
    let kind = field("kind");
    let shown = corpus.relative(file);

    let (windowed, all_time) = match (&produces, &kind) {
        (None, _) => {
            findings.push(Finding {
                id: format!("skill.no-contract.{name}"),
                severity: Severity::Warning,
                message: format!(
                    "{shown} declares no 'produces:' — nothing says what this skill is \
                     supposed to leave behind"
                ),
                remediation: "add 'produces:' and 'evidence:' to its frontmatter, or \
                              'kind: reference' if it produces nothing"
                    .to_string(),
                fixable: false,
            });
            (Measure::NoContract, Measure::NoContract)
        }
        (Some(p), _) if p == "none" => (Measure::Exempt, Measure::NotApplicable),
        (_, Some(k)) if k == "reference" => (Measure::Exempt, Measure::NotApplicable),
        (Some(_), _) => match &evidence_command {
            None => {
                findings.push(Finding {
                    id: format!("skill.no-contract.{name}"),
                    severity: Severity::Warning,
                    message: format!(
                        "{shown} says what it produces but gives no 'evidence:' command to \
                         measure it"
                    ),
                    remediation: "add an 'evidence:' one-liner printing '<conforming> <total>'"
                        .to_string(),
                    fixable: false,
                });
                (Measure::NoCommand, Measure::NoCommand)
            }
            Some(command) if !options.run_evidence => {
                let _ = command;
                (Measure::Unmeasured, Measure::Unmeasured)
            }
            Some(command) => {
                let dir = corpus.root.path();
                let windowed = evidence::run(command, dir, Some(evidence::WINDOW), options.timeout);
                let all = evidence::run(command, dir, None, options.timeout);
                if windowed.is_none() || all.is_none() {
                    findings.push(Finding {
                        id: format!("skill.evidence-failed.{name}"),
                        severity: Severity::Warning,
                        message: format!(
                            "the 'evidence:' command for {name} did not run, or printed \
                             something other than two integers"
                        ),
                        remediation: format!(
                            "run the 'evidence:' one-liner in {shown} by hand and see what it \
                             prints"
                        ),
                        fixable: false,
                    });
                }
                (
                    windowed.map(Measure::Rate).unwrap_or(Measure::Unmeasured),
                    all.map(Measure::Rate).unwrap_or(Measure::Unmeasured),
                )
            }
        },
    };

    SkillRow {
        wired,
        name: name.to_string(),
        produces,
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
    /// Deliberately `run_evidence: false`. What is asserted here is that each
    /// skill *declares* a measure, which is a property of the repo; whether
    /// the measure can run is a property of the machine. `create-pr` measures
    /// itself with `gh pr list`, so on any unauthenticated or offline box —
    /// CI, for one — it correctly degrades to `unmeasured` and reports
    /// `skill.evidence-failed`. Asserting no findings while running the
    /// measures made this test claim the network was part of the contract.
    #[test]
    fn every_skill_in_this_repo_declares_a_contract_and_is_wired() {
        let report = run(
            &real(),
            &Options {
                run_evidence: false,
                ..Options::default()
            },
        );
        assert_eq!(report.skills.len(), 8, "expected eight skills");

        for row in &report.skills {
            assert!(row.wired, "{} is not wired into .claude/skills", row.name);
            assert!(
                row.produces.is_some(),
                "{} declares no 'produces:'",
                row.name
            );
            assert!(
                !matches!(row.windowed, Measure::NoContract | Measure::NoCommand),
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

    /// `pinst` is the one skill that produces no artifact, and the exemption
    /// is written down in its frontmatter rather than inferred from silence.
    #[test]
    fn the_reference_skill_is_exempt_rather_than_zero() {
        // Exemption is decided from frontmatter alone, so this needs no
        // subprocess — and running them would drag `gh` and the network into
        // a test about a YAML field.
        let report = run(
            &real(),
            &Options {
                run_evidence: false,
                ..Options::default()
            },
        );
        let pinst = row(&report, "pinst");
        assert_eq!(pinst.windowed, Measure::Exempt);
        assert_eq!(pinst.all_time, Measure::NotApplicable);
        assert_eq!(pinst.all_time.label(), "-");
    }

    /// Read the issues column against the rate: a skill at 100% conformance
    /// with a dozen issues naming it is producing perfectly-shaped artifacts
    /// by a procedure that keeps going wrong.
    #[test]
    fn issues_are_tallied_from_the_skill_lines_plans_record() {
        let report = run(
            &real(),
            &Options {
                run_evidence: false,
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
            timeout: Duration::from_secs(5),
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
        fixture.skill("unmeasured", "produces: 'a thing'\n");

        let report = fixture.report(quick());
        let ids: Vec<&str> = report.findings.iter().map(|f| f.id.as_str()).collect();

        assert!(ids.contains(&"skill.no-contract.silent"), "{ids:?}");
        assert!(ids.contains(&"skill.no-contract.unmeasured"), "{ids:?}");
        assert_eq!(row(&report, "silent").windowed, Measure::NoContract);
        assert_eq!(row(&report, "unmeasured").windowed, Measure::NoCommand);
    }

    /// A measure that cannot run reports `unmeasured` and never `0`, because
    /// a zero meaning "offline" is worse than a gap that admits it.
    #[test]
    fn a_broken_measure_is_unmeasured_never_zero() {
        let fixture = Fixture::new();
        fixture.skill("one-number", "produces: 'a thing'\nevidence: 'echo 3'\n");
        fixture.skill("exits-nonzero", "produces: 'a thing'\nevidence: 'exit 1'\n");

        let report = fixture.report(quick());
        for name in ["one-number", "exits-nonzero"] {
            assert_eq!(row(&report, name).windowed, Measure::Unmeasured);
            assert_eq!(row(&report, name).windowed.label(), "unmeasured");
            assert!(
                report
                    .findings
                    .iter()
                    .any(|f| f.id == format!("skill.evidence-failed.{name}")),
                "{name} should report a failed measure"
            );
        }
    }

    /// `--no-evidence` must actually not run anything. Asserted by pointing a
    /// measure at a command that would leave a file behind, and checking the
    /// file never appears — SEC-001 is about a subprocess, so the test has to
    /// be about a subprocess.
    #[test]
    fn no_evidence_runs_no_subprocess_at_all() {
        let fixture = Fixture::new();
        let marker = fixture.path().join("the-measure-ran");
        fixture.skill(
            "writes-a-file",
            &format!(
                "produces: 'a thing'\nevidence: 'touch {} && echo 1 1'\n",
                marker.display()
            ),
        );

        let report = fixture.report(Options {
            run_evidence: false,
            ..quick()
        });
        assert!(!marker.exists(), "the evidence command was executed");
        assert_eq!(row(&report, "writes-a-file").windowed, Measure::Unmeasured);
        assert!(
            report.findings.is_empty(),
            "skipping a measure is not a failure: {:?}",
            report.findings
        );

        // ...and with it on, the same measure does run, so the test above is
        // proving something.
        let report = fixture.report(quick());
        assert!(marker.exists());
        assert_eq!(
            row(&report, "writes-a-file").windowed,
            Measure::Rate(Rate {
                conforming: 1,
                total: 1
            })
        );
    }

    /// A rate over all history is dominated by history. The windowed one is
    /// what makes a regression visible while it is still one commit old.
    #[test]
    fn a_windowed_rate_below_all_time_reaches_the_agenda() {
        let fixture = Fixture::new();
        fixture.skill(
            "slipping",
            "produces: 'a thing'\nevidence: 'if [ -n \"$ASH_RANGE\" ]; then echo 1 2; else echo 9 10; fi'\n",
        );
        let report = fixture.report(quick());

        assert!(row(&report, "slipping").regressed());
        assert!(
            report
                .agenda
                .iter()
                .any(|item| item.contains("windowed rate below all-time")
                    && item.contains("slipping")),
            "{:?}",
            report.agenda
        );
    }

    /// An issue attributed to no skill is a gap, once per area its plan
    /// declares — and can be told the question has been settled.
    #[test]
    fn gaps_group_unowned_issues_by_area_and_can_be_answered() {
        let fixture = Fixture::new();
        fixture.skill("a-skill", "produces: none\nkind: reference\n");
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
        fixture.skill("a-skill", "produces: none\nkind: reference\n");
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
