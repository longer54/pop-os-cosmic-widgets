# cosmic-widget-drawer — drawers from the screen edges

🇵🇱 [Polska wersja](README.pl.md)

Two slide-in layer-shell drawers: the right one with widget content (weather, agenda, tasks, markets), the left one with the Obsidian vault file tree. Opened with a shortcut, a touchpad gesture or by hovering over the screen edge. Uses the shared `../cosmic-widget-common` crate (build it from within this repo). The task modules are shared with `../cosmic-widget-tasks`.

## Features
- daemon + client: `--toggle/--show/--hide`, `--toggle-files`, `--swipe-left/--swipe-right`
- close with `Esc` or by clicking outside, animation, dimmed backdrop
- hot strip along the screen edge (configurable)

## Requirements
COSMIC (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`.

## Building
```sh
cd cosmic-widget-drawer
cargo build --release
cp target/release/cosmic-widget-drawer ~/.local/bin/
cp data/*.desktop ~/.config/autostart/   # optional
```

## Configuration
`~/.config/cosmic-widget-drawer/config.toml`: `sections`, `width`, `edge_trigger`… The vault path is taken from the `cosmic-widget-tasks` config. Shortcut: e.g. Super+N → `cosmic-widget-drawer --toggle`.

---

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
