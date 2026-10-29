# cosmic-comp — custom patches for the COSMIC compositor

🇵🇱 [Polska wersja](README.pl.md)

This directory holds patches for `cosmic-comp` and a shared `build.sh` that builds
the compositor with **all** `*.patch` files from here and installs the binary into
`~/.local/bin` (no sudo; the stock binary in `/usr/bin` stays untouched).

- `pamiec-okien.patch` — per-application window size and position
- `bez-ramki.patch` (2026-10-04) — no yellow active-window border for selected apps;
  list of app_ids in `~/.config/cosmic-bez-ramki` (one per line), empty by default.
  Build: `LATKI="pamiec-okien.patch bez-ramki.patch" sh build.sh`; revert:
  `mv ~/.local/bin/cosmic-comp.bak-przed-bez-ramki ~/.local/bin/cosmic-comp` + log out and back in.

---

## Patch 1: window memory (per-application size and position)

The equivalent of KDE's “Remember size and position” window rule, which
COSMIC doesn't have. A patch for the `cosmic-comp` compositor.

## The problem

Telegram and Claude Desktop opened smaller than they had been sized every time
they were launched. Other apps seemed to work fine.

The cause is in cosmic-comp itself, `src/shell/layout/floating/mod.rs`,
function `map_internal()` — a new floating window is clamped to **2/3 of
the screen**:

```rust
let mut width = output_geometry.size.w / 3 * 2;
```

Measured live (1920×1080 screen): the window asked for 1700×950 and got
**1280×698** — exactly 2/3 of the area excluding the panel. That's why “everything else works”:
smaller windows never hit this limit.

The second problem: cosmic-comp only remembers geometry for the lifetime of a window
(`CosmicMapped::last_geometry`). Once the app is closed, that knowledge is lost.

It **can't be fixed from the outside** — the `zcosmic_toplevel_manager_v1` protocol
only has `close`, `activate`, `set_maximized/minimized/fullscreen/sticky`
and `move_to_workspace`. There's no request to change size or position, so
no KWin-rules-style daemon is possible.

## What the patch does

- a new module `src/utils/window_memory.rs` — stores the last geometry under
  the `app_id` key in `~/.local/state/cosmic-comp/window-geometry.ron`
  (the file is human-readable and can be edited/deleted by hand);
- `FloatingLayout::unmap()` — saves the geometry when a window closes
  (skips maximised and tiled windows);
- `FloatingLayout::map_internal()` — when a window is mapped for the first time, restores
  the remembered size and position, **bypassing the 2/3 limit**, and clamps
  the result to the screen area;
- `CosmicMapped` gets a `saved_app_id` field — the `app_id` has to be stored
  at map time, because when the window closes the toplevel may already be dead and
  it can no longer be read from the surface.

Works for all applications, regardless of whether they save
their own geometry.

## Installation

```
sh build.sh
```

The script checks `pacman -Q cosmic-comp` itself, clones the sources at the same
version (tag `epoch-X.Y.Z`), applies the patch, builds and installs the binary into
`~/.local/bin/cosmic-comp`. Then log out and log back in.

**No sudo.** `cosmic-session` starts `cosmic-comp` by name from PATH,
and `~/.local/bin` comes first in that PATH (verified in `/proc/<pid>/environ`
of the running compositor). The stock binary in `/usr/bin` stays untouched.

## Reverting

```
rm ~/.local/bin/cosmic-comp
```

and log in again. The original compositor from `/usr/bin` is back.

## After a cosmic-comp update

Pacman updates `/usr/bin/cosmic-comp` but does **not** touch the one in `~/.local/bin`.
The compositor and session versions must match (otherwise `cosmic-session` logs
“Unknown session socket message, are you using incompatible cosmic-session
and cosmic-comp versions?”), so after every update you need to run
`sh build.sh` again. If anything goes wrong — `rm ~/.local/bin/cosmic-comp`
and you're back where you started.

## Other machines

The build and the binary `cp` have to be repeated on every computer — just like
with the COSMIC widgets.


---

## Patch 2: night light — NOT included

An earlier “night light” patch (gamma inside the compositor) had a bug
(wrong gamma table size → washed-out screen until restart), so it isn't
in this repo. Night light is now handled by the `../../cosmic-applet-nocny-filtr/` applet
via the external `drm-colortemp` daemon.

## Automation: rebuild after a system update (2026-09-16)

The patched binary must be the same version as `cosmic-session` from the package.
So you don't have to remember this on every `updateall` / `pacman -Syu`,
a `systemd --user` timer checks it every 15 minutes:

- `auto-przebuduj.sh` — compares `--version` of the patched build and `/usr/bin/cosmic-comp`;
  on a mismatch it **removes the outdated patched binary first** (so logging in
  is safe even if the build doesn't finish in time — the stock compositor starts,
  just without window memory), then rebuilds and installs the new one, and finally
  runs `notify-send`. Log: `~/.local/state/cosmic-comp-latka.log`.
- Units: `~/.config/systemd/user/cosmic-comp-latka.{timer,service}` —
  you need to create them yourself (the service runs `auto-przebuduj.sh`,
  the timer fires every 15 min) on every computer that has `~/.local/bin/cosmic-comp`.
- Status: `systemctl --user list-timers cosmic-comp-latka.timer`
  Disable: `systemctl --user disable --now cosmic-comp-latka.timer`

The remaining gap: a system update followed by an **immediate** reboot within
the same 15 minutes. Blind recovery in that case: Ctrl+Alt+F3,
`rm ~/.local/bin/cosmic-comp`, then log in normally.
