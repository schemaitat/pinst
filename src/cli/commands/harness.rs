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

use std::path::PathBuf;

use color_eyre::eyre::Result;
use schemars::JsonSchema;
use serde::Serialize;

use crate::cli::output::{Ctx, Envelope, ExitCode, Status};
use crate::cli::{
    HarnessAction, HarnessArgs, HarnessIndexArgs, HarnessNewIdArgs, HarnessRenumberLessonArgs,
    HarnessSkillsArgs, HarnessStatusArgs, ScopeArg,
};
use crate::core::doctor::{Finding, Severity};
use crate::core::harness::check;
use crate::core::harness::corpus::Corpus;
use crate::core::harness::index::{self, IndexState};
use crate::core::harness::install::state::{self, AssetKindLabel, AssetState, AssetStatus};
use crate::core::harness::project::InstallRoot;
use crate::core::harness::root::CorpusRoot;
use crate::core::harness::vendor::Vendor;
use crate::core::harness::{asset, evidence, id, renumber, skills};
use crate::core::home_dir;
use crate::core::usage;

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
        HarnessAction::RenumberLesson(renumber) => {
            self::renumber_lesson(ctx, &load(ctx, args)?, renumber)
        }
        // `status` reads `.agents/` and the vendor directories it projects
        // into, not the `.ash/` corpus, so it does not go through `load`.
        HarnessAction::Status(status_args) => self::status(ctx, args, status_args),
    }
}

/// The ref a merge-base is taken against when the caller names none.
///
/// Defaulted here rather than in `clap` so `--json` reports the ref that was
/// actually used: a report saying `against: null` leaves the one thing that
/// decided the rewrite's scope unrecorded.
const DEFAULT_AGAINST: &str = "main";

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

/// One row of `harness status --json`: an `install::state::AssetStatus`
/// tagged with which scope it came from, because `--scope both` reports two
/// answers to "is this installed" and the payload has to say which is which.
#[derive(Debug, Serialize, JsonSchema)]
pub struct StatusItem {
    pub scope: String,
    pub kind: AssetKindLabel,
    pub name: String,
    pub target: PathBuf,
    pub state: AssetState,
}

impl StatusItem {
    fn from(scope: &str, row: AssetStatus) -> Self {
        Self {
            scope: scope.to_string(),
            kind: row.kind,
            name: row.name,
            target: row.target,
            state: row.state,
        }
    }
}

/// `pinst harness status` — where the harness is installed, right now,
/// computed by comparing the vendor directories against `.agents/` rather
/// than by trusting a receipt. A receipt says what an install *did*; this
/// says what is actually there, which is the question `status` exists to
/// answer even when nothing was ever installed by this binary at all (a
/// hand-wired repo, or one still running `agents-wire.sh`).
fn status(ctx: &Ctx, args: &HarnessArgs, status_args: &HarnessStatusArgs) -> Result<ExitCode> {
    let vendor = Vendor::parse(&status_args.vendor).ok_or_else(|| {
        usage(format!(
            "unknown vendor '{}' (try: claude)",
            status_args.vendor
        ))
    })?;
    let source = asset::resolve_source();

    let mut items = Vec::new();
    let mut summary = serde_json::Map::new();
    let want_project = matches!(status_args.scope, ScopeArg::Project | ScopeArg::Both);
    let want_global = matches!(status_args.scope, ScopeArg::Global | ScopeArg::Both);

    if want_project {
        let root = InstallRoot::resolve(args.root.as_deref())?;
        let rows = state::status(&source, vendor, root.path())?;
        if !ctx.json {
            print_scope_banner("project", root.path(), &rows);
        }
        summary.insert("project".to_string(), scope_summary(root.path(), &rows));
        items.extend(rows.into_iter().map(|r| StatusItem::from("project", r)));
    }

    if want_global {
        let home = home_dir()?;
        let rows = state::status(&source, vendor, &home)?;
        if !ctx.json {
            print_scope_banner("global", &home, &rows);
        }
        summary.insert("global".to_string(), scope_summary(&home, &rows));
        items.extend(rows.into_iter().map(|r| StatusItem::from("global", r)));
    }

    let issues = items
        .iter()
        .filter(|i| {
            matches!(
                i.state,
                AssetState::Missing | AssetState::Drifted | AssetState::Foreign
            )
        })
        .count();
    let status = if issues > 0 {
        Status::Issues
    } else {
        Status::Ok
    };

    ctx.finish(
        Envelope::new("harness status", status, items).summary(serde_json::Value::Object(summary)),
    )
}

fn scope_summary(root: &std::path::Path, rows: &[AssetStatus]) -> serde_json::Value {
    let count = |state: AssetState| rows.iter().filter(|r| r.state == state).count();
    serde_json::json!({
        "root": root,
        "linked": count(AssetState::Linked),
        "copied": count(AssetState::Copied),
        "missing": count(AssetState::Missing),
        "drifted": count(AssetState::Drifted),
        "foreign": count(AssetState::Foreign),
        "unmanaged": count(AssetState::Unmanaged),
    })
}

fn print_scope_banner(label: &str, root: &std::path::Path, rows: &[AssetStatus]) {
    let installed = rows.iter().any(|r| r.state.satisfied());
    println!(
        "\n== {label} ({}) — {} ==",
        root.display(),
        if installed {
            "installed"
        } else {
            "not installed"
        }
    );
    for row in rows {
        let mark = match row.state {
            AssetState::Linked | AssetState::Copied => "[ok]",
            AssetState::Missing => "[--]",
            AssetState::Drifted | AssetState::Foreign => "[!!]",
            AssetState::Unmanaged => "[??]",
        };
        println!(
            "{mark} {:<8} {:<24} {:?}",
            row.kind.label(),
            row.name,
            row.state
        );
    }
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

/// Moves one lesson to a free id and takes this branch's citations with it.
///
/// Not a `report()` caller, because it produces no findings: this is the
/// *remedy* for `lesson.duplicate-id`, so its exit codes are the plain ones —
/// `0` when it renamed something, `2` when the title matched no heading or
/// several (which `renumber` raises as a `UsageError` and `main` maps).
/// Exiting `3` here would tell a caller there is still something to act on
/// immediately after the thing was acted on.
fn renumber_lesson(
    ctx: &Ctx,
    corpus: &Corpus,
    args: &HarnessRenumberLessonArgs,
) -> Result<ExitCode> {
    let report = renumber::run(
        &corpus.root,
        &renumber::Options {
            title: args.title.clone(),
            to: args.to.clone(),
            against: args
                .against
                .clone()
                .unwrap_or_else(|| DEFAULT_AGAINST.to_string()),
            dry_run: ctx.dry_run,
        },
    )?;

    if !ctx.json {
        print_renumber(&report);
    }
    ctx.finish(
        Envelope::new("harness renumber-lesson", Status::Ok, vec![&report]).dry_run(ctx.dry_run),
    )
}

/// The human report.
///
/// Every line it did not rewrite is printed as loudly as every line it did:
/// in degrade mode the unrewritten hits *are* the output, and a tool that
/// buried them would have quietly reinstated the manual grep it replaced.
fn print_renumber(report: &renumber::Report) {
    let verb = if report.dry_run {
        "would move"
    } else {
        "moved"
    };
    println!(
        "{verb} {} -> {} ({})",
        report.old_id, report.new_id, report.title
    );
    println!(
        "  {}:{}  {}",
        report.heading.file, report.heading.line, report.heading.text
    );
    for citation in &report.rewritten {
        println!("  {}:{}  {}", citation.file, citation.line, citation.text);
    }

    match &report.scope {
        renumber::Scope::Scoped {
            against,
            merge_base,
        } => println!(
            "\nscope: {} citation(s) this branch added since {against} ({})",
            report.rewritten.len(),
            &merge_base[..merge_base.len().min(12)]
        ),
        renumber::Scope::Degraded { against, reason } => {
            println!("\nscope: unknown — no merge-base against {against}: {reason}");
            println!(
                "the heading was renamed; these {} occurrence(s) were NOT touched, because",
                report.unrewritten.len()
            );
            println!("nothing could tell which of them mean the lesson that moved:");
            for citation in &report.unrewritten {
                println!("  {}:{}  {}", citation.file, citation.line, citation.text);
            }
        }
    }
    println!("\nreview the diff before committing: no tool can tell whether a");
    println!("rewritten citation was the one you meant.");
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

/// One row of the skill report, carried in `summary.skills` by `--json`.
///
/// Not in `items`: that slot holds findings for every command in this family,
/// so an agent branching on exit code 3 finds the reason in the same place
/// each time.
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
    let summary = skills_summary(corpus, &report, &items, &gaps)?;

    if !ctx.json && report.findings.is_empty() {
        ctx.note("harness: every skill has a contract");
    }
    if !ctx.json {
        print_findings(&report.findings);
    }

    // `items` is findings, here as in `check` and as in `pinst doctor` — the
    // exit code says there is something to act on, so the thing to act on has
    // to be in the payload. Putting the skill rows there instead left a
    // `status: issues` envelope whose reason appeared nowhere in it, which is
    // what the scheduled distillation found when it tried to read this.
    // The report itself is `summary.skills`, beside the counts it belongs to.
    ctx.finish(
        Envelope::new("harness skills", status, report.findings)
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

/// The `summary` object `skills --json` carries.
///
/// A free function so the envelope's shape can be tested. It was not, and a
/// `status: issues` envelope shipped whose findings appeared nowhere in it —
/// caught only when the scheduled distillation tried to read
/// `agenda_items` out of it.
fn skills_summary(
    corpus: &Corpus,
    report: &skills::Report,
    rows: &[SkillReportItem],
    gaps: &[GapItem],
) -> Result<serde_json::Value> {
    let mut summary = check::summary(corpus, &report.findings);
    if let Some(map) = summary.as_object_mut() {
        map.insert("skills".to_string(), serde_json::to_value(rows)?);
        map.insert("gaps".to_string(), serde_json::to_value(gaps)?);
        map.insert("agenda_items".to_string(), report.agenda.len().into());
        map.insert("agenda".to_string(), serde_json::to_value(&report.agenda)?);
    }
    Ok(summary)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::harness::root::CorpusRoot;

    /// The envelope contract for `skills`, which nothing checked until a
    /// consumer arrived and could not read it.
    ///
    /// Two properties matter and neither is obvious from the call site:
    /// `items` carries the findings, so an exit code of 3 always has a
    /// reason in the payload; and the counts a caller branches on sit under
    /// `summary`, where `pinst schema output` says command-specific counts
    /// live.
    #[test]
    fn the_skills_envelope_puts_findings_in_items_and_counts_in_summary() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".ash/plans")).unwrap();
        let skill = dir.path().join(".agents/skills/silent");
        std::fs::create_dir_all(&skill).unwrap();
        // No `produces:`, so this skill has no contract and is a finding.
        std::fs::write(
            skill.join("SKILL.md"),
            "---\nname: silent\ndescription: x\n---\n",
        )
        .unwrap();

        let corpus = Corpus::load(CorpusRoot::new(dir.path())).unwrap();
        let report = skills::run(
            &corpus,
            &skills::Options {
                run_evidence: false,
                ..skills::Options::default()
            },
        );
        let rows: Vec<SkillReportItem> = report
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

        let summary = skills_summary(&corpus, &report, &rows, &[]).unwrap();
        let envelope = Envelope::new("harness skills", Status::Issues, report.findings.clone())
            .summary(summary);
        let value = serde_json::to_value(&envelope).unwrap();

        // The reason for the exit code is in the payload.
        assert_eq!(value["status"], "issues");
        assert_eq!(value["items"][0]["id"], "skill.no-contract.silent");
        assert!(value["items"][0]["remediation"].as_str().is_some());

        // ...and the report and its counts are where a caller looks for them.
        assert_eq!(value["summary"]["skills"][0]["name"], "silent");
        assert_eq!(value["summary"]["findings"], 1);
        assert!(value["summary"]["agenda_items"].is_number());
    }
}
