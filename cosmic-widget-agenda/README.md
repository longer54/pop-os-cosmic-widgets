# cosmic-widget-agenda

Desktop layer-shell widget showing your upcoming Google Calendar events, using the calendar's **private ICS feed** — no OAuth, no Google API client, just a periodic `GET` request.

## Getting your ICS URL

1. Open [calendar.google.com](https://calendar.google.com) on a computer.
2. In the left sidebar, under "My calendars", hover the calendar you want and open its three-dot menu → **Settings and sharing**.
3. Scroll to **Integrate calendar** → **Secret address in iCal format**.

The link looks like `https://calendar.google.com/calendar/ical/you%40gmail.com/private-XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX/basic.ics`.

**Treat this link like a password.** Anyone who has it can read your entire calendar without logging in. Don't commit it, don't paste it anywhere public — it only ever belongs in your local `config.toml`.

## What it does

- Fetches the ICS feed every `refresh_secs` (Google's own cache means the feed itself only updates every several minutes — don't expect real-time sync).
- Parses it with a small hand-written RFC5545 subset (see [`src/ics.rs`](src/ics.rs)) — no external ICS/RRULE crate, to keep the parsing logic auditable in one file. Handles `VEVENT`, `DTSTART`/`DTEND` (`Z`, `TZID`, and all-day `VALUE=DATE` variants), `EXDATE`, and `RRULE` with `FREQ=DAILY/WEEKLY/MONTHLY/YEARLY`, `INTERVAL`, `COUNT`, `UNTIL`, and `BYDAY`.
- **Known limitation:** `BYMONTHDAY`/`BYMONTH`/`BYSETPOS` aren't parsed explicitly. In practice this is usually harmless for `FREQ=YEARLY` recurrences (e.g. birthdays) because Google's own exports set `DTSTART` to a date that already matches those fields, so the day/month just carries through each year — but a hand-edited `RRULE` where `DTSTART` and `BYMONTHDAY`/`BYMONTH` disagree won't be honored.
- Groups events by day ("Today" / "Tomorrow" / weekday + date) and shows a time range (or "all day"); hover a row for its location, if it has one.
- Clicking the card opens `calendar_url` in your browser.

## Config — `~/.config/cosmic-widget-agenda/config.toml`

| Field | Meaning |
|---|---|
| `ics_url` | Your secret ICS feed URL — **required**, empty by default |
| `refresh_secs` | Poll interval |
| `days_ahead` | How many days ahead (including today) to show |
| `max_events` | Cap on events shown |
| `width` | Card width in logical px |
| `anchor` | `top-left` \| `top-right` \| `bottom-left` \| `bottom-right` |
| `margin_x` / `margin_y` | Offset from the anchored corner |
| `calendar_url` | Opened in the browser on click |

## Build & run

```sh
cargo build --release
cp target/release/cosmic-widget-agenda ~/.local/bin/
~/.local/bin/cosmic-widget-agenda &
```
Edit `~/.config/cosmic-widget-agenda/config.toml` and set `ics_url`, then restart.
