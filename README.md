# Fireshot Wayland

Fireshot is a Wayland-first screenshot tool with an in-place selection overlay,
editor tools, and a tray/daemon mode. It uses the desktop portal for capture so
it stays compositor-agnostic.

## Requirements

- Wayland session
- xdg-desktop-portal + a backend (wlr/gnome/kde) depending on the type of the desktop
- `wl-copy` and `xclip` for clipboard integration

## ❗ Important

Make sure you have installed `wl-copy` and `xclip`

## Make sure the portal is properly configured

Fireshot uses `xdg-desktop-portal` for screenshots.

Each desktop has it's preffered portal backend:

- `hyprland`, `niri`, `sway`, `wlroots` - `xdg-desktop-portal-wlr`
- `GNOME` - `xdg-desktop-portal-gnome`
- `KDE` - `xdg-desktop-portal-kde`

See the XDG_CURRENT_DESKTOP and the available portal backends that are installed on your machine with `fireshot diagnose`.
If portal service is `false` meaning that it is not running, start it with `systemctl --user restart xdg-desktop-portal` and then restart the backend with one of the following, depending on the desktop type:

- `systemctl --user restart xdg-desktop-portal-wlr`
- `systemctl --user restart xdg-desktop-portal-gnome`
- `systemctl --user restart xdg-desktop-portal-kde`
- `systemctl --user restart xdg-desktop-portal-gtk`

## ❗ Important for `niri`

`niri` does not have it's own portal backend. But it can work well with the backends of other desktops, for example `xdg-desktop-portal-wlr` or `xdg-desktop-portal-gnome`.
Most probably `fireshot` will fail to work on niri because niri defaults to `xdg-desktop-portal-gtk`, but this backend does not offer screenshot support so it is needed to tell `niri` to use a different backend for taking screenshots. For this, it is needed to create a file `~/.config/xdg-desktop-portal/niri-portals.conf` with the following content:

```
[preferred]
default=wlr;gnome;gtk;

org.freedesktop.impl.portal.Screenshot=wlr;gnome;gtk;
```

if the file is already present, just add `org.freedesktop.impl.portal.Screenshot=wlr;gnome;gtk` to it.

**Then restart the desktop session**.

## Install

```bash
cargo install --path crates/app
```

Binary: `fireshot` (in your cargo bin path)

## Usage

Run `fireshot --gelp` to see usage examples.
