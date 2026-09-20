//! The corpus invariants: what `just qc` gates on.
//!
//! Every check here enforces something the skills previously only asserted in
//! prose. Prose is enforced by remembering; this is enforced by an exit code,
//! which is checkable by anyone, at any time, without having read the skill
//! (LESSON-008).
//!
//! Three properties are deliberate and easy to lose in a port:
//!
//! - **Nothing exits early.** One run reports the whole state of the corpus,
//!   not its first problem.
//! - **Finding ids are stable and literal.** An agent matches on
//!   `phase.logged-not-done`, and so does `lesson.unenforced`, which greps
//!   the source for a lesson's `**Check:**` id. An id assembled from
//!   fragments would compile, pass every test, and break that grep silently.
//! - **Only hard invariants live here.** Rates, tallies and suggestions
//!   belong to `skills`, because any finding of any severity exits 3, and a
//!   check that speaks on a healthy corpus is a check that gets switched off.
//!
//! Several of these compare the corpus against a record written *after* the
//! fact — `CHANGELOG.log`, a run log — because that is the only kind of
//! evidence that can contradict it. A checker comparing a record only with
//! itself passes happily on a record describing a world that no longer exists
//! (LESSON-009).

use std::collections::BTreeSet;

use crate::core::doctor::{Finding, Severity};

use super::corpus::{self as model, Corpus, Plan};
use super::id;
use super::index::{self, IndexState};

/// Directories a `**Check:**` search never descends into.
///
/// `.ash` is the important one and the only subtle one: the `**Check:**`
/// line *is* in `LEARNINGS.md`, so searching the corpus would find every id
/// inside its own declaration and the check could never fire. The other two
/// are skipped for speed.
///
/// The same trap applies to prose anywhere else in the repo: this search
/// cannot tell a finding id in a comment from one in a `finding()` call, so
/// writing a real id into a doc comment here would quietly satisfy the
/// lesson that names it. Refer to them by shape, not by name — which is why
/// this paragraph does not contain one.
const NOT_SOURCE: [&str; 3] = [".ash", ".git", "target"];

/// Runs every invariant. Order matches the bash implementation's so two
/// reports read the same way side by side.
pub fn run(corpus: &Corpus) -> Vec<Finding> {
    let mut findings = Vec::new();

    if !corpus.plans_dir_exists {
        findings.push(error(
            "corpus.missing",
            format!(
                "{} does not exist",
                corpus.relative(&corpus.root.plans_dir())
            ),
            "create it, or run plan-write to author the first plan",
        ));
    } else {
        let mut seen_ids: BTreeSet<&str> = BTreeSet::new();
        for plan in &corpus.plans {
            check_plan(corpus, plan, &mut seen_ids, &mut findings);
        }
    }

    check_lessons(corpus, &mut findings);
    check_distillation(corpus, &mut findings);
    check_index(corpus, &mut findings);
    findings
}

fn check_plan<'c>(
    corpus: &'c Corpus,
    plan: &'c Plan,
    seen_ids: &mut BTreeSet<&'c str>,
    findings: &mut Vec<Finding>,
) {
    let name = &plan.name;
    let Some((id, slug)) = plan.id.as_deref().zip(plan.slug.as_deref()) else {
        let suggestion = id::mint(None).unwrap_or_else(|_| "260919-qwerty".to_string());
        findings.push(error(
            format!("plan.malformed-dir.{name}"),
            format!("plan directory '{name}' is not <yymmdd>-<6 letters>-<kebab-slug>"),
            format!("rename it, e.g. {suggestion}-{name}"),
        ));
        return;
    };

    if !seen_ids.insert(id) {
        findings.push(error(
            format!("plan.duplicate-id.{id}"),
            format!("id {id} is used by more than one plan directory"),
            "mint a fresh id for the later plan: pinst harness new-id",
        ));
    }

    let Some(readme) = plan.readme.as_ref() else {
        findings.push(error(
            format!("plan.readme-missing.{name}"),
            format!("{}/README.md is missing", corpus.relative(&plan.dir)),
            "every plan folder needs a README.md, single- or multi-phase",
        ));
        return;
    };
    let shown = corpus.relative(&readme.path);

    if let Some(error_) = readme.error.as_ref() {
        findings.push(error(
            format!("plan.frontmatter-unparsed.{name}"),
            format!("{shown} frontmatter does not parse: {error_}"),
            "fix the line it names — the frontmatter must be flat 'key: value' \
             entries, with lists written inline as [a, b, c]",
        ));
    } else if readme.frontmatter.is_none() {
        findings.push(error(
            format!("plan.frontmatter-missing.{name}"),
            format!("{shown} has no YAML frontmatter"),
            "add the frontmatter block from the plan-write skill (Step 5)",
        ));
    } else {
        for key in [
            "id", "slug", "status", "created", "updated", "areas", "summary",
        ] {
            if readme.text(key).is_none() {
                findings.push(error(
                    format!("plan.frontmatter-key.{name}.{key}"),
                    format!("{shown} frontmatter is missing '{key}'"),
                    format!("add '{key}:' — .ash/INDEX.md is generated from these keys"),
                ));
            }
        }
        if let Some(declared) = readme.text("id").filter(|d| d != id) {
            findings.push(error(
                format!("plan.id-mismatch.{name}"),
                format!("{shown} frontmatter id '{declared}' does not match directory id '{id}'"),
                format!("set id: {id}"),
            ));
        }
        if let Some(declared) = readme.text("slug").filter(|d| d != slug) {
            findings.push(error(
                format!("plan.slug-mismatch.{name}"),
                format!(
                    "{shown} frontmatter slug '{declared}' does not match directory slug '{slug}'"
                ),
                format!("set slug: {slug}"),
            ));
        }
        check_status_value(
            corpus,
            readme,
            format!("plan.status-invalid.{name}"),
            findings,
        );
    }
    check_status_section(corpus, readme, findings);

    check_phases(corpus, plan, id, findings);
    check_learnings(corpus, plan, findings);
    check_changelog(corpus, plan, id, findings);
}

fn check_status_value(
    corpus: &Corpus,
    document: &model::Document,
    id: String,
    findings: &mut Vec<Finding>,
) {
    let Some(status) = document.status() else {
        return;
    };
    if matches!(status.as_str(), "Proposed" | "In Progress" | "Done" | "") {
        return;
    }
    findings.push(error(
        id,
        format!("{} has status '{status}'", corpus.relative(&document.path)),
        "use one of: Proposed, In Progress, Done",
    ));
}

/// Status lives in frontmatter only. Two copies of one mutable field is two
/// copies to drift apart, which is the whole reason the skills forbid it.
fn check_status_section(corpus: &Corpus, document: &model::Document, findings: &mut Vec<Finding>) {
    if !document.has_status_section() {
        return;
    }
    findings.push(warning(
        format!(
            "plan.status-section.{}",
            corpus.plan_relative(&document.path)
        ),
        format!(
            "{} has a '## Status' section as well as frontmatter status",
            corpus.relative(&document.path)
        ),
        "delete the section; frontmatter status is authoritative",
    ));
}

fn check_phases(corpus: &Corpus, plan: &Plan, id: &str, findings: &mut Vec<Finding>) {
    if plan.phases.is_empty() {
        return;
    }
    let name = &plan.name;
    let mut expected = 1u32;

    for phase in &plan.phases {
        let document = &phase.document;
        let shown = corpus.relative(&document.path);
        let key = corpus.plan_relative(&document.path);

        let Some(number) = phase.number else {
            findings.push(error(
                format!("phase.malformed-name.{key}"),
                format!("{shown} is not phase-<two digits>.md"),
                format!("rename it, e.g. phase-{expected:02}.md"),
            ));
            continue;
        };

        // Phases are a strictly linear chain, so their numbers must run 1..N
        // with no gaps — a gap means a phase file was lost or never written.
        if number != expected {
            findings.push(error(
                format!("phase.chain-gap.{name}"),
                format!("{shown} breaks the phase chain: expected phase {expected}"),
                "renumber the phase files so they run 01..N with no gaps",
            ));
        }
        expected += 1;

        if let Some(error_) = document.error.as_ref() {
            findings.push(error(
                format!("phase.frontmatter-unparsed.{key}"),
                format!("{shown} frontmatter does not parse: {error_}"),
                "fix the line it names — the frontmatter must be flat 'key: value' entries",
            ));
            continue;
        }
        if document.frontmatter.is_none() {
            findings.push(error(
                format!("phase.frontmatter-missing.{key}"),
                format!("{shown} has no YAML frontmatter"),
                "add id/slug/phase/status per the plan-write skill (Step 4)",
            ));
            continue;
        }

        for field in ["id", "slug", "phase", "status"] {
            if document.text(field).is_none() {
                findings.push(error(
                    format!("phase.frontmatter-key.{key}.{field}"),
                    format!("{shown} frontmatter is missing '{field}'"),
                    format!("add '{field}:'"),
                ));
            }
        }
        if document.text("id").as_deref() != Some(id) {
            findings.push(error(
                format!("phase.id-mismatch.{key}"),
                format!("{shown} frontmatter id does not match its plan's id '{id}'"),
                format!("set id: {id}"),
            ));
        }
        if let Some(declared) = document.text("phase")
            && declared != number.to_string()
        {
            findings.push(error(
                format!("phase.number-mismatch.{key}"),
                format!("{shown} frontmatter says phase {declared} but the filename says {number}"),
                "make them agree",
            ));
        }

        check_status_value(
            corpus,
            document,
            format!("phase.status-invalid.{key}"),
            findings,
        );
        check_status_section(corpus, document, findings);

        // The README's ## Phases table mirrors each phase's status for
        // readability; whoever flips one flips both, so disagreement is a
        // finding rather than a formatting quibble.
        if let (Some(status), Some(mirrored)) = (document.status(), table_status(plan, number))
            && status != mirrored
        {
            findings.push(warning(
                format!("phase.status-mirror.{name}.{number}"),
                format!(
                    "phase {number} is '{status}' in {shown} but '{mirrored}' in the README's Phases table"
                ),
                format!("update the table row to '{status}'"),
            ));
        }
    }

    if plan.is_done() {
        for phase in &plan.phases {
            if phase.document.status().as_deref() != Some("Done") {
                findings.push(error(
                    format!("plan.done-with-open-phase.{name}"),
                    format!(
                        "{name} is marked Done but {} is not",
                        corpus.relative(&phase.document.path)
                    ),
                    "finish the phase, or set the plan back to In Progress",
                ));
            }
        }
    } else {
        // The converse, and the cheaper half to forget: when the last phase
        // closes, the plan is over. Left open it goes on advertising work
        // that has already shipped, and the longer it does the more expensive
        // it is for the next reader to tell the difference.
        let all_done = plan
            .phases
            .iter()
            .all(|p| p.document.status().as_deref() == Some("Done"));
        if all_done {
            let status = plan.status().unwrap_or_default();
            findings.push(warning(
                format!("plan.phases-all-done.{name}"),
                format!("every phase of {name} is Done but the plan is '{status}'"),
                "close the plan (status: Done) and run the plan-learn skill, or reopen the \
                 phase that is not finished",
            ));
        }
    }
}

/// The status this phase has in the README's `## Phases` table, read by
/// column out of the markdown row whose first cell is the phase number.
fn table_status(plan: &Plan, number: u32) -> Option<String> {
    let readme = plan.readme.as_ref()?;
    let text = std::fs::read_to_string(&readme.path).ok()?;
    for line in text.lines() {
        if !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // ["", "1", "title", "file", "Status", ""]
        if cells.len() < 6 {
            continue;
        }
        if cells[1].parse::<u32>() == Ok(number) {
            return Some(cells[4].to_string());
        }
    }
    None
}

/// A missing `learnings.md` and a clean run must look different to whoever
/// checks later.
///
/// The trigger is a run that reached `run_end`, never the mere existence of a
/// log: `learnings.md` cannot exist until a run ends, so firing on the log's
/// first line would keep `qc` red for the whole duration of every
/// implementation run, on every plan, by design — and a gate that is always
/// red during work is a gate people learn to skip (LESSON-011).
fn check_learnings(corpus: &Corpus, plan: &Plan, findings: &mut Vec<Finding>) {
    if plan.has_learnings {
        return;
    }
    let name = &plan.name;
    let id = format!("plan.learnings-missing.{name}");
    let remediation = format!("run the plan-learn skill for {name}");
    let _ = corpus;

    if plan.is_done() {
        findings.push(warning(
            id,
            format!("{name} is Done but has no learnings.md"),
            remediation,
        ));
    } else if plan.has_finished_run {
        findings.push(info(
            id,
            format!("{name} has a finished run but no learnings.md yet"),
            remediation,
        ));
    }
}

/// The corpus against the one record written after the fact.
///
/// Only one direction is an invariant. A phase with no log line may simply
/// predate the log; a log line with no finished phase is drift.
fn check_changelog(corpus: &Corpus, plan: &Plan, id: &str, findings: &mut Vec<Finding>) {
    let log = corpus.root.changelog_file();
    if !log.is_file() {
        return;
    }
    let entries: Vec<_> = model::read_changelog(&log)
        .into_iter()
        .filter(|entry| entry.id == id)
        .collect();
    let name = &plan.name;
    let shown = corpus.relative(&log);

    if plan.is_done() && entries.is_empty() {
        findings.push(warning(
            format!("plan.unlogged.{name}"),
            format!("{name} is Done but {shown} has no entry for id={id}"),
            "append the closing line plan-implement writes when a plan ships",
        ));
    }

    let phases: BTreeSet<u32> = entries.iter().filter_map(|entry| entry.phase).collect();
    for number in phases {
        let Some(phase) = plan.phases.iter().find(|p| p.number == Some(number)) else {
            findings.push(warning(
                format!("phase.logged-missing.{name}.{number}"),
                format!(
                    "{shown} records phase {number} of {id} as shipped but \
                     {}/phase-{number:02}.md does not exist",
                    corpus.relative(&plan.dir)
                ),
                "restore the phase file, or correct the id in the log entry",
            ));
            continue;
        };
        let status = phase.document.status().unwrap_or_default();
        if status != "Done" {
            findings.push(warning(
                format!("phase.logged-not-done.{name}.{number}"),
                format!(
                    "{shown} says phase {number} of {id} shipped, but {} is '{status}'",
                    corpus.relative(&phase.document.path)
                ),
                "set that phase to Done, or work out what the log entry was really about",
            ));
        }
    }
}

/// A lesson claiming enforcement it does not have is worse than one honestly
/// marked `prose`: it tells the next reader the problem is handled. Hence
/// `error`, not `warning`.
fn check_lessons(corpus: &Corpus, findings: &mut Vec<Finding>) {
    let file = corpus.root.learnings_file();
    if !file.is_file() {
        return;
    }
    let shown = corpus.relative(&file);
    let lessons = model::read_lessons(&file);

    // LESSON-022: a shared counter minted by hand collides across parallel
    // branches, and nothing used to notice — the renumbering that followed
    // was manual and silent both times it happened.
    //
    // One finding per colliding id, not one per extra heading: three headings
    // sharing a number are one problem, and emitting it twice under the same
    // finding id would give an agent two rows it cannot tell apart. Reported
    // at the *second* occurrence so the order stays the document's.
    let mut defined: BTreeSet<&str> = BTreeSet::new();
    let mut reported: BTreeSet<&str> = BTreeSet::new();
    for lesson in &lessons {
        if defined.insert(lesson.id.as_str()) || !reported.insert(lesson.id.as_str()) {
            continue;
        }
        let id = &lesson.id;
        let group: Vec<&model::Lesson> = lessons.iter().filter(|l| &l.id == id).collect();
        findings.push(error(
            format!("lesson.duplicate-id.{id}"),
            format!(
                "{shown} has {} '### {id}' headings: {}",
                group.len(),
                group
                    .iter()
                    .map(|l| shown_title(l))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            renumber_remediation(id, &group),
        ));
    }

    if let Ok(text) = std::fs::read_to_string(&file) {
        let mut cited: BTreeSet<String> = BTreeSet::new();
        for reference in model::lesson_ids(&text) {
            if cited.insert(reference.clone()) && !defined.contains(reference.as_str()) {
                findings.push(error(
                    format!("lesson.dangling-reference.{reference}"),
                    format!(
                        "{shown} cites {reference}, which no '### {reference}' heading defines"
                    ),
                    "fix the typo, or write the lesson it was meant to cite",
                ));
            }
        }
    }

    for lesson in &lessons {
        if lesson.status.as_deref() != Some("mechanized") {
            continue;
        }
        let id = format!("lesson.unenforced.{}", lesson.id);
        match lesson.check.as_deref() {
            None | Some("-") => findings.push(error(
                id,
                format!(
                    "{shown} marks {} mechanized but names no '**Check:**' finding id",
                    lesson.id
                ),
                "add the finding id that enforces it, or set '**Status:** prose'",
            )),
            Some(check) if !emitted_somewhere(corpus, check) => findings.push(error(
                id,
                format!(
                    "{shown} says {} is mechanized by '{check}', which nothing in this repo \
                     emits",
                    lesson.id
                ),
                "point '**Check:**' at a finding id that exists, or set '**Status:** prose'",
            )),
            Some(_) => {}
        }
    }
}

/// How `lesson.duplicate-id` tells the reader to fix it.
///
/// It names each colliding heading by its **title**, because that is what
/// `renumber::locate` matches on and it is the only thing that tells two
/// headings sharing a number apart. Naming the id instead — which a title can
/// never contain — produced a remediation whose command failed every time it
/// was followed.
fn renumber_remediation(id: &str, group: &[&model::Lesson]) -> String {
    let usable: Vec<&str> = group
        .iter()
        .map(|lesson| lesson.title.as_str())
        .filter(|title| !title.is_empty())
        .collect();
    if usable.is_empty() {
        return format!(
            "give the '### {id}' headings distinct titles, then run: \
             pinst harness renumber-lesson --title \"<that title>\""
        );
    }
    let commands: Vec<String> = usable
        .iter()
        .map(|title| format!("pinst harness renumber-lesson --title \"{title}\""))
        .collect();
    format!(
        "renumber whichever one did not reach main first — {} — adding --dry-run \
         first to see what it would move",
        commands.join(", or ")
    )
}

/// A lesson's title as a finding should print it, including the untitled case
/// — which is a real heading shape and must not render as an empty pair of
/// quotes the reader cannot act on.
fn shown_title(lesson: &model::Lesson) -> String {
    if lesson.title.is_empty() {
        format!("{} (untitled)", lesson.id)
    } else {
        format!("'{}'", lesson.title)
    }
}

/// Whether any file in the repo — outside the corpus itself — contains this
/// finding id as a literal.
///
/// A plain substring search, exactly as `grep -rqF` was, and the reason ids
/// must stay contiguous literals: an id built by concatenation is invisible
/// here, and the lesson naming it would be reported as unenforced although
/// the check exists.
///
/// The search covers the whole repo rather than a fixed `src/` and
/// `scripts/`: those are this project's layout, and `pinst harness` runs
/// against whatever repo the caller is standing in — a lesson mechanized by
/// something in `tools/` or `lib/` is not this command's business to
/// disbelieve.
///
/// Documentation is excluded, though. An id *emitted* by a check is in code;
/// an id in a `.md` file is someone writing about the check, and a lesson
/// satisfied by the prose describing it would be enforced by nothing at all.
fn emitted_somewhere(corpus: &Corpus, check: &str) -> bool {
    contains_literal(corpus.root.path(), check)
}

/// Extensions that are documentation rather than something that runs.
const NOT_CODE: [&str; 3] = ["md", "txt", "log"];

fn contains_literal(dir: &std::path::Path, needle: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if NOT_SOURCE.iter().any(|skip| name == *skip) {
            continue;
        }
        if path.is_dir() {
            if contains_literal(&path, needle) {
                return true;
            }
            continue;
        }
        if path
            .extension()
            .map(|ext| NOT_CODE.iter().any(|skip| ext == *skip))
            .unwrap_or(false)
        {
            continue;
        }
        if std::fs::read_to_string(&path)
            .map(|text| text.contains(needle))
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

/// A plan closing starts the clock on distilling what it taught, and this
/// finding is the clock: it fires exactly when a plan has recorded something
/// nobody has decided about, and stops when someone decides.
///
/// Three outcomes, one of them free — promote it, fold it into an existing
/// lesson's `Seen in:` line, or write `**Distilled:** declined — <reason>`.
/// A check that cries wolf is a check that gets deleted, so declining has to
/// be a one-liner (LESSON-013, LESSON-014).
fn check_distillation(corpus: &Corpus, findings: &mut Vec<Finding>) {
    let learnings = corpus.root.learnings_file();
    let lessons = model::read_lessons(&learnings);

    // A `Seen in:` line naming two plans yields the cross-product, which can
    // only silence a finding — the safe direction.
    let mut cited: BTreeSet<(String, String)> = BTreeSet::new();
    for lesson in &lessons {
        for plan in &lesson.plans {
            for issue in &lesson.issues {
                cited.insert((plan.clone(), issue.clone()));
            }
        }
    }

    for plan in &corpus.plans {
        let Some(id) = plan.id.as_deref() else {
            continue;
        };
        let file = plan.learnings_file();
        if !file.is_file() {
            continue;
        }
        for issue in model::read_issues(&file) {
            if issue.distilled_declined || cited.contains(&(id.to_string(), issue.id.clone())) {
                continue;
            }
            findings.push(warning(
                format!("learnings.untriaged.{}.{}", plan.name, issue.id),
                format!(
                    "{} records {} but no lesson in {} references it",
                    corpus.relative(&file),
                    issue.id,
                    corpus.relative(&learnings)
                ),
                "run the plan-learn skill: promote it, add it to an existing lesson's \
                 'Seen in:' line, or write '**Distilled:** declined — <reason>' on the issue",
            ));
        }
    }
}

/// A stale index is a corpus problem like any other, so `check` catches it
/// without the caller having to remember a second command.
fn check_index(corpus: &Corpus, findings: &mut Vec<Finding>) {
    let shown = corpus.relative(&corpus.root.index_file());
    match index::state(corpus) {
        IndexState::Fresh => {}
        IndexState::Stale => findings.push(warning(
            "index.stale",
            format!("{shown} does not match the plan frontmatter"),
            "run: pinst harness index",
        )),
        IndexState::Missing => findings.push(warning(
            "index.missing",
            format!("{shown} does not exist"),
            "run: pinst harness index",
        )),
    }
}

/// The counts `--json` carries in `summary`.
///
/// Deliberately the same seven keys the bash envelope had: `plans`,
/// `lessons` and `issues` size the corpus, the rest size the report. An agent
/// already reading that envelope should not have to learn a new shape because
/// the implementation language changed.
pub fn summary(corpus: &Corpus, findings: &[Finding]) -> serde_json::Value {
    let lessons = model::read_lessons(&corpus.root.learnings_file()).len();
    let issues: usize = corpus
        .plans
        .iter()
        .map(|plan| model::read_issues(&plan.learnings_file()).len())
        .sum();
    let count = |severity: Severity| findings.iter().filter(|f| f.severity == severity).count();
    serde_json::json!({
        "plans": corpus.len(),
        "lessons": lessons,
        "issues": issues,
        "findings": findings.len(),
        "error": count(Severity::Error),
        "warning": count(Severity::Warning),
        "info": count(Severity::Info),
    })
}

fn finding(
    id: impl Into<String>,
    severity: Severity,
    message: impl Into<String>,
    remediation: impl Into<String>,
) -> Finding {
    Finding {
        id: id.into(),
        severity,
        message: message.into(),
        remediation: remediation.into(),
        // Nothing here is auto-fixable. Every one of these needs a decision
        // about which side is right, which is the one thing a fixer cannot
        // make — `index.stale` excepted, and that one has its own command.
        fixable: false,
    }
}

fn error(
    id: impl Into<String>,
    message: impl Into<String>,
    remediation: impl Into<String>,
) -> Finding {
    finding(id, Severity::Error, message, remediation)
}

fn warning(
    id: impl Into<String>,
    message: impl Into<String>,
    remediation: impl Into<String>,
) -> Finding {
    finding(id, Severity::Warning, message, remediation)
}

fn info(
    id: impl Into<String>,
    message: impl Into<String>,
    remediation: impl Into<String>,
) -> Finding {
    finding(id, Severity::Info, message, remediation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::harness::renumber;
    use crate::core::harness::root::CorpusRoot;
    use std::collections::BTreeSet;
    use std::path::Path;

    fn ids(findings: &[Finding]) -> BTreeSet<String> {
        findings.iter().map(|f| f.id.clone()).collect()
    }

    fn real() -> Corpus {
        Corpus::load(CorpusRoot::new(env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    /// The corpus this repo actually has is clean, and must stay that way:
    /// `just qc` runs this, so a finding here is a failing build.
    #[test]
    fn the_real_corpus_is_clean() {
        let findings = run(&real());
        assert!(
            findings.is_empty(),
            "unexpected findings: {:#?}",
            ids(&findings)
        );
    }

    /// LESSON-009's named enforcement, and two others, moved into `src/` in
    /// this phase. `lesson.unenforced` finds a `**Check:**` id by searching
    /// the source for it as a literal, so an id built by concatenation would
    /// break the lesson that names it — silently, since it would still
    /// compile and still pass every other test.
    #[test]
    fn every_mechanized_lesson_resolves_to_a_literal_in_the_source() {
        let corpus = real();
        let lessons = model::read_lessons(&corpus.root.learnings_file());
        let mechanized: Vec<_> = lessons
            .iter()
            .filter(|l| l.status.as_deref() == Some("mechanized"))
            .collect();
        assert!(!mechanized.is_empty(), "the corpus should have some");

        for lesson in mechanized {
            let check = lesson
                .check
                .as_deref()
                .expect("a mechanized lesson names one");
            assert!(
                emitted_somewhere(&corpus, check),
                "{} names '{check}', which no literal under src/ or scripts/ carries",
                lesson.id
            );
        }
    }

    // --- fixtures -----------------------------------------------------------
    // Built in a tempdir and run through *both* implementations. Comparing
    // two clean runs proves almost nothing; comparing them on a corpus that
    // is deliberately broken is what shows the port reproduces the incumbent.

    struct Fixture {
        dir: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join(".ash/plans")).unwrap();
            Self { dir }
        }

        fn path(&self) -> &Path {
            self.dir.path()
        }

        fn plan(&self, name: &str, frontmatter: &str) -> std::path::PathBuf {
            let dir = self.path().join(".ash/plans").join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("README.md"),
                format!("---\n{frontmatter}---\n\n# A plan\n"),
            )
            .unwrap();
            dir
        }

        /// A plan that is correct in every respect, as the baseline to break.
        fn good_plan(&self, id: &str, slug: &str) -> std::path::PathBuf {
            self.plan(
                &format!("{id}-{slug}"),
                &format!(
                    "id: {id}\nslug: {slug}\nstatus: Proposed\ncreated: 2026-09-19\n\
                     updated: 2026-09-19\nareas: [cli]\nsummary: A plan.\n"
                ),
            )
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }

        fn corpus(&self) -> Corpus {
            Corpus::load(CorpusRoot::new(self.path())).unwrap()
        }

        /// Generates the index so `index.missing` does not drown out the
        /// finding each fixture is actually about.
        fn indexed(&self) -> Corpus {
            let corpus = self.corpus();
            index::write(&corpus).unwrap();
            corpus
        }
    }

    /// Each fixture breaks exactly one invariant, and the checker must name
    /// it.
    ///
    /// Until the cutover these ran against `scripts/ash.sh` too, which is how
    /// the port was shown to reproduce the incumbent finding-for-finding —
    /// and it earned its keep, catching a plan-id scanner that matched
    /// nothing. That oracle went with the script; the fixtures stay, because
    /// what they guard now is this implementation against itself.
    #[test]
    fn each_broken_corpus_produces_its_finding() {
        /// One deliberately broken corpus, and the finding it must produce.
        struct Case {
            expected: &'static str,
            description: &'static str,
            build: fn(&Fixture),
        }
        fn case(expected: &'static str, description: &'static str, build: fn(&Fixture)) -> Case {
            Case {
                expected,
                description,
                build,
            }
        }

        let cases = vec![
            case(
                "plan.malformed-dir.not-a-plan-name",
                "a directory name that is not <yymmdd>-<6 letters>-<slug>",
                |f: &Fixture| {
                    f.plan("not-a-plan-name", "id: x\n");
                },
            ),
            case(
                "plan.duplicate-id.260919-qwerty",
                "two plans sharing one id",
                |f: &Fixture| {
                    f.good_plan("260919-qwerty", "first");
                    f.good_plan("260919-qwerty", "second");
                },
            ),
            case(
                "phase.chain-gap.260919-qwerty-thing",
                "phase files numbered 01 and 03",
                |f: &Fixture| {
                    let dir = f.good_plan("260919-qwerty", "thing");
                    for n in ["01", "03"] {
                        std::fs::write(
                            dir.join(format!("phase-{n}.md")),
                            format!(
                                "---\nid: 260919-qwerty\nslug: thing\nphase: {}\nstatus: Proposed\n---\n",
                                n.trim_start_matches('0')
                            ),
                        )
                        .unwrap();
                    }
                },
            ),
            case(
                "phase.status-mirror.260919-qwerty-thing.1",
                "a phase whose status disagrees with the README's table",
                |f: &Fixture| {
                    let dir = f.plan(
                        "260919-qwerty-thing",
                        "id: 260919-qwerty\nslug: thing\nstatus: In Progress\n\
                         created: 2026-09-19\nupdated: 2026-09-19\nareas: [cli]\n\
                         summary: A plan.\n",
                    );
                    std::fs::write(
                        dir.join("README.md"),
                        std::fs::read_to_string(dir.join("README.md")).unwrap()
                            + "\n## Phases\n| # | Phase | File | Status |\n\
                               |---|-------|------|--------|\n\
                               | 1 | One | [phase-01.md](phase-01.md) | Done |\n",
                    )
                    .unwrap();
                    std::fs::write(
                        dir.join("phase-01.md"),
                        "---\nid: 260919-qwerty\nslug: thing\nphase: 1\nstatus: Proposed\n---\n",
                    )
                    .unwrap();
                },
            ),
            case(
                "learnings.untriaged.260919-qwerty-thing.ISSUE-001",
                "an issue no lesson references and nobody declined",
                |f: &Fixture| {
                    let dir = f.good_plan("260919-qwerty", "thing");
                    std::fs::write(
                        dir.join("learnings.md"),
                        "# Learnings\n\n### ISSUE-001: Something went wrong\n\nIt did.\n",
                    )
                    .unwrap();
                },
            ),
            case(
                "lesson.unenforced.LESSON-001",
                "a mechanized lesson naming a check nothing emits",
                |f: &Fixture| {
                    f.good_plan("260919-qwerty", "thing");
                    f.write(
                        ".ash/LEARNINGS.md",
                        "# Distilled learnings\n\n### LESSON-001: A thing\n\
                         **Status:** mechanized\n**Check:** nothing.emits.this\n",
                    );
                },
            ),
            case(
                "lesson.duplicate-id.LESSON-001",
                "two lessons minted the same number, as happens across parallel branches",
                |f: &Fixture| {
                    f.good_plan("260919-qwerty", "thing");
                    f.write(
                        ".ash/LEARNINGS.md",
                        "# Distilled learnings\n\n### LESSON-001: First\n**Status:** prose\n\n\
                         ### LESSON-001: Second\n**Status:** prose\n",
                    );
                },
            ),
            case(
                "lesson.dangling-reference.LESSON-999",
                "a 'Seen in:' line citing a lesson number nothing defines",
                |f: &Fixture| {
                    f.good_plan("260919-qwerty", "thing");
                    f.write(
                        ".ash/LEARNINGS.md",
                        "# Distilled learnings\n\n### LESSON-001: A thing\n**Status:** prose\n\
                         **Seen in:** also see LESSON-999\n",
                    );
                },
            ),
        ];

        for Case {
            expected,
            description,
            build,
        } in cases
        {
            let fixture = Fixture::new();
            build(&fixture);
            let corpus = fixture.indexed();

            let ours = ids(&run(&corpus));
            assert!(
                ours.contains(expected),
                "{description}: expected {expected}, got {ours:?}"
            );
        }
    }

    /// The clean case, so the fixtures are not only ever exercised on broken
    /// input — a checker that fires on a healthy corpus is exactly how
    /// checkers get switched off.
    #[test]
    fn a_healthy_fixture_is_silent() {
        let fixture = Fixture::new();
        fixture.good_plan("260919-qwerty", "thing");
        assert!(run(&fixture.indexed()).is_empty());
    }

    /// Every `--title "..."` a remediation suggests, in the order it suggests
    /// them.
    fn suggested_titles(remediation: &str) -> Vec<String> {
        remediation
            .split("--title \"")
            .skip(1)
            .filter_map(|rest| rest.split_once('"').map(|(title, _)| title.to_string()))
            .collect()
    }

    /// The remedy is mechanized now too, so the finding has to name the
    /// command rather than sending the reader back to grepping the corpus —
    /// the half of LESSON-022 the detection alone left open.
    ///
    /// And the command it names has to *work*. This runs `renumber::locate`
    /// with the exact `--title` the remediation hands the reader, because the
    /// first version of this test only asserted the string contained some
    /// words: it named the shared id, which is the one thing a title can never
    /// contain, so following it failed every time and every assertion passed
    /// anyway (LESSON-034).
    #[test]
    fn the_duplicate_id_remediation_suggests_a_title_that_resolves() {
        let learnings = "# Distilled learnings\n\n\
             ### LESSON-001: The one that reached main first\n**Status:** prose\n\n\
             ### LESSON-001: The one the branch minted\n**Status:** prose\n\n\
             ### LESSON-002: An unrelated neighbour\n**Status:** prose\n";
        let fixture = Fixture::new();
        fixture.good_plan("260919-qwerty", "thing");
        fixture.write(".ash/LEARNINGS.md", learnings);

        let findings = run(&fixture.indexed());
        let duplicates: Vec<&Finding> = findings
            .iter()
            .filter(|f| f.id == "lesson.duplicate-id.LESSON-001")
            .collect();
        assert_eq!(duplicates.len(), 1, "one finding per colliding id");
        let duplicate = duplicates[0];

        // Both colliding headings are named, by the thing that tells them
        // apart rather than by the number they share.
        assert!(
            duplicate
                .message
                .contains("The one that reached main first")
                && duplicate.message.contains("The one the branch minted"),
            "{}",
            duplicate.message
        );

        let titles = suggested_titles(&duplicate.remediation);
        assert_eq!(titles.len(), 2, "{}", duplicate.remediation);
        assert!(
            duplicate
                .remediation
                .contains("pinst harness renumber-lesson"),
            "{}",
            duplicate.remediation
        );

        // The actual bug: run what the remediation told the reader to run.
        let headings = renumber::headings(learnings);
        for title in &titles {
            let found = renumber::locate(&headings, title)
                .unwrap_or_else(|err| panic!("--title {title:?} does not resolve: {err:#}"));
            assert_eq!(found.id, "LESSON-001", "--title {title:?}");
        }
        // ...and the two suggestions are not the same heading twice.
        assert_ne!(
            renumber::locate(&headings, &titles[0]).unwrap().line,
            renumber::locate(&headings, &titles[1]).unwrap().line
        );
    }

    /// Three headings on one number are one problem, and the reader is
    /// offered all three titles rather than having to find the third.
    #[test]
    fn a_three_way_collision_is_one_finding_naming_all_three() {
        let fixture = Fixture::new();
        fixture.good_plan("260919-qwerty", "thing");
        fixture.write(
            ".ash/LEARNINGS.md",
            "# Distilled learnings\n\n\
             ### LESSON-001: First\n**Status:** prose\n\n\
             ### LESSON-001: Second\n**Status:** prose\n\n\
             ### LESSON-001: Third\n**Status:** prose\n",
        );

        let findings = run(&fixture.indexed());
        let duplicates: Vec<&Finding> = findings
            .iter()
            .filter(|f| f.id == "lesson.duplicate-id.LESSON-001")
            .collect();
        assert_eq!(duplicates.len(), 1);
        assert_eq!(suggested_titles(&duplicates[0].remediation).len(), 3);
    }

    /// The remediation is only useful if the titles in the real file are
    /// distinguishable, so the corpus itself is the fixture: every lesson has
    /// a title, and passing that title to the command resolves to that one
    /// heading and no other.
    #[test]
    fn every_lesson_in_the_corpus_is_reachable_by_its_own_title() {
        let corpus = real();
        let text = std::fs::read_to_string(corpus.root.learnings_file()).unwrap();
        let headings = renumber::headings(&text);
        assert!(headings.len() >= 30, "found only {}", headings.len());

        for heading in &headings {
            assert!(!heading.title.is_empty(), "{} has no title", heading.id);
            let found = renumber::locate(&headings, &heading.title)
                .unwrap_or_else(|err| panic!("{}: {err:#}", heading.id));
            assert_eq!(found.line, heading.line, "{}", heading.id);
        }
    }

    /// The staleness checks are the ones that compare the corpus against a
    /// record written after the fact, and the only ones that can catch a plan
    /// still advertising work that shipped days ago.
    #[test]
    fn the_changelog_contradicts_an_unfinished_phase() {
        let fixture = Fixture::new();
        let dir = fixture.good_plan("260919-qwerty", "thing");
        std::fs::write(
            dir.join("phase-01.md"),
            "---\nid: 260919-qwerty\nslug: thing\nphase: 1\nstatus: In Progress\n---\n",
        )
        .unwrap();
        fixture.write(
            ".ash/CHANGELOG.log",
            "ts=2026-09-19T10:42:03Z id=260919-qwerty slug=thing phase=1 summary=\"shipped\"\n",
        );
        let corpus = fixture.indexed();

        let ours = ids(&run(&corpus));
        assert!(
            ours.contains("phase.logged-not-done.260919-qwerty-thing.1"),
            "{ours:?}"
        );
    }

    /// The finding this port adds: a document the strict parser refuses. The
    /// awk reader tolerated it silently, so there is nothing to compare
    /// against here — that is the point of adding it.
    #[test]
    fn a_document_outside_the_accepted_subset_is_reported() {
        let fixture = Fixture::new();
        fixture.plan(
            "260919-qwerty-thing",
            "id: 260919-qwerty\nmeta:\n  nested: true\n",
        );
        let ours = ids(&run(&fixture.indexed()));
        assert!(
            ours.contains("plan.frontmatter-unparsed.260919-qwerty-thing"),
            "{ours:?}"
        );
    }
}
