# COSMIC panel applet patch guard

🇵🇱 [Polska wersja](README.pl.md)

The patched applets live in `~/.local/bin` and must match the version of the
`cosmic-applets` package:

| binary | patch |
|---|---|
| `cosmic-applet-battery` | `tryby-mocy-cpu/` (not in this repo — skipped if missing) |
| `cosmic-applet-minimize` | `cosmic-applet-minimize-napisy/` |
| `cosmic-applet-time` | `cosmic-applet-zegar-obsidian/` |

The `cosmic-applets-latki.timer` timer (systemd --user, every 15 min + 3 min after
boot) runs `auto-przebuduj.sh`. When the package version has changed, it
rebuilds every patch present on the machine in the background, runs `pkill -x cosmic-panel`
and sends a notification. If a build fails: that patch falls back to the stock version,
a critical notification is shown, and no retry happens until the next update.

- Installing the timer: create `~/.config/systemd/user/cosmic-applets-latki.{service,timer}`
  (the service runs `auto-przebuduj.sh`, the timer has `OnBootSec=3min`, `OnUnitActiveSec=15min`).
- State: `~/.local/state/cosmic-applets-latki.wersja`, log: `~/.local/state/cosmic-applets-latki.log`.
- Forcing a rebuild: write any other version into the state file and run
  `systemctl --user start cosmic-applets-latki.service`.
