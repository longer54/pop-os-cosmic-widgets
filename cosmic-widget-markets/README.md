# cosmic-widget-markets — market prices on the desktop

🇵🇱 [Polska wersja](README.pl.md)

A desktop widget (layer-shell) showing the price of one asset at a time: cryptocurrencies (CoinGecko), stocks, commodities and ETFs (Yahoo Finance). Uses the shared `../cosmic-widget-common` crate (build it from within this repo).

## Features
- price and change, category tabs (Favourites/Crypto/US/WSE/Commodities/ETF), arrows to navigate
- click the price → full chart (24h/7D/1M/1Y, lightweight-charts) in the browser
- search and add assets (the + button)
- drag with the mouse, automatic stacking below other widgets (`stack_below`)

## Requirements
COSMIC (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`. Internet access (public APIs, no key needed).

## Building
```sh
cd cosmic-widget-markets
cargo build --release
cp target/release/cosmic-widget-markets ~/.local/bin/
```

## Configuration
`~/.config/cosmic-widget-markets/config.toml` — created on first run with a sample watchlist (`assets`), `refresh_secs` and position (`anchor`, `margin_x/y`).

---

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
