# Fireshot Wayland (Rust) - MVP

This is an early rewrite focused on Wayland via xdg-desktop-portal (compositor independent).

## MVP scope

- Capture via xdg-desktop-portal Screenshot (interactive selection)
- Optional save to file
- Basic editor: pencil, line, rectangle; save-as

## Usage

```sh
cargo run -p fireshot -- gui
cargo run -p fireshot -- gui -d 2000 -p /tmp/capture.png
cargo run -p fireshot -- full -p /tmp/capture.png
```

## Daemon + DBus

Run a DBus daemon so other apps/scripts can trigger captures:

```sh
./target/release/fireshot daemon
```

DBus service:

- `org.fireshot.Fireshot`
- `/org/fireshot/Fireshot`

Methods:

- `gui(delay_ms: u64, path: String)`
- `full(delay_ms: u64, path: String)`
- `quit()`
- `version() -> String`

Tray icon: available in daemon mode (uses StatusNotifierItem).

## Wayland portal setup (required)

This app relies on `xdg-desktop-portal` and a compositor-specific backend.
If the portal backend is missing or mismatched, capture will fail.

### Recommended for wlroots/niri

Start the wlroots backend and restart the portal:

```sh
systemctl --user start xdg-desktop-portal-wlr
systemctl --user restart xdg-desktop-portal
```

Create `~/.config/xdg-desktop-portal/portals.conf`:

```ini
[preferred]
default=wlr;gtk;

[niri]
default=wlr;gtk;
```

Log out/in (or restart the portal again), then test:

```sh
./target/release/fireshot diagnose --ping
```

### GNOME/KDE

Ensure the correct backend is running:

- GNOME: `xdg-desktop-portal-gnome`
- KDE: `xdg-desktop-portal-kde`

## Diagnostics

```sh
./target/release/fireshot diagnose
./target/release/fireshot diagnose --ping
```

## Testing

Scripted smoke test:

```sh
./scripts/test.sh
```

Include the interactive portal ping:

```sh
FIRESHOT_PORTAL_PING=1 ./scripts/test.sh
```

## TODO (next milestones)

- Clipboard copy
- Non-interactive full screen capture
- Editor tools (rect, arrow, text, blur)
- Config file + tray/daemon
- DBus interface
