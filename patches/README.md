# patches — patches for COSMIC components

🇵🇱 [Polska wersja](README.pl.md)

Patches (`.patch`) against upstream System76 repositories, plus scripts that clone
upstream, apply the patches, build and install the binary into `~/.local/bin`
(system packages are left untouched). Use at your own risk — you are replacing
your own compositor / panel applets.

| Directory | Target | What it does |
|---|---|---|
| [`cosmic-comp-pamiec-okien`](cosmic-comp-pamiec-okien/) | `cosmic-comp` | remembers the size/position of floating windows, optionally borderless windows |
| [`cosmic-applet-minimize-napisy`](cosmic-applet-minimize-napisy/) | `cosmic-applet-minimize` | text labels for minimised windows |
| [`cosmic-applet-zegar-obsidian`](cosmic-applet-zegar-obsidian/) | `cosmic-applet-time` | clicking a calendar day opens the Obsidian daily note |
| [`cosmic-applets-latki`](cosmic-applets-latki/) | `cosmic-applets` | shared script that builds the patched applets + automatic rebuild after updates |

Each directory has its own README (in English; Polish version in `README.pl.md`).
The automatic rebuilds are optional `systemd --user` units (timers) that you create yourself.
