# “Minimised” applet — text labels instead of thumbnails (COSMIC)

🇵🇱 [Polska wersja](README.pl.md)

The stock `cosmic-applet-minimize` applet shows a window thumbnail in the panel.
This patch replaces it with a **text label with the app name** (e.g. “Gmail”), styled
like the “Workspaces” / “Applications” buttons. Clicking restores the window; the
tooltip with the name stays. On a vertical panel the thumbnails remain. Names longer
than 20 characters are truncated with “…”.

- Patch: `latka/napisy-zamiast-miniatur.patch` (against the `epoch-<package version>` tag).
- Build and install: `sh build-latka.sh` → `~/.local/bin/cosmic-applet-minimize`
  (PATH takes precedence over /usr/bin, no sudo). Then `pkill -x cosmic-panel`.
- **Rebuild after any `pacman -Syu` that touches `cosmic-applets`** (same as the other applet patches). Note: both scripts use the same source tree
  and run `checkout -f`, so each one builds only its own applet — that's fine.
- Revert: `rm ~/.local/bin/cosmic-applet-minimize && pkill -x cosmic-panel`.
