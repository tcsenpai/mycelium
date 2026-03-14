#!/usr/bin/env bash
set -euo pipefail

# Mycelium installer — builds and installs myc (CLI) and/or MycUI (GUI)

INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
INSTALL_CLI=false
INSTALL_GUI=false

# ---------- argument parsing ----------

if [ $# -eq 0 ]; then
    INSTALL_CLI=true
    INSTALL_GUI=true
fi

for arg in "$@"; do
    case "$arg" in
        --cli)  INSTALL_CLI=true ;;
        --gui)  INSTALL_GUI=true ;;
        --all)  INSTALL_CLI=true; INSTALL_GUI=true ;;
        --help|-h)
            echo "Usage: ./install.sh [--cli] [--gui] [--all]"
            echo ""
            echo "  --cli   Build and install myc (CLI only)"
            echo "  --gui   Build and install MycUI (Tauri GUI)"
            echo "  --all   Install both (default when no flags given)"
            echo ""
            echo "Set INSTALL_DIR to change install path (default: /usr/local/bin)"
            exit 0
            ;;
        *)
            echo "Unknown option: $arg (use --help for usage)"
            exit 1
            ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OS="$(uname -s)"

# ---------- helpers ----------

info()  { echo -e "\033[1;34m==>\033[0m $*"; }
ok()    { echo -e "\033[1;32m==>\033[0m $*"; }
err()   { echo -e "\033[1;31m==>\033[0m $*" >&2; }

check_cmd() {
    if ! command -v "$1" &>/dev/null; then
        err "Required: $1 — $2"
        return 1
    fi
}

need_sudo() {
    local target_dir="${SUDO_TARGET_DIR:-$INSTALL_DIR}"
    if [ -w "$target_dir" ]; then
        "$@"
    else
        sudo "$@"
    fi
}

# ---------- platform check ----------

case "$OS" in
    Linux|Darwin) ;;
    *)
        err "Unsupported platform: $OS (only Linux and macOS are supported)"
        exit 1
        ;;
esac

# ---------- dependency checks ----------

info "Checking dependencies ($OS)..."

MISSING=false

check_cmd cargo "Install from https://rustup.rs" || MISSING=true

if $INSTALL_GUI; then
    check_cmd bun "Install from https://bun.sh" || MISSING=true
    if [ "$OS" = "Linux" ]; then
        for lib in libwebkit2gtk-4.1 libgtk-3 libayatana-appindicator3; do
            if ! pkg-config --exists "${lib}-dev" 2>/dev/null && ! pkg-config --exists "$lib" 2>/dev/null; then
                info "Note: $lib may be needed — install Tauri prerequisites if build fails"
                info "  See: https://v2.tauri.app/start/prerequisites/#linux"
            fi
        done
    fi
fi

if $MISSING; then
    err "Install missing dependencies and try again."
    exit 1
fi

# ---------- build CLI ----------

if $INSTALL_CLI; then
    info "Building myc (CLI)..."
    cd "$SCRIPT_DIR"
    cargo build --release

    info "Installing myc to $INSTALL_DIR..."
    need_sudo install -m 755 target/release/myc "$INSTALL_DIR/myc"
    ok "myc installed to $INSTALL_DIR/myc"
fi

# ---------- build GUI ----------

if $INSTALL_GUI; then
    info "Building MycUI (GUI)..."
    cd "$SCRIPT_DIR/mycui"
    bun install
    bun run tauri:build

    if [ "$OS" = "Darwin" ]; then
        APP_BUNDLE="src-tauri/target/release/bundle/macos/MycUI.app"
        if [ ! -d "$APP_BUNDLE" ]; then
            err "MycUI.app not found at $APP_BUNDLE"
            exit 1
        fi
        info "Installing MycUI.app to /Applications..."
        if [ -d "/Applications/MycUI.app" ]; then
            SUDO_TARGET_DIR=/Applications need_sudo rm -rf /Applications/MycUI.app
        fi
        SUDO_TARGET_DIR=/Applications need_sudo cp -R "$APP_BUNDLE" /Applications/
        ok "MycUI installed to /Applications/MycUI.app"
    else
        GUI_BIN="src-tauri/target/release/mycui"
        if [ ! -f "$GUI_BIN" ]; then
            # Tauri may use productName casing
            GUI_BIN="src-tauri/target/release/MycUI"
        fi
        if [ ! -f "$GUI_BIN" ]; then
            err "MycUI binary not found in src-tauri/target/release/"
            exit 1
        fi
        info "Installing mycui to $INSTALL_DIR..."
        need_sudo install -m 755 "$GUI_BIN" "$INSTALL_DIR/mycui"
        ok "mycui installed to $INSTALL_DIR/mycui"
    fi
fi

# ---------- done ----------

echo ""
ok "Done! Installed:"
$INSTALL_CLI && echo "  myc   -> $INSTALL_DIR/myc"
if $INSTALL_GUI; then
    if [ "$OS" = "Darwin" ]; then
        echo "  MycUI -> /Applications/MycUI.app"
    else
        echo "  mycui -> $INSTALL_DIR/mycui"
    fi
fi
