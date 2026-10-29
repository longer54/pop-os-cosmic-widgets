# cosmic-widget-common — shared widget code

🇵🇱 [Polska wersja](README.pl.md)

A library crate used by the widgets in this repo — not a standalone program.

## Features
- `drag` — dragging layer-shell surfaces with the mouse and saving their position
- `geometry`/`WidgetPos` — automatic stacking of widgets in a column (`stack_below`)
- `glass` — transparency taken from the dock's slider + background blur (`ext-background-effect-v1`)
- `vault_io` — writing to the vault locally or over SSH to a server

## Requirements
COSMIC (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`.

## Building
Built automatically as a path dependency (`path = "../cosmic-widget-common"`).

## Configuration
None — it is configured by the widgets.

---

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
