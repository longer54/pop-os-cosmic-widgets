# cosmic-widget-panel

Desktop layer-shell widget showing the status of your own services/servers, fetched from a JSON HTTP endpoint you control. This is a **client only** — you need a backend somewhere serving the expected JSON shape below.

## What it does

- Polls `GET {api_url}` every `refresh_secs` with header `X-Api-Key: {api_key}`.
- Renders one line per host with an overall OK/total count and optional metrics (RAM %, disk %, temperature), then a line per service (all of them, or only the non-`up` ones, depending on `show_all`).
- Clicking the card opens `panel_url` in your browser.

## Expected JSON response

```json
{
  "hosts": {
    "my-server": {
      "services": [
        { "service": "nginx", "status": "up", "last_check": "2026-07-09T12:00:00Z", "detail": null },
        { "service": "postgres", "status": "down", "last_check": "2026-07-09T12:00:00Z", "detail": "connection refused" }
      ],
      "metrics": { "ram_pct": 35.0, "hdd_pct": 75.0, "temp_c": 38.0 }
    }
  }
}
```

- `status` — `"up"` renders green, `"stale"` renders orange, anything else renders red.
- `detail` — optional, shown as a tooltip on hover.
- `metrics` — optional; omit fields you don't have.
- `last_check` — currently parsed but not displayed; keep it for your own future use or drop it.

See [`src/api.rs`](src/api.rs) for the exact Serde structs.

## Config — `~/.config/cosmic-widget-panel/config.toml`

| Field | Meaning |
|---|---|
| `api_url` | Your status endpoint (see above) |
| `panel_url` | Opened in the browser on click |
| `api_key` | Sent as `X-Api-Key` — **required**, the widget shows a hint until this is set |
| `refresh_secs` | Poll interval (minimum enforced: 30s) |
| `width` | Card width in logical px |
| `anchor` | `top-left` \| `top-right` \| `bottom-left` \| `bottom-right` |
| `margin_x` / `margin_y` | Offset from the anchored corner |
| `show_all` | `true` = list every service; `false` = only non-`up` ones plus the summary |

## Build & run

```sh
cargo build --release
cp target/release/cosmic-widget-panel ~/.local/bin/
~/.local/bin/cosmic-widget-panel &
```
Edit `~/.config/cosmic-widget-panel/config.toml` (at minimum, set `api_url` and `api_key`), then restart.
