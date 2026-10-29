# cosmic-tray-reset — restart the widgets from the tray

🇵🇱 [Polska wersja](README.pl.md)

A tray icon (StatusNotifierItem). Clicking it restarts the autostart services of the widgets in this repo when they lose their portal connection and stop responding.

## Features
- kills duplicate instances (`pkill`), then restarts the `systemctl --user` units one by one

## Requirements
A COSMIC panel with the system tray applet, Rust.

## Building
```sh
cd cosmic-tray-reset
cargo build --release
cp target/release/cosmic-tray-reset ~/.local/bin/
```

## Configuration
List of units: the `UNITS` constant in `src/main.rs`.

---

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
