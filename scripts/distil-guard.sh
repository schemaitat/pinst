#!/usr/bin/env bash
# The two rails around an unattended `/distil` run (.github/workflows/distil.yml).
#
#   scripts/distil-guard.sh preflight   is there anything to distil?
#   scripts/distil-guard.sh verify      is what it produced safe to publish?
#
# Why this is a script and not workflow YAML: a rail that can only be
# exercised by waiting for 06:17 UTC is a rail nobody tests. Both verbs run
# at a terminal, against the real corpus, with the same exit codes CI sees.
#
#   0  go ahead        2  usage error
#   1  nothing to do   3  ran fine, found things to act on
#
# The 1 is the odd one out and is deliberate: everywhere else in this repo 1
# means "failed to run", but the common case for a daily job here is a clean
# corpus, and that is not a failure — it is the answer. A preflight that
# genuinely could not run exits 3 with `distil.preflight-failed`, so 1 never
# means "broken". `preflight` also prints `work=true|false` on stdout, and the
# workflow branches on that string rather than on the code.
set -euo pipefail

ASH="${ASH_BIN:-scripts/ash.sh}"
WIRE="${WIRE_BIN:-scripts/agents-wire.sh}"

# What an unattended distillation is allowed to touch. Everything it produces
# is prose in the corpus or an edit to a skill it already had; anything else —
# source, CI, the manifest — is a sign the pass went somewhere it should not.
ALLOWED_PREFIXES=(".ash/" ".agents/skills/" ".claude/skills/")

# The diff budget. Variables, not literals, so a legitimate breach is
# re-examined by raising a bound in one place rather than by deleting a check.
MAX_LINES="${DISTIL_MAX_LINES:-500}"
MAX_FILES="${DISTIL_MAX_FILES:-40}"

# What the working tree is compared against. HEAD in CI, where the model's
# work is still uncommitted; overridable for testing a range by hand.
BASE="${DISTIL_BASE:-HEAD}"

FIND_ID=(); FIND_SEV=(); FIND_MSG=(); FIND_REM=()
finding() { FIND_ID+=("$1"); FIND_SEV+=("$2"); FIND_MSG+=("$3"); FIND_REM+=("$4"); }
find_indices() { [ "${#FIND_ID[@]}" -gt 0 ] && printf '%s\n' "${!FIND_ID[@]}"; return 0; }

usage() { sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }

# Mirrors ash.sh's report(): findings to stderr in a fixed shape, exit 3 if
# there were any. Same output whichever guard produced them.
report() {
  local clean_msg="${1:-}" total=0 i
  for i in $(find_indices); do total=$((total + 1)); done
  for i in $(find_indices); do
    printf '%-7s %s\n         %s\n         fix: %s\n' \
      "${FIND_SEV[$i]}" "${FIND_ID[$i]}" "${FIND_MSG[$i]}" "${FIND_REM[$i]}" >&2
  done
  if [ "$total" -eq 0 ]; then
    [ -z "$clean_msg" ] || printf 'distil: %s\n' "$clean_msg" >&2
  else
    printf 'distil: %s finding(s)\n' "$total" >&2
    return 3
  fi
}

# --- preflight --------------------------------------------------------------
# The model-free half of the gate. `ash.sh` already computes both numbers that
# matter — `summary.findings` from check, `agenda_items` from skills — so this
# asks the corpus rather than forming its own opinion about what needs doing.
#
# Both subcommands exit 3 when they have something to say, which is the normal
# case here, so their status is captured rather than allowed to kill the run.

json_field() {
  # json_field <field-path> — reads JSON on stdin, prints an integer, or
  # nothing at all if the document does not parse or the field is missing.
  python3 -c '
import json, sys
try:
    doc = json.load(sys.stdin)
except Exception:
    sys.exit(1)
for key in sys.argv[1].split("."):
    if not isinstance(doc, dict) or key not in doc:
        sys.exit(1)
    doc = doc[key]
print(int(doc))
' "$1" 2>/dev/null
}

cmd_preflight() {
  local check_json skills_json findings agenda

  check_json="$("$ASH" check --json 2>/dev/null || true)"
  skills_json="$("$ASH" skills --json 2>/dev/null || true)"

  findings="$(printf '%s' "$check_json" | json_field summary.findings || true)"
  agenda="$(printf '%s' "$skills_json" | json_field agenda_items || true)"

  if [ -z "$findings" ] || [ -z "$agenda" ]; then
    printf 'work=false\n'
    finding "distil.preflight-failed" error \
      "could not read summary.findings or agenda_items from $ASH --json" \
      "run '$ASH check --json' and '$ASH skills --json' by hand and look at what they print"
    report
    return
  fi

  printf 'distil: %s agenda item(s), %s finding(s)\n' "$agenda" "$findings" >&2
  if [ "$agenda" -eq 0 ] && [ "$findings" -eq 0 ]; then
    printf 'work=false\n'
    printf 'distil: corpus clean — nothing to distil\n' >&2
    return 1
  fi
  printf 'work=true\n'
  return 0
}

# --- verify -----------------------------------------------------------------
# Everything between the model and a pull request. Each check answers a
# different way an unattended pass goes wrong: it wandered out of the corpus,
# it wrote far more than a distillation should, it deleted something, or it
# left the corpus failing its own invariants.

# Every path the run touched, tracked and untracked alike. `git diff` alone
# would miss a brand-new file, which is exactly how a pass could write outside
# the allowlist without this noticing — the interesting artifacts here (a new
# learnings.md, a new lesson file) are created, not modified.
changed_paths() {
  { git diff --name-only "$BASE" --; git ls-files --others --exclude-standard; } \
    | sed '/^$/d' | sort -u
}

path_allowed() {
  local path="$1" prefix
  for prefix in "${ALLOWED_PREFIXES[@]}"; do
    case "$path" in "$prefix"*) return 0 ;; esac
  done
  return 1
}

# The ids of every `### LESSON-NNN` / `### ISSUE-NNN` heading in a file, one
# per line. Matching on the id alone and not the title is deliberate: retitling
# a lesson is ordinary distillation work, removing one is not.
heading_ids() { grep -oE '^### (LESSON|ISSUE)-[0-9]+' "$1" 2>/dev/null | awk '{print $2}' | sort -u; }
heading_ids_at_base() {
  git show "$BASE:$1" 2>/dev/null | grep -oE '^### (LESSON|ISSUE)-[0-9]+' | awk '{print $2}' | sort -u
}

# One file's headings, before against after. `$2` is the finding id prefix and
# `$3` the human name of what went missing.
check_append_only() {
  local file="$1" prefix="$2" label="$3" id before after
  # `|| true` on both: a learnings.md with no issues recorded is a normal
  # state (the two legacy plans have none), and grep exiting 1 on it would
  # otherwise take the whole script down under `set -e`.
  before="$(heading_ids_at_base "$file" || true)"
  [ -n "$before" ] || return 0
  after="$(heading_ids "$file" || true)"
  for id in $before; do
    # A here-string, not `printf | grep -q`: with `set -o pipefail` the -q
    # makes grep exit on its first match, printf dies of SIGPIPE, and the
    # pipeline reports 141 for a lesson that is present. That failed on
    # whichever id grep happened to match first — a phantom removal.
    grep -qx "$id" <<< "$after" && continue
    finding "$prefix.$id" error \
      "$id was removed from $file" \
      "$label is append-only — restore the entry; retire or decline it in place instead of deleting it"
  done
}

cmd_verify() {
  local paths count_files count_lines path file plan

  paths="$(changed_paths || true)"
  if [ -z "$paths" ]; then
    report "no changes to verify"
    return
  fi

  # 1. Where it wrote. SEC-001 is a property of the diff, not of the tool
  # allowlist that was supposed to produce it.
  while IFS= read -r path; do
    [ -n "$path" ] || continue
    path_allowed "$path" && continue
    finding "distil.path-not-allowed.$path" error \
      "$path is outside what an unattended distillation may touch" \
      "an unattended run may only write ${ALLOWED_PREFIXES[*]} — revert $path and look at why the run reached it"
  done <<< "$paths"

  # 2. How much it wrote.
  count_files="$(printf '%s\n' "$paths" | grep -c . || true)"
  count_lines="$(
    {
      git diff --numstat "$BASE" -- | awk '{ a = ($1 == "-" ? 0 : $1); d = ($2 == "-" ? 0 : $2); s += a + d } END { print s + 0 }'
      git ls-files --others --exclude-standard -z | xargs -0 -r cat | grep -c '' || true
    } | awk '{ s += $1 } END { print s + 0 }'
  )"
  if [ "$count_lines" -gt "$MAX_LINES" ] || [ "$count_files" -gt "$MAX_FILES" ]; then
    finding "distil.diff-too-large" error \
      "$count_lines changed line(s) across $count_files file(s); the budget is $MAX_LINES line(s) and $MAX_FILES file(s)" \
      "read the diff before raising DISTIL_MAX_LINES/DISTIL_MAX_FILES — a distillation this large is usually a pass that did something other than distil"
  fi

  # 3. What it deleted. The tidiest-looking failure mode there is.
  check_append_only "$ASH_DIR_FILE" "distil.lesson-removed" "$ASH_DIR_FILE"
  while IFS= read -r file; do
    [ -n "$file" ] || continue
    # The plan's directory goes in the finding id: two plans can each lose
    # their own ISSUE-002, and one finding id must not stand for both.
    plan="$(basename "$(dirname "$file")")"
    check_append_only "$file" "distil.issue-removed.$plan" "a plan's learnings.md"
  done < <(git ls-tree -r --name-only "$BASE" -- "$PLANS_GLOB" 2>/dev/null | grep '/learnings\.md$' || true)

  # 4. What it left behind. An edited skill that was never re-wired loads
  # nowhere (LESSON-007), and a corpus failing its own invariants must not
  # become a pull request.
  if ! "$WIRE" --check >/dev/null 2>&1; then
    finding "distil.harness-failed" error \
      "$WIRE --check fails on the distilled tree" \
      "run '$WIRE --check' to see it, then 'just wire' if a skill was edited but not re-projected"
  fi
  if ! "$ASH" check >/dev/null 2>&1; then
    finding "distil.harness-failed.corpus" error \
      "$ASH check fails on the distilled tree" \
      "run '$ASH check' to see the findings; the distillation must leave the corpus clean"
  fi

  report "$count_lines line(s) across $count_files file(s), all within the corpus"
}

ASH_DIR_FILE="${ASH_DIR:-.ash}/LEARNINGS.md"
PLANS_GLOB="${ASH_DIR:-.ash}/plans"

case "${1:-}" in
  preflight) shift; [ $# -eq 0 ] || usage; cmd_preflight ;;
  verify) shift; [ $# -eq 0 ] || usage; cmd_verify ;;
  -h|--help) usage ;;
  *) usage ;;
esac
