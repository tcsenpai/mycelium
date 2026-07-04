#!/usr/bin/env bash
set -euo pipefail

# Mycelium Claude Code hook installer.
#
# Installs the myc-followup-stop.sh hook into ~/.claude/hooks/ and wires
# it into hooks.Stop in ~/.claude/settings.json. Idempotent: safe to run
# repeatedly. The hook self-gates, so it stays silent in non-mycelium
# projects.
#
# Usage: ./install-hook.sh [--uninstall] [--settings PATH]

HOOK_NAME="myc-followup-stop.sh"
SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC_HOOK="$SRC_DIR/$HOOK_NAME"

CLAUDE_DIR="${CLAUDE_DIR:-$HOME/.claude}"
HOOKS_DIR="$CLAUDE_DIR/hooks"
SETTINGS="${SETTINGS:-$CLAUDE_DIR/settings.json}"
DEST_HOOK="$HOOKS_DIR/$HOOK_NAME"

# Command string stored in settings.json (uses $HOME so it stays portable).
HOOK_CMD="\$HOME/.claude/hooks/$HOOK_NAME"

UNINSTALL=false
while [ $# -gt 0 ]; do
    case "$1" in
        --uninstall) UNINSTALL=true ;;
        --settings)  SETTINGS="$2"; shift ;;
        --help|-h)
            echo "Usage: ./install-hook.sh [--uninstall] [--settings PATH]"
            echo ""
            echo "  --uninstall     Remove the hook from settings.json and delete the script"
            echo "  --settings PATH Target settings.json (default: \$HOME/.claude/settings.json)"
            exit 0 ;;
        *) echo "Unknown arg: $1" >&2; exit 1 ;;
    esac
    shift
done

command -v jq >/dev/null 2>&1 || { echo "error: jq is required" >&2; exit 1; }

# Patch settings.json using jq. Reads current file (or {} if absent),
# writes atomically via temp file.
patch_settings() {
    local filter="$1"
    local tmp
    tmp="$(mktemp)"
    if [ -f "$SETTINGS" ]; then
        jq "$filter" "$SETTINGS" > "$tmp"
    else
        echo '{}' | jq "$filter" > "$tmp"
    fi
    mv "$tmp" "$SETTINGS"
}

if [ "$UNINSTALL" = true ]; then
    echo "Uninstalling mycelium follow-up hook…"
    if [ -f "$SETTINGS" ]; then
        # Drop any Stop entry whose hooks[].command references our script.
        patch_settings "
          .hooks.Stop = ((.hooks.Stop // []) | map(
            select(
              [ (.hooks // [])[].command ] | any(. == \"$HOOK_CMD\") | not
            )
          ))
        "
        echo "  ✓ Removed hook entry from $SETTINGS"
    fi
    if [ -f "$DEST_HOOK" ]; then
        rm -f "$DEST_HOOK"
        echo "  ✓ Deleted $DEST_HOOK"
    fi
    echo "Done."
    exit 0
fi

echo "Installing mycelium follow-up hook…"

# 1. Copy hook script into place.
mkdir -p "$HOOKS_DIR"
cp "$SRC_HOOK" "$DEST_HOOK"
chmod +x "$DEST_HOOK"
echo "  ✓ Installed $DEST_HOOK"

# 2. Wire into hooks.Stop, but only if not already present (idempotent).
already="$(
    if [ -f "$SETTINGS" ]; then
        jq --arg cmd "$HOOK_CMD" '
          [ (.hooks.Stop // [])[].hooks[]?.command ] | any(. == $cmd)
        ' "$SETTINGS"
    else
        echo false
    fi
)"

if [ "$already" = "true" ]; then
    echo "  ✓ settings.json already wired (no change)"
else
    patch_settings "
      .hooks.Stop = ((.hooks.Stop // []) + [
        { hooks: [ { type: \"command\", command: \"$HOOK_CMD\" } ] }
      ])
    "
    echo "  ✓ Appended Stop hook to $SETTINGS"
fi

echo "Done. The hook is silent outside mycelium projects."
