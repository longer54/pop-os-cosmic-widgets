# cosmic-widget-tasks

Desktop layer-shell widget listing overdue and due-today tasks from an Obsidian vault, matching the semantics of the [Obsidian Tasks](https://github.com/obsidian-tasks-group/obsidian-tasks) plugin's `not done` + `due on or before today` query — without calling any plugin API, just by scanning the vault's Markdown files directly.

## What it does

- Recursively scans the vault (skipping dotfolders and common build directories) for checkbox lines: `- [ ] some text 📅 2026-07-09`.
- Reads the **actual Tasks plugin status configuration** from `.obsidian/plugins/obsidian-tasks-plugin/data.json` (`statusSettings.coreStatuses` / `customStatuses`), so custom statuses (e.g. numeric priority-as-status symbols) are honored instead of guessed. A task counts as "done" (and is excluded) if its status type is `DONE` **or** `CANCELLED` — matching the plugin's actual behavior, which is stricter than the literal `type !== 'DONE'` you might assume from the docs.
- Extracts the due date from the `📅 YYYY-MM-DD` marker and the priority from `🔺⏫🔼🔽⏬` if present; the visible text is everything before the first metadata marker on the line.
- Only bullet-style tasks (`- [ ]`) are recognized; other list markers (`*`, `+`) aren't handled.
- **Sort order matches** `sort by function task.status.symbol === ' ' ? '9' : task.status.symbol` — i.e. sorted by the raw status-symbol character, with the plain "Todo" space-symbol pushed to sort after any custom in-progress symbols (ties broken by due date).
- Clicking a task opens its source note in Obsidian at `obsidian://open?vault={vault_name}&file={path}` (no line-jump — that needs a plugin like Advanced URI, which this widget doesn't depend on).

## Config — `~/.config/cosmic-widget-tasks/config.toml`

| Field | Meaning |
|---|---|
| `vault_path` | Filesystem path to the vault |
| `vault_name` | Vault name as registered in Obsidian (used in the `obsidian://` URI) |
| `refresh_secs` | Re-scan interval |
| `max_tasks` | Cap on tasks shown |
| `width` | Card width in logical px |
| `anchor` | `top-left` \| `top-right` \| `bottom-left` \| `bottom-right` |
| `margin_x` / `margin_y` | Offset from the anchored corner |

## Build & run

```sh
cargo build --release
cp target/release/cosmic-widget-tasks ~/.local/bin/
~/.local/bin/cosmic-widget-tasks &
```
Edit `~/.config/cosmic-widget-tasks/config.toml` (at minimum, set `vault_path` and `vault_name`), then restart.

## If a task shows up that you don't expect (or vice versa)

Check its status symbol against `.obsidian/plugins/obsidian-tasks-plugin/data.json` in your vault — an unrecognized symbol defaults to being treated as `TODO` (i.e. "not done"), matching the plugin's own fallback behavior.
