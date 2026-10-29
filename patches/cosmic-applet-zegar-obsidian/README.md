# Clock applet — Obsidian mini calendar (COSMIC)

🇵🇱 [Polska wersja](README.pl.md)

Clicking the date/time in the top panel opens a mini calendar that works like
the Calendar plugin in Obsidian (and like the `cosmic-applet-calendar` widget):

- **click a day** → creates `Kalendarz/YYYY-MM-DD.md` from a template
  (if it doesn't exist) and opens it; the popup closes. Days from
  adjacent months (dimmed) work too.
- **dots under a day** = note length (1 dot / 250 words, max 5;
  an existing empty note = 1 dot).
- today is highlighted with the accent colour.

Settings (vault, folder, template, dots, **Obsidian vs .md file**) are read
from the same file as the widget: `~/.config/cosmic-calendar-widget/config.toml`
— the `open_in_obsidian` key (the “Obsidian” toggle in the widget) applies here too.
The file is read every time the popup opens, so no restart is needed.

- Patch: `latka/kalendarzyk-obsidiana.patch` (against the `epoch-<package version>` tag).
- Build and install: `sh build-latka.sh` → `~/.local/bin/cosmic-applet-time`
  (PATH takes precedence over /usr/bin, no sudo). Then `pkill -x cosmic-panel`.
- **Rebuild after any `pacman -Syu` that touches `cosmic-applets`** (same as the
  minimise and battery patches).
- Revert: `rm ~/.local/bin/cosmic-applet-time && pkill -x cosmic-panel`.
