# Pop!_OS COSMIC Desktop Widgets

Lightweight desktop widgets, panel applets and small tools for the [COSMIC desktop](https://github.com/pop-os/cosmic-epoch) (Pop!_OS), written in Rust with [libcosmic](https://github.com/pop-os/libcosmic)/iced. Each widget is a standalone binary that pins itself to a corner of the screen via Wayland's layer-shell protocol and refreshes its content on a timer — no panel applet API required.

## Widgets

| Widget | What it shows | Data source |
|---|---|---|
| [`cosmic-widget-panel`](cosmic-widget-panel/) | Status of your own services/servers | Your own JSON HTTP endpoint |
| [`cosmic-calendar-widget`](cosmic-calendar-widget/) | Obsidian daily-notes calendar (click a day to open/create its note) | Local Obsidian vault files |
| [`cosmic-widget-agenda`](cosmic-widget-agenda/) | Upcoming events from a Google Calendar | Private ICS feed (no OAuth) |
| [`cosmic-widget-tasks`](cosmic-widget-tasks/) | Overdue / due-today tasks (Obsidian [Tasks](https://github.com/obsidian-tasks-group/obsidian-tasks) plugin syntax) | Local Obsidian vault files |
| [`cosmic-widget-weather`](cosmic-widget-weather/) | Current weather (defaults to Lublin, PL) + short forecast | Public IMGW station feed + Open-Meteo |
| [`cosmic-widget-markets`](cosmic-widget-markets/) | Asset prices + charts (crypto, stocks, commodities, ETFs) | CoinGecko + Yahoo Finance (public) |
| [`cosmic-widget-note`](cosmic-widget-note/) | A sticky note on the desktop, stored as a `.md` file | Local Obsidian vault |
| [`cosmic-widget-drawer`](cosmic-widget-drawer/) | Slide-in drawers: widget content (right) and vault file tree (left) | Other widgets' data sources |

### Panel applets and tools

| Project | What it does |
|---|---|
| [`cosmic-quicklook`](cosmic-quicklook/) | macOS-style Quick Look: preview the selected file (image, PDF, text, video) |
| [`cosmic-okno`](cosmic-okno/) | CLI to show/hide/toggle windows from keyboard shortcuts |
| [`cosmic-tray-reset`](cosmic-tray-reset/) | Tray icon that restarts the desktop widgets |
| [`cosmic-applet-pobor`](cosmic-applet-pobor/) | Panel applet: laptop power draw now + hourly averages |
| [`cosmic-applet-nocny-filtr`](cosmic-applet-nocny-filtr/) | Panel applet: night light slider (drives `drm-colortemp`) |
| [`cosmic-widget-common`](cosmic-widget-common/) | Shared crate: mouse drag, auto-stacking, glass/blur, vault I/O |
| [`patches/`](patches/) | Patches for `cosmic-comp` and COSMIC panel applets (window size memory, minimized-window labels, Obsidian clock) |

Each widget has its own README with config details and setup steps.

## Common architecture

- **Layer-shell surface** (`Layer::Bottom`: above the wallpaper, below windows, visible on every workspace), anchored to a screen corner and offset by a configurable margin — position is deterministic across restarts.
- **Explicit size + resize on refresh.** Free-floating layer-shell surfaces don't auto-size to their content the way panel applets do; every widget computes an estimated height from its current content and calls `set_size` after each data refresh.
- **Glass-style card UI** — dark, semi-transparent rounded card, consistent across all widgets.
- **Config file** at `~/.config/<widget-name>/config.toml`, auto-created with placeholder defaults on first run — edit it, then restart the widget.
- **Autostart** via a `.desktop` file you copy into `~/.config/autostart/`.

## Requirements

- Pop!_OS with the COSMIC desktop (`cosmic-comp` ≥ 1.0 — this is what exposes `zwlr_layer_shell_v1` to regular applications, not just applets).
- Rust via [rustup](https://rustup.rs) — edition 2024, `rust-version` 1.85.
- `libxkbcommon-dev` (system package). If you can't install system packages (no sudo), see Troubleshooting below.

## Building and installing a widget

```sh
cd cosmic-widget-panel   # or whichever widget you want
cargo build --release
cp target/release/cosmic-widget-panel ~/.local/bin/
cp data/*.desktop ~/.config/autostart/    # optional: launch automatically on login
~/.local/bin/cosmic-widget-panel &        # first run writes a default config.toml
```

Edit the freshly created `~/.config/<widget-name>/config.toml` (each widget needs at least one value filled in — an API key, an ICS URL, a vault path, etc.), then restart the widget for the change to take effect.

## Positioning multiple widgets in the same corner

All widgets share the same `anchor` (`top-left` / `top-right` / `bottom-left` / `bottom-right`) and `margin_x` / `margin_y` config fields. To stack two widgets in the same corner, give the second one a `margin_y` (or `margin_x`, if placing side by side) large enough to clear the first widget's height (or width) plus a small gap. Each widget's default config in this repo documents the arithmetic it assumes for its own default position — treat those as a starting point, not a fixed layout.

## Troubleshooting

- **Missing `libxkbcommon-dev` and no sudo access**: symlink `~/.local/lib/libxkbcommon.so -> libxkbcommon.so.0` (wherever the shared library actually lives on your system) and write a matching `.pc` file under `~/.local/lib/pkgconfig`, then `export PKG_CONFIG_PATH=$HOME/.local/lib/pkgconfig` before `cargo build`.
- **Widget never resizes to fit its content / content looks clipped at the bottom**: the estimated-height calculation in that widget's `app.rs` is under-counting real row height. Increase the per-row pixel constant in `estimated_height()` — better to over-provision (extra blank space) than clip content, since layer-shell surfaces don't scroll.
- **Binary built on one machine doesn't run on another**: `target/` is architecture/toolchain-specific and isn't meant to be copied between machines — rebuild with `cargo build --release` on each one.

## Po polsku (skrót)

Zestaw widgetów pulpitowych, apletów panelu i drobnych narzędzi dla pulpitu
COSMIC (Pop!_OS), w Ruście (libcosmic/iced). Każdy katalog to osobny program
z własnym README; konfiguracja w `~/.config/<nazwa>/config.toml` (tworzona
przy pierwszym uruchomieniu z neutralnymi wartościami domyślnymi). Budowanie:
`cargo build --release` w katalogu projektu. Teksty interfejsu są po polsku.

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).

## Zastrzeżenie / Disclaimer

PL: Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.

EN: Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
