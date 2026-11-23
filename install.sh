#!/usr/bin/env bash
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
APP_NAME="FloatingTimer"
APP_ID="floating-timer-rs"
BIN_DIR="$PREFIX/bin"
APP_DIR="$PREFIX/share/applications"
ICON_ROOT="$PREFIX/share/icons/hicolor"
DESKTOP_FILE="$APP_DIR/$APP_ID.desktop"

usage() {
    echo "Usage: $0 [--uninstall]"
    echo
    echo "Builds the app with cargo and installs it to $PREFIX"
    echo "(binary, icon, and desktop entry). Override the location with PREFIX=..."
}

refresh_caches() {
    command -v update-desktop-database >/dev/null 2>&1 && \
        update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 && \
        gtk-update-icon-cache -f -t "$ICON_ROOT" >/dev/null 2>&1 || true
    command -v kbuildsycoca6 >/dev/null 2>&1 && \
        kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
}

uninstall() {
    rm -f "$BIN_DIR/$APP_NAME" "$DESKTOP_FILE"
    find "$ICON_ROOT" -name "$APP_ID.png" -delete 2>/dev/null || true
    refresh_caches
    echo "Uninstalled $APP_NAME from $PREFIX"
}

if [[ "${1:-}" == "--uninstall" ]]; then
    uninstall
    exit 0
elif [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    usage
    exit 0
fi

cd "$REPO_DIR"

CARGO="${CARGO:-cargo}"
if ! command -v "$CARGO" >/dev/null 2>&1; then
    if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
        CARGO="$HOME/.cargo/bin/cargo"
    else
        echo "cargo not found; install Rust from https://rustup.rs" >&2
        exit 1
    fi
fi

echo "==> Building release binary"
"$CARGO" build --release --locked

echo "==> Installing to $PREFIX"
install -Dm755 target/release/floating-timer "$BIN_DIR/$APP_NAME"

echo "==> Installing icons"
if command -v magick >/dev/null 2>&1; then
    RESIZE=magick
elif command -v convert >/dev/null 2>&1; then
    RESIZE=convert
else
    echo "ImageMagick (magick/convert) is required to install icons" >&2
    exit 1
fi
for size in 16 24 32 48 64 128 256 512; do
    out="$ICON_ROOT/${size}x${size}/apps/$APP_ID.png"
    mkdir -p "$(dirname "$out")"
    "$RESIZE" "$REPO_DIR/assets/icon.png" -resize "${size}x${size}" "$out"
done

echo "==> Writing desktop entry"
mkdir -p "$APP_DIR"
cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=Floating Timer (Rust)
GenericName=Countdown Timer
Comment=Always-on-top floating countdown timer
Exec="$BIN_DIR/$APP_NAME"
TryExec=$BIN_DIR/$APP_NAME
Icon=$APP_ID
Terminal=false
Categories=Utility;Clock;
Keywords=timer;countdown;pomodoro;stopwatch;
StartupNotify=false
StartupWMClass=FloatingTimer
EOF
chmod 644 "$DESKTOP_FILE"

refresh_caches

echo
echo "Installed:"
echo "  $BIN_DIR/$APP_NAME"
echo "  $DESKTOP_FILE"
echo
echo "Launch it from your app menu (search \"Floating Timer\") or run: $BIN_DIR/$APP_NAME"
