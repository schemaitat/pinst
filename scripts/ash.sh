#!/usr/bin/env bash
# Maintains and validates the .ash/ plan corpus that the plan-write,
# plan-implement and plan-learn skills read and write.
#
#   scripts/ash.sh index [--check]   regenerate (or verify) .ash/INDEX.md
#   scripts/ash.sh check             validate every corpus invariant
#   scripts/ash.sh skills            grade each skill against its own contract
#     ... --transcripts DIR          also count invocations (opt-in, off by default)
#   scripts/ash.sh new-id [DATE]     mint a plan id: <yymmdd>-<6 letters>
#
# Why this exists: the skills state a dozen invariants in prose ("the id must
# match its directory", "never hand-edit INDEX.md", "mirror phase status").
# Prose is enforced by remembering. This is enforced by an exit code — the
# same contract pinst itself offers agents:
#
#   0  clean          2  usage error
#   1  failed to run  3  ran fine, found things to act on
set -euo pipefail

ASH_DIR="${ASH_DIR:-.ash}"
PLANS_DIR="$ASH_DIR/plans"
INDEX_FILE="$ASH_DIR/INDEX.md"
LOG_FILE="$ASH_DIR/CHANGELOG.log"
LEARNINGS_FILE="$ASH_DIR/LEARNINGS.md"
JSON=0

FIND_ID=(); FIND_SEV=(); FIND_MSG=(); FIND_REM=()

# Spliced into the JSON envelope by report(), for subcommands that carry a
# payload as well as findings. Must end in a comma when set.
EXTRA_JSON=""

SKILLS_DIR="${SKILLS_DIR:-.agents/skills}"
WIRED_DIR="${WIRED_DIR:-.claude/skills}"
SKILLS_WINDOW="${ASH_SKILLS_WINDOW:-20}"
COMMANDS_DIR="${COMMANDS_DIR:-.claude/commands}"
TRANSCRIPTS=""

finding() { FIND_ID+=("$1"); FIND_SEV+=("$2"); FIND_MSG+=("$3"); FIND_REM+=("$4"); }

# `${!arr[@]}` on an empty array trips `set -u` on older bash, and "no
# findings" is the expected case, so indices come from here instead.
find_indices() { [ "${#FIND_ID[@]}" -gt 0 ] && printf '%s\n' "${!FIND_ID[@]}"; return 0; }

usage() {
  sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
  exit 2
}

# --- frontmatter ------------------------------------------------------------
# The block between the first two `---` lines. Everything downstream treats a
# file with no such block as having no frontmatter at all, which is itself a
# finding rather than a crash.

fm_block() {
  awk 'NR==1 && $0!="---" {exit} NR==1 {next} /^---[[:space:]]*$/ {exit} {print}' "$1"
}

fm() { fm_block "$1" | awk -v k="$2" -F': *' '$1==k {sub(/^[^:]*: */,""); print; exit}'; }

fm_raw() { fm_block "$1" | awk -v k="$2" '$0 ~ "^"k":" {sub(/^[^:]*:[[:space:]]*/,""); print; exit}'; }

unquote() { local v="$1"; v="${v%\"}"; v="${v#\"}"; printf '%s' "$v"; }

# Frontmatter values that hold shell commands are single-quoted YAML scalars,
# where an embedded quote is written twice. Double quotes would not do: the
# commands are full of backslashes, and YAML treats those as escapes inside
# double quotes but as literals inside single ones.
sq_unquote() {
  local v="$1"
  case "$v" in
    "'"*"'") v="${v#\'}"; v="${v%\'}"; v="$(printf '%s' "$v" | sed "s/''/'/g")" ;;
    *) v="$(unquote "$v")" ;;
  esac
  printf '%s' "$v"
}

# `[a, b, c]` -> `a, b, c`
delist() { local v="$1"; v="${v#[}"; v="${v%]}"; printf '%s' "$v"; }

# Finding ids are match keys, so they carry the corpus-relative path, never
# the absolute one — the id must not change with the checkout location.
rel() { printf '%s' "${1#"$PLANS_DIR"/}"; }

jesc() { printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e 's/\t/\\t/g'; }

# --- plan ids ---------------------------------------------------------------
# A plan id is <yymmdd>-<six lowercase letters>, e.g. 260919-qwerty.
#
# The hash half is random, not allocated: work happens in parallel git
# worktrees branched from the same commit, so any "next number" scheme has two
# sessions computing the same answer and colliding at merge. Nothing that
# needs to ask the rest of the corpus a question can be a stable identifier
# here. Six letters is 26^6 ~= 309M, which is ample for a corpus of dozens.
#
# The date half is most-significant-first so that string order *is* date
# order: `ls` comes out chronological, and so does the generated index. This
# is the whole reason it is yymmdd and not the friendlier-looking ddmmyy —
# under ddmmyy, 011026 (1 Oct) would sort before 180926 (18 Sep).

mint_id() {
  local date_part="${1:-}"
  if [ -n "$date_part" ]; then
    # Accept an ISO date (YYYY-MM-DD) and render it ddmmyy.
    date_part="$(printf '%s' "$date_part" | awk -F- '{printf "%s%s%s", substr($1,3,2), $2, $3}')"
  else
    date_part="$(date -u +%y%m%d)"
  fi
  # Read a fixed block and filter it, rather than piping an endless
  # /dev/urandom into `head -c` — that closes the pipe early, and under
  # `pipefail` the resulting SIGPIPE fails the whole command.
  local hash=""
  while [ "${#hash}" -lt 6 ]; do
    hash="$hash$(LC_ALL=C head -c 1024 /dev/urandom | LC_ALL=C tr -dc 'a-z')"
  done
  printf '%s-%s\n' "$date_part" "${hash:0:6}"
}

# --- collection -------------------------------------------------------------

plan_dirs() {
  [ -d "$PLANS_DIR" ] || return 0
  find "$PLANS_DIR" -mindepth 1 -maxdepth 1 -type d | sort
}

# Every check below appends findings; none of them exit early, so one run
# reports the whole state of the corpus rather than its first problem.
check_corpus() {
  local seen_ids=""

  if [ ! -d "$PLANS_DIR" ]; then
    finding "corpus.missing" error "$PLANS_DIR does not exist" \
      "create it, or run plan-write to author the first plan"
    return
  fi

  local d name id slug rest readme
  while IFS= read -r d; do
    [ -n "$d" ] || continue
    name="$(basename "$d")"

    if ! printf '%s' "$name" | grep -qE '^[0-9]{6}-[a-z]{6}-[a-z0-9]+(-[a-z0-9]+)*$'; then
      finding "plan.malformed-dir.$name" error \
        "plan directory '$name' is not <ddmmyy>-<6 letters>-<kebab-slug>" \
        "rename it, e.g. $(mint_id)-$name"
      continue
    fi
    rest="${name#*-}"
    id="${name%%-*}-${rest%%-*}"
    slug="${rest#*-}"

    case " $seen_ids " in
      *" $id "*) finding "plan.duplicate-id.$id" error \
        "id $id is used by more than one plan directory" \
        "mint a fresh id for the later plan: scripts/ash.sh new-id" ;;
    esac
    seen_ids="$seen_ids $id"

    readme="$d/README.md"
    if [ ! -f "$readme" ]; then
      finding "plan.readme-missing.$name" error "$readme is missing" \
        "every plan folder needs a README.md, single- or multi-phase"
      continue
    fi

    check_plan_readme "$d" "$name" "$id" "$slug" "$readme"
    check_phases "$d" "$name" "$id" "$slug" "$readme"
    check_learnings "$d" "$name"
    check_changelog "$d" "$name" "$id"
    check_distillation "$d" "$name" "$id"
  done <<< "$(plan_dirs)"
}

check_plan_readme() {
  local d="$1" name="$2" id="$3" slug="$4" readme="$5"

  if [ -z "$(fm_block "$readme")" ]; then
    finding "plan.frontmatter-missing.$name" error \
      "$readme has no YAML frontmatter" \
      "add the frontmatter block from the plan-write skill (Step 5)"
    return
  fi

  local k
  for k in id slug status created updated areas summary; do
    [ -n "$(fm_raw "$readme" "$k")" ] || finding "plan.frontmatter-key.$name.$k" error \
      "$readme frontmatter is missing '$k'" \
      "add '$k:' — .ash/INDEX.md is generated from these keys"
  done

  local fm_id
  fm_id="$(fm_raw "$readme" id)"
  [ -z "$fm_id" ] || [ "$(unquote "$fm_id")" = "$id" ] || \
    finding "plan.id-mismatch.$name" error \
      "$readme frontmatter id '$(unquote "$fm_id")' does not match directory id '$id'" \
      "set id: $id"

  local fm_slug
  fm_slug="$(fm_raw "$readme" slug)"
  [ -z "$fm_slug" ] || [ "$fm_slug" = "$slug" ] || finding "plan.slug-mismatch.$name" error \
    "$readme frontmatter slug '$fm_slug' does not match directory slug '$slug'" \
    "set slug: $slug"

  check_status_value "$readme" "plan.status-invalid.$name"
  check_no_status_section "$readme"
}

check_status_value() {
  local file="$1" id="$2" st
  st="$(fm_raw "$file" status)"
  case "$st" in
    Proposed|"In Progress"|Done|"") : ;;
    *) finding "$id" error "$file has status '$st'" \
         "use one of: Proposed, In Progress, Done" ;;
  esac
}

# The skills forbid a `## Status` section: status lives in frontmatter only,
# because two copies of a mutable field is two copies to drift apart.
check_no_status_section() {
  local file="$1"
  grep -qE '^## Status[[:space:]]*$' "$file" || return 0
  finding "plan.status-section.$(rel "$file")" warning \
    "$file has a '## Status' section as well as frontmatter status" \
    "delete the section; frontmatter status is authoritative"
}

check_phases() {
  local d="$1" name="$2" id="$3" slug="$4" readme="$5"
  local phases n expected=1 file pnum pstatus table_status

  phases="$(find "$d" -mindepth 1 -maxdepth 1 -name 'phase-*.md' | sort)"
  [ -n "$phases" ] || return 0

  while IFS= read -r file; do
    [ -n "$file" ] || continue
    n="$(basename "$file" .md)"; n="${n#phase-}"

    if ! printf '%s' "$n" | grep -qE '^[0-9]{2}$'; then
      finding "phase.malformed-name.$(rel "$file")" error \
        "$file is not phase-<two digits>.md" "rename it, e.g. phase-0$((expected)).md"
      continue
    fi

    # Phases are a strictly linear chain, so their numbers must be 1..N with
    # no gaps — a gap means a phase file was lost or never written.
    if [ "$((10#$n))" -ne "$expected" ]; then
      finding "phase.chain-gap.$name" error \
        "$file breaks the phase chain: expected phase $expected" \
        "renumber the phase files so they run 01..N with no gaps"
    fi
    expected=$((expected + 1))

    if [ -z "$(fm_block "$file")" ]; then
      finding "phase.frontmatter-missing.$(rel "$file")" error \
        "$file has no YAML frontmatter" \
        "add index/slug/phase/status per the plan-write skill (Step 4)"
      continue
    fi

    local k
    for k in id slug phase status; do
      [ -n "$(fm_raw "$file" "$k")" ] || finding "phase.frontmatter-key.$(rel "$file").$k" error \
        "$file frontmatter is missing '$k'" "add '$k:'"
    done

    [ "$(unquote "$(fm_raw "$file" id)")" = "$id" ] || finding "phase.id-mismatch.$(rel "$file")" error \
      "$file frontmatter id does not match its plan's id '$id'" "set id: $id"

    pnum="$(fm_raw "$file" phase)"
    [ -z "$pnum" ] || [ "$pnum" = "$((10#$n))" ] || finding "phase.number-mismatch.$(rel "$file")" error \
      "$file frontmatter says phase $pnum but the filename says $((10#$n))" \
      "make them agree"

    check_status_value "$file" "phase.status-invalid.$(rel "$file")"
    check_no_status_section "$file"

    # The README's ## Phases table mirrors each phase's status for readability;
    # whoever flips one must flip both, so disagreement is a finding.
    pstatus="$(fm_raw "$file" status)"
    table_status="$(awk -F'|' -v want="$((10#$n))" '
      /^\|[[:space:]]*[0-9]+[[:space:]]*\|/ {
        num=$2; gsub(/[[:space:]]/,"",num)
        if (num == want) { st=$5; gsub(/^[[:space:]]+|[[:space:]]+$/,"",st); print st; exit }
      }' "$readme")"
    if [ -n "$table_status" ] && [ -n "$pstatus" ] && [ "$table_status" != "$pstatus" ]; then
      finding "phase.status-mirror.$name.$((10#$n))" warning \
        "phase $((10#$n)) is '$pstatus' in $file but '$table_status' in the README's Phases table" \
        "update the table row to '$pstatus'"
    fi
  done <<< "$phases"

  # A plan cannot be Done while a phase it depends on is not.
  if [ "$(fm_raw "$readme" status)" = "Done" ]; then
    while IFS= read -r file; do
      [ -n "$file" ] || continue
      [ "$(fm_raw "$file" status)" = "Done" ] || finding "plan.done-with-open-phase.$name" error \
        "$name is marked Done but $file is not" \
        "finish the phase, or set the plan back to In Progress"
    done <<< "$phases"
  # ...and the converse, which is the cheaper half to forget: when the last
  # phase closes, the plan is over. Left In Progress it goes on advertising
  # work that has already shipped, and the longer it does the more expensive
  # it is for the next reader to tell the difference.
  else
    local open=0
    while IFS= read -r file; do
      [ -n "$file" ] || continue
      [ "$(fm_raw "$file" status)" = "Done" ] || open=1
    done <<< "$phases"
    [ "$open" -eq 1 ] || finding "plan.phases-all-done.$name" warning \
      "every phase of $name is Done but the plan is '$(fm_raw "$readme" status)'" \
      "close the plan (status: Done) and run the plan-learn skill, or reopen the phase that is not finished"
  fi
}

# --- the shipped log --------------------------------------------------------
# .ash/CHANGELOG.log is append-only and one line per shipped phase, so it is
# the only part of the corpus that records what actually reached main. That
# makes it the one thing a stale plan can be checked against without asking
# GitHub anything: a log line saying a phase shipped and a phase file still
# saying In Progress cannot both be true, and the log is the half that cannot
# be wrong retroactively.
#
# This is the check that was missing when plan 260919-zeuuaj sat In Progress
# for a day with its own log recording all four phases as shipped.

check_changelog() {
  local d="$1" name="$2" id="$3"
  [ -f "$LOG_FILE" ] || return 0

  local status logged
  status="$(fm_raw "$d/README.md" status)"
  logged="$(grep -F "id=$id " "$LOG_FILE" || true)"

  if [ "$status" = "Done" ] && [ -z "$logged" ]; then
    finding "plan.unlogged.$name" warning \
      "$name is Done but $LOG_FILE has no entry for id=$id" \
      "append the closing line plan-implement writes when a plan ships"
  fi

  # Only this direction is an invariant. A phase with no log line may simply
  # predate the log; a log line with no finished phase is drift.
  local n file pstatus
  while IFS= read -r n; do
    [ -n "$n" ] || continue
    file="$(printf '%s/phase-%02d.md' "$d" "$((10#$n))")"
    if [ ! -f "$file" ]; then
      finding "phase.logged-missing.$name.$n" warning \
        "$LOG_FILE records phase $n of $id as shipped but $file does not exist" \
        "restore the phase file, or correct the id in the log entry"
      continue
    fi
    pstatus="$(fm_raw "$file" status)"
    [ "$pstatus" = "Done" ] || finding "phase.logged-not-done.$name.$n" warning \
      "$LOG_FILE says phase $n of $id shipped, but $file is '$pstatus'" \
      "set that phase to Done, or work out what the log entry was really about"
  done <<< "$(printf '%s\n' "$logged" | grep -oE 'phase=[0-9]+' | cut -d= -f2 | sort -un || true)"
}

# plan-learn's contract: a missing learnings.md and a clean run must look
# different to whoever checks later, so a finished or attempted plan has one.
check_learnings() {
  local d="$1" name="$2"
  local status; status="$(fm_raw "$d/README.md" status)"
  [ ! -f "$d/learnings.md" ] || return 0
  if [ "$status" = "Done" ]; then
    finding "plan.learnings-missing.$name" warning \
      "$name is Done but has no learnings.md" \
      "run the plan-learn skill for $name"
  elif [ -d "$d/logs" ] && [ -n "$(find "$d/logs" -type f -print -quit)" ]; then
    finding "plan.learnings-missing.$name" info \
      "$name has implementation logs but no learnings.md yet" \
      "run the plan-learn skill for $name when the run ends"
  fi
}

# --- distillation -----------------------------------------------------------
# A plan closing is what starts the clock on distilling what it taught. These
# two checks are the clock: an issue nobody has decided about keeps `qc` red,
# and a lesson claiming to be enforced has to name a check that exists.
#
# Deliberately no wall-clock schedule. The trigger is a plan closing, not a
# Tuesday — a weekly job fires into silence on a quiet week, misses four plans
# on a busy one, and lives in one person's account rather than in the repo.

# Every (plan id, issue id) pair referenced from a lesson's `Seen in:` line.
# The line is free prose and often wraps, so the whole tail of the block is
# buffered and both kinds of id are pulled out of it. A buffer naming two
# plans yields the cross-product, which can only *silence* a finding — the
# safe direction, given a check that cries wolf is a check that gets deleted.
#
# No `{n}` intervals in the patterns: mawk has not always supported them, and
# these scripts are meant to run on a machine that has just been bootstrapped.
seen_pairs() {
  [ -f "$LEARNINGS_FILE" ] || return 0
  awk '
    function flush(   tmp, n, m, i, j) {
      if (buf == "") return
      n = 0; m = 0
      tmp = buf
      while (match(tmp, /[0-9][0-9][0-9][0-9][0-9][0-9]-[a-z][a-z][a-z][a-z][a-z][a-z]/)) {
        plans[++n] = substr(tmp, RSTART, RLENGTH)
        tmp = substr(tmp, RSTART + RLENGTH)
      }
      tmp = buf
      while (match(tmp, /ISSUE-[0-9]+/)) {
        issues[++m] = substr(tmp, RSTART, RLENGTH)
        tmp = substr(tmp, RSTART + RLENGTH)
      }
      for (i = 1; i <= n; i++) for (j = 1; j <= m; j++) print plans[i], issues[j]
      buf = ""; delete plans; delete issues
    }
    /^### LESSON-/ { flush(); inseen = 0 }
    /^\*\*Seen in:\*\*/ { inseen = 1 }
    inseen { buf = buf " " $0 }
    /^[[:space:]]*$/ { if (inseen) { flush(); inseen = 0 } }
    END { flush() }
  ' "$LEARNINGS_FILE"
}

# "ISSUE-00N open|declined" for one learnings.md.
issue_states() {
  [ -f "$1" ] || return 0
  awk '
    function flush() { if (cur != "") print cur, (dec ? "declined" : "open"); cur = ""; dec = 0 }
    /^### ISSUE-/ { flush(); cur = $2; sub(/:$/, "", cur); next }
    /^\*\*Distilled:\*\*[[:space:]]*declined/ { if (cur != "") dec = 1 }
    /^## / { flush() }
    END { flush() }
  ' "$1"
}

check_distillation() {
  local d="$1" name="$2" id="$3"
  local lf="$d/learnings.md"
  [ -f "$lf" ] || return 0

  local pairs state issue st
  pairs="$(seen_pairs)"

  while IFS= read -r state; do
    [ -n "$state" ] || continue
    issue="${state%% *}"; st="${state##* }"
    [ "$st" = "open" ] || continue
    printf '%s\n' "$pairs" | grep -qx "$id $issue" && continue
    finding "learnings.untriaged.$name.$issue" warning \
      "$lf records $issue but no lesson in $LEARNINGS_FILE references it" \
      "run the plan-learn skill: promote it, add it to an existing lesson'\''s 'Seen in:' line, or write '\''**Distilled:** declined — <reason>'\'' on the issue"
  done <<< "$(issue_states "$lf")"
}

# A lesson may be mechanized by either checker — ash.sh owns the corpus
# invariants, agents-wire.sh owns the skill projection — so the id is looked
# for across scripts/ rather than in this file alone.
check_lessons() {
  [ -f "$LEARNINGS_FILE" ] || return 0
  local line lesson st ck
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    lesson="$(printf '%s' "$line" | cut -d" " -f1)"
    st="$(printf '%s' "$line" | cut -d" " -f2)"
    ck="$(printf '%s' "$line" | cut -d" " -f3)"
    [ "$st" = "mechanized" ] || continue
    if [ -z "$ck" ] || [ "$ck" = "-" ]; then
      finding "lesson.unenforced.$lesson" error \
        "$LEARNINGS_FILE marks $lesson mechanized but names no '\''**Check:**'\'' finding id" \
        "add the finding id that enforces it, or set '\''**Status:** prose'\''"
      continue
    fi
    grep -rqF "$ck" scripts/ 2>/dev/null && continue
    finding "lesson.unenforced.$lesson" error \
      "$LEARNINGS_FILE says $lesson is mechanized by '\''$ck'\'', which no script under scripts/ emits" \
      "point '\''**Check:**'\'' at a finding id that exists, or set '\''**Status:** prose'\''"
  done <<< "$(lesson_states)"
}

# "LESSON-00N status check-or-dash" for every lesson.
lesson_states() {
  [ -f "$LEARNINGS_FILE" ] || return 0
  awk '
    function flush() { if (cur != "") print cur, (st == "" ? "-" : st), (ck == "" ? "-" : ck); cur = ""; st = ""; ck = "" }
    /^### LESSON-/ { flush(); cur = $2; sub(/:$/, "", cur); next }
    /^\*\*Status:\*\*/ { st = $2 }
    /^\*\*Check:\*\*/ { ck = $2 }
    END { flush() }
  ' "$LEARNINGS_FILE"
}

count_lessons() { lesson_states | grep -c . || true; }
count_issues() {
  local n=0 f
  for f in "$PLANS_DIR"/*/learnings.md; do
    [ -f "$f" ] || continue
    n=$((n + $(issue_states "$f" | grep -c . || true)))
  done
  printf '%s' "$n"
}

# --- index ------------------------------------------------------------------

render_index() {
  printf '# Plan index\n\n'
  printf '<!-- Generated by scripts/ash.sh index from .ash/plans/*/README.md frontmatter. Do not hand-edit. -->\n\n'
  printf '| Plan | ID | Status | Updated | Areas | Summary |\n'
  printf '|------|----|--------|---------|-------|---------|\n'
  # No sort key needed: plan ids start with a yymmdd date, so plan_dirs'
  # plain name sort is already chronological.
  local d name id slug rest readme
  while IFS= read -r d; do
    [ -n "$d" ] || continue
    name="$(basename "$d")"
    printf '%s' "$name" | grep -qE '^[0-9]{6}-[a-z]{6}-' || continue
    readme="$d/README.md"
    [ -f "$readme" ] || continue
    rest="${name#*-}"
    id="${name%%-*}-${rest%%-*}"
    slug="${rest#*-}"
    printf '| [%s](plans/%s/README.md) | %s | %s | %s | %s | %s |\n' \
      "$slug" "$name" "$id" \
      "$(fm_raw "$readme" status)" "$(fm_raw "$readme" updated)" \
      "$(delist "$(fm_raw "$readme" areas)")" "$(fm_raw "$readme" summary)"
  done <<< "$(plan_dirs)"
}

# --- the skill audit --------------------------------------------------------
# Skills are graded on the artifacts they leave in the repo, never on whether
# anyone invoked them. A skill declares in its own frontmatter what it
# produces and the command that measures it, because the skill is the only
# place that knows what it is for — put the measure in this script and the two
# drift, which is the failure this whole audit exists to catch.
#
# Nothing in here belongs in `check`. These are rates and tallies, and
# report() exits 3 on a finding of any severity, so a number that got
# interesting would fail `just qc`. Findings here are structural only: a skill
# with no contract, or a measure that would not run.

# How many recorded issues name each skill, from the `**Skill:**` line that
# plan-learnings writes. Prints "name count" per skill that appears.
skill_tally() {
  local f
  for f in "$PLANS_DIR"/*/learnings.md; do
    [ -f "$f" ] || continue
    awk '/^\*\*Skill:\*\*/ { print $2 }' "$f"
  done | sort | uniq -c | awk '{ print $2, $1 }'
}

# Runs one `evidence:` command and echoes "<conforming> <total>", or nothing
# when it could not be measured. ASH_RANGE is git-ready ("-n 20" or empty) and
# ASH_WINDOW is the bare number, so a command can use whichever fits it.
run_evidence() {
  local cmd="$1" window="$2" out rc
  local runner=(sh -c "$cmd")
  command -v timeout >/dev/null 2>&1 && runner=(timeout 30 sh -c "$cmd")
  set +e
  out="$(ASH_WINDOW="$window" ASH_RANGE="${window:+-n $window}" "${runner[@]}" 2>/dev/null)"
  rc=$?
  set -e
  [ "$rc" -eq 0 ] || return 1
  printf '%s' "$out" | grep -qE '^[[:space:]]*[0-9]+[[:space:]]+[0-9]+[[:space:]]*$' || return 1
  printf '%s' "$out" | tr -s '[:space:]' ' ' | sed 's/^ //; s/ $//'
}

pct() {
  local n="$1" d="$2"
  [ "$d" -gt 0 ] 2>/dev/null || { printf 'n/a'; return; }
  printf '%d%%' $(( n * 100 / d ))
}

# --- invocation evidence (opt-in) -------------------------------------------
# Counts how often each skill was actually invoked, from a runtime's own
# session transcripts. Off unless --transcripts names a directory, and never
# depended on by anything in `qc`: the path reads an undocumented format owned
# by someone else's release cycle, outside the repo, on one vendor's machine.
#
# It is a second opinion, never the system of record. Counting invocations
# alone would have graded plan-implement as dead — it shows zero invocations
# across every transcript here while having written a complete run log.
#
# Two record shapes, because a skill has two front doors: a `Skill` tool call
# carrying `input.skill`, and a slash command, which appears as a
# <command-name> marker in user content. Counting only the first misses /cc
# and /pr entirely.
#
# SEC-001: the only strings this prints are skill names it already knew and
# ISO dates. A name is emitted only after matching the set of directories in
# .agents/skills, so nothing typed into a conversation can reach stdout, and
# nothing here writes to .ash/.
transcript_counts() {
  local dir="$1"
  [ -d "$dir" ] || return 1
  command -v python3 >/dev/null 2>&1 || return 1
  python3 - "$dir" "$SKILLS_DIR" "$COMMANDS_DIR" <<'PYEOF' 2>/dev/null
import collections, glob, json, os, re, sys

root, skills_dir, cmds_dir = sys.argv[1:4]
try:
    known = {d for d in os.listdir(skills_dir)
             if os.path.isdir(os.path.join(skills_dir, d))}
except OSError:
    sys.exit(1)

# A command file names its skill by the path it tells the agent to read.
cmd_map = {}
for f in glob.glob(os.path.join(cmds_dir, "*.md")):
    try:
        text = open(f, errors="replace").read()
    except OSError:
        continue
    m = re.search(r"\.agents/skills/([a-z0-9-]+)/", text)
    if m and m.group(1) in known:
        cmd_map[os.path.basename(f)[:-3]] = m.group(1)

MARKER = re.compile(r"<command-name>/?([a-z0-9:_-]+)</command-name>")
counts, last = collections.Counter(), {}

def bump(skill, day):
    counts[skill] += 1
    if day and (skill not in last or day > last[skill]):
        last[skill] = day

def scan_text(text, day):
    for name in MARKER.findall(text or ""):
        if name in cmd_map:
            bump(cmd_map[name], day)

for path in glob.glob(os.path.join(root, "*", "*.jsonl")):
    try:
        fh = open(path, errors="replace")
    except OSError:
        continue
    with fh:
        for line in fh:
            if "Skill" not in line and "command-name" not in line:
                continue
            try:
                rec = json.loads(line)
            except Exception:
                continue
            if not isinstance(rec, dict):
                continue
            day = (rec.get("timestamp") or "")[:10]
            content = (rec.get("message") or {}).get("content")
            if isinstance(content, str):
                scan_text(content, day)
            elif isinstance(content, list):
                for c in content:
                    if not isinstance(c, dict):
                        continue
                    if c.get("type") == "tool_use" and c.get("name") == "Skill":
                        skill = (c.get("input") or {}).get("skill")
                        if skill in known:
                            bump(skill, day)
                    elif c.get("type") == "text":
                        scan_text(c.get("text"), day)

for skill in sorted(counts):
    print(skill, counts[skill], last.get(skill, "-"))
PYEOF
}

cmd_skills() {
  local rows_json="" sep="" dir name file produces evidence kind wired
  local win all wn wt an at
  local counts="" have_counts=0

  if [ -n "$TRANSCRIPTS" ]; then
    if counts="$(transcript_counts "$TRANSCRIPTS")"; then
      have_counts=1
    else
      finding "skills.transcripts-unreadable" warning \
        "could not read invocation counts from $TRANSCRIPTS" \
        "check the path, or drop --transcripts — nothing else depends on it"
      have_counts=2
    fi
  fi

  if [ ! -d "$SKILLS_DIR" ]; then
    finding "skills.missing" error "$SKILLS_DIR does not exist" \
      "run this from the repo root"
    return
  fi

  if [ "$JSON" -ne 1 ]; then
    if [ "$have_counts" -eq 0 ]; then
      printf '%-22s %-6s %-22s %-22s %s\n' \
        "skill" "wired" "conforming (last $SKILLS_WINDOW)" "all-time" "issues"
    else
      printf '%-22s %-6s %-22s %-22s %-7s %-6s %s\n' \
        "skill" "wired" "conforming (last $SKILLS_WINDOW)" "all-time" "issues" "fired" "last seen"
    fi
  fi

  while IFS= read -r dir; do
    [ -n "$dir" ] || continue
    name="$(basename "$dir")"
    file="$dir/SKILL.md"
    [ -f "$file" ] || continue

    produces="$(sq_unquote "$(fm_raw "$file" produces)")"
    evidence="$(sq_unquote "$(fm_raw "$file" evidence)")"
    kind="$(sq_unquote "$(fm_raw "$file" kind)")"
    wired="no"; [ -e "$WIRED_DIR/$name" ] && wired="yes"

    local tally
    tally="$(skill_tally | awk -v n="$name" '$1 == n { print $2 }')"
    [ -n "$tally" ] || tally=0

    if [ -z "$produces" ]; then
      finding "skill.no-contract.$name" warning \
        "$file declares no 'produces:' — nothing says what this skill is supposed to leave behind" \
        "add 'produces:' and 'evidence:' to its frontmatter, or 'kind: reference' if it produces nothing"
      win="no contract"; all="no contract"; wn=0; wt=0; an=0; at=0
    elif [ "$produces" = "none" ] || [ "$kind" = "reference" ]; then
      win="reference (exempt)"; all="-"; wn=0; wt=0; an=0; at=0
    elif [ -z "$evidence" ]; then
      finding "skill.no-contract.$name" warning \
        "$file says what it produces but gives no 'evidence:' command to measure it" \
        "add an 'evidence:' one-liner printing '<conforming> <total>'"
      win="no measure"; all="no measure"; wn=0; wt=0; an=0; at=0
    else
      local r
      if r="$(run_evidence "$evidence" "$SKILLS_WINDOW")"; then
        wn="${r%% *}"; wt="${r##* }"; win="$wn/$wt $(pct "$wn" "$wt")"
      else
        finding "skill.evidence-failed.$name" warning \
          "the 'evidence:' command for $name did not run, or printed something other than two integers" \
          "run the 'evidence:' one-liner in $file by hand and see what it prints"
        wn=0; wt=0; win="unmeasured"
      fi
      if r="$(run_evidence "$evidence" "")"; then
        an="${r%% *}"; at="${r##* }"; all="$an/$at $(pct "$an" "$at")"
      else
        an=0; at=0; all="unmeasured"
      fi
    fi

    local fired="-" seen="-"
    if [ "$have_counts" -eq 1 ]; then
      fired="$(printf '%s\n' "$counts" | awk -v n="$name" '$1 == n { print $2 }')"
      seen="$(printf '%s\n' "$counts" | awk -v n="$name" '$1 == n { print $3 }')"
      [ -n "$fired" ] || { fired=0; seen="never"; }
    elif [ "$have_counts" -eq 2 ]; then
      fired="unmeasured"; seen="-"
    fi

    if [ "$JSON" -ne 1 ]; then
      if [ "$have_counts" -eq 0 ]; then
        printf '%-22s %-6s %-22s %-22s %s\n' "$name" "$wired" "$win" "$all" "$tally"
      else
        printf '%-22s %-6s %-22s %-22s %-7s %-6s %s\n' \
          "$name" "$wired" "$win" "$all" "$tally" "$fired" "$seen"
      fi
    fi

    local inv_json=""
    [ "$have_counts" -eq 0 ] || inv_json=",\"invocations\":\"$(jesc "$fired")\",\"last_invoked\":\"$(jesc "$seen")\""

    rows_json="$rows_json$sep{\"name\":\"$(jesc "$name")\",\"wired\":$([ "$wired" = yes ] && echo true || echo false),\"produces\":\"$(jesc "$produces")\",\"windowed\":{\"conforming\":$wn,\"total\":$wt,\"label\":\"$(jesc "$win")\"},\"all_time\":{\"conforming\":$an,\"total\":$at,\"label\":\"$(jesc "$all")\"},\"issues\":$tally$inv_json}"
    sep=","
  done <<< "$(find "$SKILLS_DIR" -mindepth 1 -maxdepth 1 -type d | sort)"

  EXTRA_JSON="\"skills\":[$rows_json],"
}

# --- reporting --------------------------------------------------------------

report() {
  local command="$1" clean_msg="${2:-}" errors=0 warnings=0 infos=0 i
  for i in $(find_indices); do
    case "${FIND_SEV[$i]}" in
      error) errors=$((errors + 1)) ;;
      warning) warnings=$((warnings + 1)) ;;
      *) infos=$((infos + 1)) ;;
    esac
  done
  local total=$((errors + warnings + infos))

  if [ "$JSON" -eq 1 ]; then
    local status="ok"
    [ "$total" -eq 0 ] || status="issues"
    printf '{"schema_version":1,"command":"%s","status":"%s","items":[' "$command" "$status"
    local sep=""
    for i in $(find_indices); do
      printf '%s{"id":"%s","severity":"%s","message":"%s","remediation":"%s"}' \
        "$sep" "$(jesc "${FIND_ID[$i]}")" "${FIND_SEV[$i]}" \
        "$(jesc "${FIND_MSG[$i]}")" "$(jesc "${FIND_REM[$i]}")"
      sep=","
    done
    printf '],%s"errors":[],"summary":{"plans":%s,"lessons":%s,"issues":%s,"findings":%s,"error":%s,"warning":%s,"info":%s}}\n' \
      "$EXTRA_JSON" \
      "$(plan_dirs | grep -c . || true)" "$(count_lessons)" "$(count_issues)" \
      "$total" "$errors" "$warnings" "$infos"
  else
    for i in $(find_indices); do
      printf '%-7s %s\n         %s\n         fix: %s\n' \
        "${FIND_SEV[$i]}" "${FIND_ID[$i]}" "${FIND_MSG[$i]}" "${FIND_REM[$i]}" >&2
    done
    if [ "$total" -eq 0 ]; then
      [ -z "$clean_msg" ] || printf 'ash: %s\n' "$clean_msg" >&2
    else
      printf 'ash: %s finding(s) — %s error, %s warning, %s info\n' \
        "$total" "$errors" "$warnings" "$infos" >&2
    fi
  fi

  [ "$total" -eq 0 ] || return 3
}

# --- main -------------------------------------------------------------------

cmd="${1:-}"
[ $# -eq 0 ] || shift
check_only=0
positional=""
while [ $# -gt 0 ]; do
  case "$1" in
    --json) JSON=1 ;;
    --check) check_only=1 ;;
    --transcripts)
      shift
      [ $# -gt 0 ] || { printf 'ash: --transcripts needs a directory\n' >&2; usage; }
      TRANSCRIPTS="$1"
      ;;
    --transcripts=*) TRANSCRIPTS="${1#--transcripts=}" ;;
    -h|--help) usage ;;
    --*) printf 'ash: unknown flag %s\n' "$1" >&2; usage ;;
    *)
      [ -z "$positional" ] || { printf 'ash: unexpected argument %s\n' "$1" >&2; usage; }
      positional="$1"
      ;;
  esac
  shift
done

case "$cmd" in
  check)
    check_corpus
    check_lessons
    # A stale index is a corpus problem like any other, so `check` catches it
    # without the caller having to remember a second command.
    if [ -f "$INDEX_FILE" ]; then
      if ! render_index | diff -q - "$INDEX_FILE" >/dev/null 2>&1; then
        finding "index.stale" warning \
          "$INDEX_FILE does not match the plan frontmatter" \
          "run: scripts/ash.sh index"
      fi
    else
      finding "index.missing" warning "$INDEX_FILE does not exist" \
        "run: scripts/ash.sh index"
    fi
    report check "corpus clean ($(plan_dirs | grep -c . || true) plans)"
    ;;
  skills)
    cmd_skills
    report skills "every skill has a contract"
    ;;
  index)
    if [ "$check_only" -eq 1 ]; then
      if [ -f "$INDEX_FILE" ] && render_index | diff -q - "$INDEX_FILE" >/dev/null 2>&1; then
        report index "$INDEX_FILE is up to date"
      else
        finding "index.stale" warning \
          "$INDEX_FILE does not match the plan frontmatter" \
          "run: scripts/ash.sh index"
        report index
      fi
    else
      mkdir -p "$ASH_DIR"
      render_index > "$INDEX_FILE.tmp"
      mv "$INDEX_FILE.tmp" "$INDEX_FILE"
      report index "wrote $INDEX_FILE"
    fi
    ;;
  new-id)
    # `plan-write` calls this rather than inventing an id, so the format has
    # exactly one definition and it lives here.
    mint_id "$positional"
    ;;
  ""|-h|--help) usage ;;
  *) printf 'ash: unknown command %s\n' "$cmd" >&2; usage ;;
esac
