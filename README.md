# Fireshot Wayland

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

## First Run Checklist (Portals)

Fireshot uses xdg-desktop-portal for screenshots. If capture fails, check:

1. Portal service is running:

   ```bash
   systemctl --user status xdg-desktop-portal
   ```

2. A portal backend is installed (wlr/gnome/kde):

   ```bash
   ls /usr/share/xdg-desktop-portal/portals
   ```

3. Install a portal backend (examples):

   ```bash
   # Arch
   sudo pacman -S xdg-desktop-portal-wlr
   sudo pacman -S xdg-desktop-portal-gnome
   sudo pacman -S xdg-desktop-portal-kde

   # Debian/Ubuntu
   sudo apt install xdg-desktop-portal-wlr
   sudo apt install xdg-desktop-portal-gnome
   sudo apt install xdg-desktop-portal-kde

   # Fedora
   sudo dnf install xdg-desktop-portal-wlr
   sudo dnf install xdg-desktop-portal-gnome
   sudo dnf install xdg-desktop-portal-kde
   ```

   Pick the backend that matches your compositor/desktop:
   - wlroots/niri/sway/hyprland → `xdg-desktop-portal-wlr`
   - GNOME → `xdg-desktop-portal-gnome`
   - KDE → `xdg-desktop-portal-kde`

4. (Optional) Override backend selection with `portals.conf`:

   ```ini
   # ~/.config/xdg-desktop-portal/portals.conf
   [preferred]
   default=wlr;gtk;

   [niri]
   default=wlr;gtk;
   ```

Note: If multiple backends are installed, the portal will auto-select one based
on `XDG_CURRENT_DESKTOP` unless `portals.conf` overrides it.

5. Run built-in diagnostics:

   ```bash
   fireshot diagnose --ping
   ```

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
fireshot diagnose
```

Portal ping:

```bash
fireshot diagnose --ping
```

## Notes

- Portal selection UI is not used; selection happens inside the overlay editor.
- Clipboard uses `wl-copy`/`xclip` for maximum compatibility.
