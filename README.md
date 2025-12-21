# Fireshot Wayland (Rust)

Fireshot is a Wayland-first screenshot tool with an in-place selection overlay,
editor tools, and a tray/daemon mode. It uses the desktop portal for capture so
it stays compositor-agnostic.

## Features

- In-place selection overlay on the active monitor
- Editor tools: pencil, line, arrow, rectangle, circle, marker, text
- Effects: pixelate, blur
- Copy to clipboard, save to file
- Tray icon + DBus daemon

## Requirements

- Wayland session
- xdg-desktop-portal + a backend (wlr/gnome/kde)
- `wl-copy` and `xclip` for clipboard integration

## Install

```bash
cargo build --release
```

Binary: `./target/release/fireshot`

## Usage

Capture with GUI:

```bash
fireshot gui
```

Capture with GUI and delay:

```bash
fireshot gui -d 2000
```

Fullscreen capture (no editor):

```bash
fireshot full -p /tmp/capture.png
```

Fullscreen capture then open editor:

```bash
fireshot full --edit
```

Run tray/daemon:

```bash
fireshot daemon
```

Diagnostics:

```bash
fireshot diagnose --ping
```

## Notes

- Portal selection UI is not used; selection happens inside the overlay editor.
- Clipboard uses `wl-copy`/`xclip` for maximum compatibility.
