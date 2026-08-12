#!/usr/bin/env bash
set -euo pipefail

# Mycelium installer — builds and installs myc (CLI) and/or MycUI (GUI)

INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
INSTALL_CLI=false
INSTALL_GUI=false
REPO="tcsenpai/mycelium"
# CLI source: "auto" tries a prebuilt release binary then falls back to source;
# "release" forces prebuilt (no fallback); "build" forces a source build.
CLI_MODE="auto"

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
        --from-release) INSTALL_CLI=true; CLI_MODE="release" ;;
        --build)        CLI_MODE="build" ;;
        --help|-h)
            echo "Usage: ./install.sh [--cli] [--gui] [--all] [--from-release] [--build]"
            echo ""
            echo "  --cli            Install myc (CLI). Downloads a prebuilt release"
            echo "                   binary, falling back to a source build."
            echo "  --gui            Build and install MycUI (Tauri GUI, always from source)"
            echo "  --all            Install both (default when no flags given)"
            echo "  --from-release   Force the prebuilt binary (no cargo needed; errors"
            echo "                   if no matching asset exists)"
            echo "  --build          Force a source build (skip the prebuilt lookup)"
            echo ""
            echo "Set INSTALL_DIR to change install path (default: /usr/local/bin)"
            echo "Set MYC_VERSION to pin the prebuilt version (default: latest release)"
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

# Pre-warm sudo only when the install target actually needs elevation, so a
# prebuilt install into a writable dir stays fully non-interactive.
if [ ! -w "$INSTALL_DIR" ]; then
    echo "Requesting superuser access to install into $INSTALL_DIR:"
    sudo -v
fi

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

# ---------- prebuilt release install ----------

# Map uname to the target triples published by release.yml. Echoes the triple,
# or nothing when there's no matching prebuilt asset.
detect_release_target() {
    case "$(uname -s)-$(uname -m)" in
        Darwin-x86_64)             echo "x86_64-apple-darwin" ;;
        Darwin-arm64)              echo "aarch64-apple-darwin" ;;
        Linux-x86_64)              echo "x86_64-unknown-linux-gnu" ;;
        *)                         echo "" ;;
    esac
}

# Download + install the prebuilt myc binary. Returns 1 (without exiting) so
# "auto" mode can fall back to a source build.
install_myc_from_release() {
    local target version tmpdir url
    target="$(detect_release_target)"
    if [ -z "$target" ]; then
        info "No prebuilt myc for $(uname -s)/$(uname -m)."
        return 1
    fi
    check_cmd curl "needed to fetch prebuilt releases" || return 1

    version="${MYC_VERSION:-latest}"
    version="${version#v}"
    if [ "$version" = "latest" ]; then
        # The /releases/latest redirect ends in the tag; strip to the version.
        local tag
        tag="$(curl -fsSL -o /dev/null -w '%{url_effective}' \
            "https://github.com/$REPO/releases/latest" 2>/dev/null || true)"
        version="${tag##*/}"; version="${version#v}"
        [ -n "$version" ] || { info "Could not resolve latest release."; return 1; }
    fi

    url="https://github.com/$REPO/releases/download/v${version}/myc-${target}.tar.gz"
    info "Fetching prebuilt myc ${version} for ${target}..."
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN
    if ! curl -fsSL -o "$tmpdir/myc.tar.gz" "$url"; then
        info "No prebuilt asset at ${url}."
        return 1
    fi
    tar -xzf "$tmpdir/myc.tar.gz" -C "$tmpdir" myc || { err "Extract failed."; return 1; }
    info "Installing myc to $INSTALL_DIR..."
    need_sudo install -m 755 "$tmpdir/myc" "$INSTALL_DIR/myc"
    ok "myc ${version} installed to $INSTALL_DIR/myc (prebuilt)"
}

build_myc_from_source() {
    check_cmd cargo "Install from https://rustup.rs" || {
        err "cargo is required to build myc from source."; exit 1; }
    info "Building myc (CLI) from source..."
    cd "$SCRIPT_DIR"
    cargo build --release
    info "Installing myc to $INSTALL_DIR..."
    need_sudo install -m 755 target/release/myc "$INSTALL_DIR/myc"
    ok "myc installed to $INSTALL_DIR/myc"
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

# cargo is hard-required only for the GUI (Tauri) and forced source builds.
# In "auto" CLI mode a missing cargo just disables the fallback; the prebuilt
# path handles that case, so don't fail here.
if $INSTALL_GUI || { $INSTALL_CLI && [ "$CLI_MODE" = "build" ]; }; then
    check_cmd cargo "Install from https://rustup.rs" || MISSING=true
fi

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

# ---------- install CLI ----------

if $INSTALL_CLI; then
    case "$CLI_MODE" in
        release)
            install_myc_from_release || {
                err "Prebuilt install failed and --from-release forbids a source build."
                exit 1; }
            ;;
        build)
            build_myc_from_source
            ;;
        auto)
            install_myc_from_release || {
                info "Falling back to a source build..."
                build_myc_from_source; }
            ;;
    esac
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
