# cosmic-widget-weather

Desktop layer-shell widget showing the current weather and a short forecast, defaulting to **Lublin, Poland** — but usable for any location by editing two config values.

## What it does

- **Current conditions from a real weather station**, not an interpolated model: fetches Poland's public IMGW synoptic data feed (`https://danepubliczne.imgw.pl/api/data/synop`, no API key) and picks the station named `imgw_station` in the config. Shows temperature, wind speed/direction, humidity, pressure, and precipitation.
- **Short forecast from [Open-Meteo](https://open-meteo.com/)** (no API key) for the exact `lat`/`lon` in the config — daily min/max temperature, precipitation probability, and a plain-language description derived from the WMO weather code.
- The two sources are fetched **concurrently** and **independently**: if one is unreachable, the widget still shows whatever the other returned, with a small warning noting what failed. It only shows a hard error if both fail.
- Clicking the card opens `weather_url` in your browser.

**Using this outside Poland:** the Open-Meteo forecast works anywhere — just change `lat`/`lon`. The IMGW current-conditions feed only has Polish stations; if your station isn't in there, leave `imgw_station` pointing at whatever's closest, or adapt `fetch_current` in [`src/weather.rs`](src/weather.rs) to a different national/regional weather-station API — the polling and rendering code doesn't care where the numbers come from.

## Config — `~/.config/cosmic-widget-weather/config.toml`

| Field | Meaning |
|---|---|
| `lat` / `lon` | Coordinates used for the Open-Meteo forecast |
| `imgw_station` | Station name to match in the IMGW feed (case-insensitive) |
| `refresh_secs` | Poll interval — IMGW updates roughly hourly, so there's little value going much below the 600s default |
| `forecast_days` | How many days (including today) to show |
| `width` | Card width in logical px |
| `anchor` | `top-left` \| `top-right` \| `bottom-left` \| `bottom-right` |
| `margin_x` / `margin_y` | Offset from the anchored corner |
| `weather_url` | Opened in the browser on click |

## Build & run

```sh
cargo build --release
cp target/release/cosmic-widget-weather ~/.local/bin/
~/.local/bin/cosmic-widget-weather &
```

First run writes a default `config.toml` for Lublin. Edit `lat`/`lon`/`imgw_station` if you want a different location, then restart.
