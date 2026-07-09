# cosmic-calendar-widget

Desktop layer-shell calendar widget that mirrors an Obsidian vault's daily notes 1:1 — no Obsidian API involved, it reads the vault's files directly from disk.

## What it does

- Renders a month grid. Clicking any day (including days from the adjacent month shown in the grid) opens that day's daily note in Obsidian, creating it from a template first if it doesn't exist yet.
- Dots under each day mirror the community **Calendar** plugin's own algorithm (`getWordLengthAsDots`): word count of the note, `clamp(words / words_per_dot, 1, max_dots)`. A note that exists but is empty still gets 1 dot; a day with no note gets 0.
- Re-scans the visible month from disk every 30 seconds and whenever you change month/jump to today, so edits made directly in Obsidian show up without restarting the widget.
- National holidays (currently: Polish fixed + Easter-based holidays, see [`src/holidays.rs`](src/holidays.rs)) are marked in the grid and listed below the calendar for the next 3 upcoming ones. Swap this module out if you need a different country's holiday calendar.

## Requirements

- An Obsidian vault with the **Daily notes** core plugin (or a compatible folder/filename convention) — the widget writes plain files, it doesn't call the Obsidian API.
- Obsidian installed and the vault registered under the exact name you put in `vault_name` (needed for the `obsidian://open?vault=...` URI to resolve). If you're on the Flatpak build of Obsidian, the registered name is in `~/.var/app/md.obsidian.Obsidian/config/obsidian/obsidian.json`.

## Config — `~/.config/cosmic-calendar-widget/config.toml`

| Field | Meaning |
|---|---|
| `vault_path` | Filesystem path to the vault |
| `vault_name` | Vault name as registered in Obsidian (used in the `obsidian://` URI) |
| `daily_notes_folder` | Folder inside the vault holding daily notes |
| `daily_note_template` | Path (relative to vault, no extension) to a template file; supports simple `{{date}}` / `{{title}}` placeholders (string substitution, not the Templater plugin). Leave empty for no template. |
| `words_per_dot` | Word count per dot (matches the Calendar plugin's own setting) |
| `max_dots` | Cap on dots per day |
| `width` | Card width in logical px |
| `anchor` | `top-left` \| `top-right` \| `bottom-left` \| `bottom-right` |
| `margin_x` / `margin_y` | Offset from the anchored corner |

## Build & run

```sh
cargo build --release
cp target/release/cosmic-calendar-widget ~/.local/bin/
~/.local/bin/cosmic-calendar-widget &
```
Edit `~/.config/cosmic-calendar-widget/config.toml` (at minimum, set `vault_path` and `vault_name`), then restart.
