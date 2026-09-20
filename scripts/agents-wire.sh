#!/usr/bin/env bash
# Projects the vendor-neutral skills in .agents/skills/ into the per-vendor
# directory each agent runtime actually reads.
#
#   scripts/agents-wire.sh            (re)create the links
#   scripts/agents-wire.sh --check    verify without changing anything
#   scripts/agents-wire.sh --copy     write real copies instead of symlinks
#
# Why this exists: no runtime reads `.agents/skills/`. Claude Code reads
# `.claude/skills/`. Without this projection the repo's skills are inert
# files, and the only copy that loads is whatever drifted into someone's
# personal ~/.claude — which is exactly what had happened here.
#
# This is the same trick pinst plays with configs/: one source tree, symlinked
# into the place the consumer looks, so edits round-trip with no sync step.
#
#   0  wired and correct   2  usage error
#   1  failed to run       3  ran fine, found things to act on
set -euo pipefail

SRC_DIR="${AGENTS_SKILLS_DIR:-.agents/skills}"
# One entry per vendor. Adding a runtime that reads skills is one line here.
TARGET_DIRS=("${AGENTS_TARGET_DIR:-.claude/skills}")

MODE=wire
LINK_STYLE=symlink

FIND_ID=(); FIND_SEV=(); FIND_MSG=(); FIND_REM=()
finding() { FIND_ID+=("$1"); FIND_SEV+=("$2"); FIND_MSG+=("$3"); FIND_REM+=("$4"); }
find_indices() { [ "${#FIND_ID[@]}" -gt 0 ] && printf '%s\n' "${!FIND_ID[@]}"; return 0; }

usage() { sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 2; }

for arg in "$@"; do
  case "$arg" in
    --check) MODE=check ;;
    --copy) LINK_STYLE=copy ;;
    -h|--help) usage ;;
    *) printf 'agents-wire: unknown flag %s\n' "$arg" >&2; usage ;;
  esac
done

[ -d "$SRC_DIR" ] || { printf 'agents-wire: %s does not exist\n' "$SRC_DIR" >&2; exit 1; }

skills() { find "$SRC_DIR" -mindepth 1 -maxdepth 1 -type d | sort; }

# A SKILL.md whose frontmatter `name` disagrees with its directory is a skill
# that loads under a name nothing references — worth catching before it ships.
# Returns non-zero if the skill is not fit to wire. A skill that declares the
# wrong name, or none, would load under a name nothing references; one with no
# description never triggers. Linking either puts a known-broken skill in front
# of the model, so validation failures are reported and then skipped.
check_skill_source() {
  local dir="$1" name="$2" declared
  # Separate statement: `local` expands every word before it assigns any of
  # them, so folding this into the line above reads $dir before it is set.
  local file="$dir/SKILL.md"
  local bad=0
  if [ ! -f "$file" ]; then
    finding "skill.no-manifest.$name" error "$dir has no SKILL.md" \
      "add SKILL.md with name/description frontmatter, or remove the directory"
    return 1
  fi
  declared="$(awk -F': *' '/^name:/ {print $2; exit}' "$file")"
  if [ -z "$declared" ]; then
    finding "skill.no-name.$name" error "$file frontmatter has no 'name:'" \
      "add 'name: $name'"
    bad=1
  elif [ "$declared" != "$name" ]; then
    finding "skill.name-mismatch.$name" error \
      "$file declares name '$declared' but lives in '$name/'" \
      "make them agree — the directory name is what gets invoked"
    bad=1
  fi
  if ! awk -F': *' '/^description:/ {found=1} END {exit !found}' "$file"; then
    finding "skill.no-description.$name" error "$file frontmatter has no 'description:'" \
      "add one — it is the only thing a model sees when deciding to load the skill"
    bad=1
  fi

  # This frontmatter has always *looked* like YAML while only ever being read
  # by grep and awk, so nothing noticed that half of it would not parse: a
  # description reading "... (type(scope): subject) ..." ends a plain scalar at
  # the colon. It cost nothing until something started reading the block as
  # YAML, and then it cost every skill at once. Quoting is the fix; this is
  # what keeps it fixed.
  local unquoted
  unquoted="$(awk '
    NR == 1 && $0 != "---" { exit }
    NR == 1 { next }
    /^---[[:space:]]*$/ { exit }
    /^[a-z_-]+:[[:space:]]/ {
      key = $0; sub(/:.*$/, "", key)
      val = $0; sub(/^[a-z_-]+:[[:space:]]+/, "", val)
      first = substr(val, 1, 1)
      if (first == "\"" || first == "\047") next
      if (val ~ /:[[:space:]]/) print key
    }
  ' "$file" | tr '\n' ' ')"
  if [ -n "$unquoted" ]; then
    finding "skill.unquoted-value.$name" warning \
      "$file frontmatter value(s) contain a colon and are not quoted: ${unquoted% }" \
      "wrap the value in single quotes, doubling any apostrophe inside it"
    bad=1
  fi

  return "$bad"
}

wire_one() {
  local target_dir="$1" name="$2"
  # Separate statements: `local` expands every word before it assigns any of
  # them, so a self-referencing one-liner would read an unset variable.
  local src="$SRC_DIR/$name"
  local dest="$target_dir/$name"
  local depth rel
  # Relative link, so the repo stays portable across clones and worktrees.
  # `realpath -s` does this correctly for any mix of absolute and relative
  # paths without resolving symlinks; the arithmetic fallback only has to
  # handle the ordinary case of two repo-relative paths.
  mkdir -p "$target_dir"
  rel="$(realpath -s --relative-to="$target_dir" "$src" 2>/dev/null || true)"
  if [ -z "$rel" ]; then
    case "$src" in
      /*) rel="$src" ;;
      *)
        depth="$(printf '%s' "$target_dir" | awk -F/ '{print NF}')"
        rel="$(printf '../%.0s' $(seq 1 "$depth"))$src"
        ;;
    esac
  fi

  if [ "$LINK_STYLE" = copy ]; then
    if [ "$MODE" = check ]; then
      if [ ! -f "$dest/SKILL.md" ] || ! diff -q "$src/SKILL.md" "$dest/SKILL.md" >/dev/null 2>&1; then
        finding "wire.stale.$name" error "$dest is missing or differs from $src" \
          "run: scripts/agents-wire.sh --copy"
      fi
    else
      mkdir -p "$dest"
      cp "$src/SKILL.md" "$dest/SKILL.md"
    fi
    return 0
  fi

  if [ "$MODE" = check ]; then
    if [ ! -L "$dest" ]; then
      if [ -e "$dest" ]; then
        finding "wire.not-a-link.$name" error "$dest exists but is not a symlink" \
          "remove it and run: scripts/agents-wire.sh"
      else
        finding "wire.missing.$name" error "$name is not wired into $target_dir" \
          "run: scripts/agents-wire.sh"
      fi
    elif [ ! -e "$dest/SKILL.md" ]; then
      finding "wire.broken.$name" error "$dest is a dangling symlink" \
        "run: scripts/agents-wire.sh"
    fi
    return 0
  fi

  mkdir -p "$target_dir"
  [ ! -L "$dest" ] || rm "$dest"
  if [ -e "$dest" ]; then
    finding "wire.not-a-link.$name" error "$dest exists and is not a symlink" \
      "move it aside, then re-run — this script will not delete real files"
    return 0
  fi
  ln -s "$rel" "$dest"
}

# Links pointing at a skill that no longer exists are the harness quietly
# offering a skill that was deleted; they get cleaned up, real files never do.
prune_stale() {
  local target_dir="$1" dest name
  [ -d "$target_dir" ] || return 0
  while IFS= read -r dest; do
    [ -n "$dest" ] || continue
    name="$(basename "$dest")"
    [ ! -d "$SRC_DIR/$name" ] || continue
    if [ -L "$dest" ]; then
      if [ "$MODE" = check ]; then
        finding "wire.orphan.$name" warning "$dest links to a skill that no longer exists" \
          "run: scripts/agents-wire.sh"
      else
        rm "$dest"
        printf 'agents-wire: pruned orphan link %s\n' "$dest" >&2
      fi
    else
      finding "wire.unmanaged.$name" warning \
        "$dest is not managed by $SRC_DIR" \
        "move it into $SRC_DIR/$name so it is versioned with the rest of the harness"
    fi
  done <<< "$(find "$target_dir" -mindepth 1 -maxdepth 1 | sort)"
}

count=0
while IFS= read -r dir; do
  [ -n "$dir" ] || continue
  name="$(basename "$dir")"
  check_skill_source "$dir" "$name" || continue
  count=$((count + 1))
  for target in "${TARGET_DIRS[@]}"; do
    wire_one "$target" "$name"
  done
done <<< "$(skills)"

for target in "${TARGET_DIRS[@]}"; do
  prune_stale "$target"
done

errors=0; warnings=0
for i in $(find_indices); do
  case "${FIND_SEV[$i]}" in
    error) errors=$((errors + 1)) ;;
    *) warnings=$((warnings + 1)) ;;
  esac
  printf '%-7s %s\n         %s\n         fix: %s\n' \
    "${FIND_SEV[$i]}" "${FIND_ID[$i]}" "${FIND_MSG[$i]}" "${FIND_REM[$i]}" >&2
done

total=$((errors + warnings))
if [ "$total" -eq 0 ]; then
  if [ "$MODE" = check ]; then
    printf 'agents-wire: %s skill(s) wired into %s\n' "$count" "${TARGET_DIRS[*]}" >&2
  else
    printf 'agents-wire: wired %s skill(s) into %s (%s)\n' "$count" "${TARGET_DIRS[*]}" "$LINK_STYLE" >&2
  fi
  exit 0
fi
printf 'agents-wire: %s finding(s) — %s error, %s warning\n' "$total" "$errors" "$warnings" >&2
exit 3
