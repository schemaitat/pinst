//! `pinst harness` — the checks over this repo's own agent corpus.
//!
//! The odd one out among the command families, in two ways worth stating
//! where someone will read them. It loads **no manifest**: its subject is the
//! repo the caller is standing in, not the machine. And it is **synchronous**
//! — nothing here probes a tool or talks to the network, so there is no
//! reason to make every caller `.await` a future that never yields.
//!
//! The exit codes carry the verdict as everywhere else: `0` clean, `2` a
//! usage error, `3` ran fine and found things to act on.

use color_eyre::eyre::Result;
use schemars::JsonSchema;
use serde::Serialize;

use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
use crate::cli::{
    HarnessAction, HarnessArgs, HarnessIndexArgs, HarnessNewIdArgs, HarnessSkillsArgs,
};
use crate::core::doctor::{Finding, Severity};
use crate::core::harness::check;
use crate::core::harness::corpus::Corpus;
use crate::core::harness::index::{self, IndexState};
use crate::core::harness::root::CorpusRoot;
use crate::core::harness::{evidence, id, skills};

/// What `new-id` returns. One field, but an envelope item all the same: an
/// agent that has learned to read `items[0]` from every other command should
/// not have to special-case this one.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MintedId {
    pub id: String,
}

pub fn run(ctx: &Ctx, args: &HarnessArgs) -> Result<ExitCode> {
    match &args.action {
        // `new-id` comes first because it is the one action that needs no
        // corpus: minting an id is what you do *before* there is a plan
        // directory to put it in.
        HarnessAction::NewId(new_id) => mint(ctx, new_id),
        HarnessAction::Index(index) => self::index(ctx, &load(ctx, args)?, index),
        HarnessAction::Check => self::check(ctx, &load(ctx, args)?),
        HarnessAction::Skills(skills) => self::skills(ctx, &load(ctx, args)?, skills),
    }
}

/// Discovers the corpus and reads it, announcing where it landed.
///
/// The note is not decoration: "checked the wrong repo" is the failure this
/// subsystem is most likely to have, and the working directory is the only
/// thing that decides.
fn load(ctx: &Ctx, args: &HarnessArgs) -> Result<Corpus> {
    let root = CorpusRoot::resolve(args.root.as_deref())?;
    let corpus = Corpus::load(root)?;
    ctx.note(format!(
        "harness: {} ({} plans)",
        corpus.root.path().display(),
        corpus.len()
    ));
    Ok(corpus)
}

/// Prints the bare id on stdout in human mode.
///
/// Deliberately unadorned: `scripts/ash.sh new-id` printed one line, and the
/// `/plan` command substitutes its output straight into a prompt. Anything
/// else here — a label, a trailing note — becomes part of a plan directory
/// name the first time someone pastes it.
fn mint(ctx: &Ctx, args: &HarnessNewIdArgs) -> Result<ExitCode> {
    let id = id::mint(args.date.as_deref())?;
    if !ctx.json {
        println!("{id}");
    }
    ctx.finish(Envelope::new(
        "harness new-id",
        Status::Ok,
        vec![MintedId { id }],
    ))
}

/// Rewrites `.ash/INDEX.md`, or with `--check` reports whether it is current.
///
/// A stale index is a `warning` rather than an `error` for the same reason it
/// is in `check`: the corpus is still correct, only its routing table has
/// fallen behind, and one command puts it right.
fn index(ctx: &Ctx, corpus: &Corpus, args: &HarnessIndexArgs) -> Result<ExitCode> {
    let state = index::state(corpus);
    let path = corpus.root.index_file();
    let shown = corpus.relative(&path);

    if args.check || ctx.dry_run {
        let finding = match state {
            IndexState::Fresh => None,
            IndexState::Stale => Some(Finding {
                id: "index.stale".to_string(),
                severity: Severity::Warning,
                message: format!("{shown} does not match the plan frontmatter"),
                remediation: "run: pinst harness index".to_string(),
                fixable: true,
            }),
            IndexState::Missing => Some(Finding {
                id: "index.missing".to_string(),
                severity: Severity::Warning,
                message: format!("{shown} does not exist"),
                remediation: "run: pinst harness index".to_string(),
                fixable: true,
            }),
        };
        return report(
            ctx,
            "harness index",
            corpus,
            finding.into_iter().collect(),
            &format!("{shown} is up to date"),
        );
    }

    index::write(corpus)?;
    report(
        ctx,
        "harness index",
        corpus,
        Vec::new(),
        &format!("wrote {shown}"),
    )
}

/// Every corpus invariant, in one pass.
///
/// Silent on a clean corpus beyond the one-line note, because any finding of
/// any severity exits 3 — so a check that speaks when nothing is wrong would
/// fail `just qc` on a healthy repo, and would be switched off within a week.
fn check(ctx: &Ctx, corpus: &Corpus) -> Result<ExitCode> {
    let findings = check::run(corpus);
    let clean = format!("corpus clean ({} plans)", corpus.len());
    report(ctx, "harness check", corpus, findings, &clean)
}

/// The shared rendering for every action that produces findings.
///
/// Human mode prints them in the same shape as `pinst doctor`, because an
/// operator reading both should not have to learn two layouts; JSON mode puts
/// them in `items` and lets `ctx.finish` pick the exit code.
fn report(
    ctx: &Ctx,
    command: &str,
    corpus: &Corpus,
    findings: Vec<Finding>,
    clean: &str,
) -> Result<ExitCode> {
    if !ctx.json {
        print_findings(&findings);
        if findings.is_empty() {
            ctx.note(format!("harness: {clean}"));
        }
    }

    let status = if findings.is_empty() {
        Status::Ok
    } else {
        Status::Issues
    };
    let summary = check::summary(corpus, &findings);
    ctx.finish(
        Envelope::new(command, status, findings)
            .dry_run(ctx.dry_run)
            .summary(summary),
    )
}

/// One row of the skill report, as `--json` carries it.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SkillReportItem {
    pub name: String,
    /// Whether it is projected into `.claude/skills`. An unwired skill never
    /// loads, and does so silently.
    pub wired: bool,
    /// What the skill says it leaves behind, from its own frontmatter.
    pub produces: Option<String>,
    pub windowed: MeasureItem,
    pub all_time: MeasureItem,
    /// How many recorded issues name this skill. Read it against the rate.
    pub issues: usize,
    /// Present only with `--transcripts`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invocations: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_invoked: Option<String>,
}

/// A conformance rate, or the reason there is not one.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MeasureItem {
    /// `null` when unmeasured — never `0`, which would read as "nothing
    /// conforms" rather than "nobody could tell".
    pub conforming: Option<u64>,
    pub total: Option<u64>,
    pub percent: Option<u64>,
    /// How the table renders it: a rate, `unmeasured`, `no contract`,
    /// `no measure`, or `reference (exempt)`.
    pub label: String,
}

impl From<&skills::Measure> for MeasureItem {
    fn from(measure: &skills::Measure) -> Self {
        let rate = match measure {
            skills::Measure::Rate(rate) => Some(*rate),
            _ => None,
        };
        Self {
            conforming: rate.map(|r| r.conforming),
            total: rate.map(|r| r.total),
            percent: rate.and_then(|r| r.percent()),
            label: measure.label(),
        }
    }
}

#[derive(Debug, Serialize)]
struct GapItem {
    area: String,
    issues: usize,
    ids: String,
}

/// The review pass: what each skill produces, how much of it conforms, and
/// what nobody owns.
///
/// Separate from `check` on purpose. `check` carries hard invariants and
/// gates `qc`; this carries numbers, and a number that got interesting would
/// fail the build if it were a finding. Invariants gate commits;
/// measurements start conversations.
fn skills(ctx: &Ctx, corpus: &Corpus, args: &HarnessSkillsArgs) -> Result<ExitCode> {
    let options = skills::Options {
        run_evidence: !args.no_evidence,
        transcripts: args.transcripts.as_deref(),
        timeout: evidence::TIMEOUT,
    };
    let report = skills::run(corpus, &options);

    if !ctx.json {
        render_table(&report);
        render_gaps(&report);
    }

    let items: Vec<SkillReportItem> = report
        .skills
        .iter()
        .map(|row| SkillReportItem {
            name: row.name.clone(),
            wired: row.wired,
            produces: row.produces.clone(),
            windowed: (&row.windowed).into(),
            all_time: (&row.all_time).into(),
            issues: row.issues,
            invocations: row.invocations,
            last_invoked: row.last_invoked.clone(),
        })
        .collect();

    let gaps: Vec<GapItem> = report
        .gaps
        .iter()
        .map(|gap| GapItem {
            area: gap.area.clone(),
            issues: gap.issues,
            ids: gap.ids.join(","),
        })
        .collect();

    let status = if report.findings.is_empty() {
        Status::Ok
    } else {
        Status::Issues
    };
    let mut summary = check::summary(corpus, &report.findings);
    if let Some(map) = summary.as_object_mut() {
        map.insert("gaps".to_string(), serde_json::to_value(&gaps)?);
        map.insert("agenda_items".to_string(), report.agenda.len().into());
    }

    if !ctx.json && report.findings.is_empty() {
        ctx.note("harness: every skill has a contract");
    }
    if !ctx.json {
        print_findings(&report.findings);
    }

    ctx.finish(
        Envelope::new("harness skills", status, items)
            .dry_run(ctx.dry_run)
            .summary(summary),
    )
}

/// Findings in the same shape `pinst doctor` prints them, on stdout: an
/// operator reading both should not have to learn two layouts, and stdout is
/// where pinst's human report goes.
fn print_findings(findings: &[Finding]) {
    for finding in findings {
        let mark = match finding.severity {
            Severity::Error => "[error]",
            Severity::Warning => "[warn ]",
            Severity::Info => "[info ]",
        };
        println!("{mark} {} {}", finding.id, finding.message);
        println!("         fix: {}", finding.remediation);
    }
}

fn render_table(report: &skills::Report) {
    let width = 22;
    if report.counted {
        println!(
            "{:<width$} {:<6} {:<22} {:<22} {:<7} {:<6} last seen",
            "skill",
            "wired",
            format!("conforming (last {})", evidence::WINDOW),
            "all-time",
            "issues",
            "fired",
        );
    } else {
        println!(
            "{:<width$} {:<6} {:<22} {:<22} issues",
            "skill",
            "wired",
            format!("conforming (last {})", evidence::WINDOW),
            "all-time",
        );
    }
    for row in &report.skills {
        let wired = if row.wired { "yes" } else { "no" };
        if report.counted {
            println!(
                "{:<width$} {:<6} {:<22} {:<22} {:<7} {:<6} {}",
                row.name,
                wired,
                row.windowed.label(),
                row.all_time.label(),
                row.issues,
                row.invocations.unwrap_or(0),
                row.last_invoked.as_deref().unwrap_or("-")
            );
        } else {
            println!(
                "{:<width$} {:<6} {:<22} {:<22} {}",
                row.name,
                wired,
                row.windowed.label(),
                row.all_time.label(),
                row.issues
            );
        }
    }
}

fn render_gaps(report: &skills::Report) {
    println!("\n## Gaps — recorded issues no skill owns");
    if report.gaps.is_empty() {
        println!("  none");
    } else {
        for gap in &report.gaps {
            println!(
                "  {:<14} {} issue(s)  {}",
                gap.area,
                gap.issues,
                gap.ids.join(",")
            );
        }
    }

    println!("\n## Agenda");
    if report.agenda.is_empty() {
        println!("  nothing to act on");
    } else {
        for item in &report.agenda {
            println!("  - {item}");
        }
    }
}
