# Floating Timer

An always-on-top, borderless countdown timer built with `eframe`/`egui`. It
lives in the system tray, remembers its position and task name, and
automatically shrinks into a small pill while running.

## Features

- Always on top, borderless, transparent rounded card, draggable
- Task name and countdown display
- System tray icon (Show / Quit) and a desktop notification when time is up
- Saves position, task, and duration between launches
- Compact mode while running; hover to reveal (fades to 35% when idle)
- Automatic compact mode 10 seconds after the pointer leaves a running timer
- `Space` = Start/Pause, `Ctrl+R` = Reset
- Appears on all virtual desktops and stays above other windows
  (KDE Plasma via KWin scripting; other X11 desktops via EWMH)

## Install

Build the app and install it for the current user (binary, icons, and desktop
entry) with:

```bash
./install.sh
```

This requires a Rust toolchain (`rustup`) and ImageMagick. It installs:

- `~/.local/bin/FloatingTimer`
- `~/.local/share/applications/floating-timer-rs.desktop`
- icons under `~/.local/share/icons/hicolor/`

Launch **Floating Timer** from your application menu, or run:

```bash
~/.local/bin/FloatingTimer
```

To install somewhere else, set `PREFIX`:

```bash
PREFIX=/opt/floating-timer-rs ./install.sh
```

To remove the installed binary, icons, and desktop entry:

```bash
./install.sh --uninstall
```

## Run from source

```bash
cargo run --release
```

## Build only

```bash
cargo build --release
./target/release/floating-timer
```

## Settings

State is stored as JSON at:

```
~/.config/floating-timer-rs/settings.json
```

## Platform notes

- On **Wayland**, compositors do not let applications place their own windows,
  so the saved position is only restored on X11. All other features work.
- The "all desktops / always above" behaviour uses a short KWin script on KDE
  Plasma (Wayland and X11), and EWMH client messages on other X11 window
  managers.

## Third-party assets

- `assets/DejaVuSans-Bold.ttf` - DejaVu Sans Bold, see
  `assets/DejaVuSans-Bold.LICENSE.txt`.
- `assets/icon.png` - application icon.
