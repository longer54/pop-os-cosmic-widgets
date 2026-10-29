# cosmic-applet-pobor — power draw in the panel

🇵🇱 [Polska wersja](README.pl.md)

A COSMIC panel applet showing the laptop's current power draw (from `/sys/class/power_supply/BAT0`) and averages over the last few hours.

## Features
- a sample every 30 s, 12 h of history in `~/.local/state/pobor-pradu/`
- popup with the averages

## Requirements
COSMIC, a laptop with a `BAT0` battery, Rust.

## Building
```sh
cd cosmic-applet-pobor
sh install.sh   # builds, installs into ~/.local and adds the applet to the panel
```

## Configuration
Constants in `src/main.rs` (`BAT`, `CO_ILE_S`, `HISTORIA_H`).

---

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
