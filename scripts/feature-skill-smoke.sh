#!/usr/bin/env bash
set -euo pipefail

# Keep the end-to-end skill's handoffs visible to a cheap local check. The
# harness owns frontmatter and projection invariants; this check owns the
# composition contract that those generic checks cannot infer.
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
skill="$root/.agents/skills/implement-feature/SKILL.md"
command="$root/.agents/commands/feature.md"
orchestrate="$root/.agents/skills/orchestrate/SKILL.md"

test -f "$skill"
test -f "$command"
test -f "$orchestrate"

grep -Fq 'name: implement-feature' "$skill"
for handoff in plan-write plan-implement plan-learnings create-pr; do
    grep -Fq "\`$handoff\`" "$skill"
done
grep -Fq 'git worktree' "$skill"
grep -Fq 'just qc' "$skill"
grep -Fq 'plan-learnings' "$skill"
grep -Fq 'fully resolved' "$skill"
grep -Fq 'implement-feature' "$orchestrate"
grep -Fq 'resolved' "$orchestrate"
grep -Fq 'plan-write' "$command"
grep -Fq 'create-pr' "$command"

printf '%s\n' "implement-feature contract: ok"
