#!/usr/bin/env bash
# Mycelium Stop hook — enforce end-of-task follow-up check.
#
# No-op unless the current project is a mycelium project (AGENTS.md
# carries the myc marker). When active follow-ups exist, it feeds them
# back to the agent so they get surfaced to the user instead of relying
# on the agent remembering the AGENTS.md rule.
#
# Installed to ~/.claude/hooks/ and wired into hooks.Stop by install-hook.sh.

# Claude Code passes the hook payload as JSON on stdin.
input=$(cat)

# Loop guard: if this Stop was itself triggered by a Stop hook, bail —
# otherwise blocking would re-fire this hook forever.
if echo "$input" | jq -e '.stop_hook_active == true' >/dev/null 2>&1; then
  exit 0
fi

# Gate 1: mycelium project? Marker is version-independent (myc:agents-start v=N).
grep -q "myc:agents-start" AGENTS.md 2>/dev/null || exit 0

# Gate 2: myc binary available?
command -v myc >/dev/null 2>&1 || exit 0

# jq is required to parse myc JSON; degrade silently if missing.
command -v jq >/dev/null 2>&1 || exit 0

# Count OPEN follow-ups only. Per the project rule, `in_progress` items are
# already being worked and don't need an end-of-task decision — so they must
# NOT trigger the block (matches the `myc task close` close-hint behavior).
counts=$(myc followup count --format json 2>/dev/null) || exit 0
open=$(echo "$counts" | jq '(.open // 0)')
[ "${open:-0}" -gt 0 ] 2>/dev/null || exit 0

# Build a bullet list of the OPEN items for the reminder.
items=$(myc followup list --status open --format json 2>/dev/null \
  | jq -r '.[] | "  - [\(.id)] \(.title // "untitled"): \(.body)"')

# Emit Stop-hook JSON. `block` + `reason` is how a Stop hook feeds text
# back to the model; the guard above ensures it fires at most once.
jq -n --arg open "$open" --arg items "$items" '{
  decision: "block",
  reason: ("MYCELIUM FOLLOW-UP CHECK: " + $open + " open follow-up(s) exist. " +
    "Per project rule, surface them to the user before wrapping — never process " +
    "silently. Ask whether to handle them now or leave for later.\n\n" +
    "Open follow-ups:\n" + $items)
}'
