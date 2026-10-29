# cosmic-widget-markets — kursy na pulpicie

Widget pulpitowy (layer-shell) z ceną jednego aktywa naraz: kryptowaluty (CoinGecko), akcje, surowce i ETF (Yahoo Finance). Korzysta z biblioteki `../cosmic-widget-common` (buduj z katalogu w tym repo).

## Funkcje
- cena i zmiana, zakładki kategorii (Ulubione/Krypto/US/GPW/Surowce/ETF), strzałki do nawigacji
- klik w cenę → pełny wykres (24h/7D/1M/1R, lightweight-charts) w przeglądarce
- wyszukiwarka i dodawanie aktywów (przycisk +)
- przeciąganie myszką, auto-układanie pod innymi widgetami (`stack_below`)

## Wymagania
COSMIC (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`. Dostęp do internetu (publiczne API, bez klucza).

## Budowanie
```sh
cd cosmic-widget-markets
cargo build --release
cp target/release/cosmic-widget-markets ~/.local/bin/
```

## Konfiguracja
`~/.config/cosmic-widget-markets/config.toml` — tworzony przy pierwszym uruchomieniu z przykładową watchlistą (`assets`), `refresh_secs`, pozycja (`anchor`, `margin_x/y`).

---

## English

Desktop (layer-shell) widget showing one asset price at a time: crypto (CoinGecko), stocks, commodities and ETFs (Yahoo Finance). Uses the shared crate `../cosmic-widget-common` (build inside this repo).

### Features
- price and change, category tabs, arrow navigation
- click the price → full chart (24h/7D/1M/1Y, lightweight-charts) in the browser
- search and add assets (+ button)
- mouse drag, auto-stacking below other widgets (`stack_below`)

### Requirements
COSMIC desktop (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`. Internet access (public APIs, no key).

### Building
Same as above.

### Configuration
`~/.config/cosmic-widget-markets/config.toml` — created on first run with a sample watchlist (`assets`), `refresh_secs`, position (`anchor`, `margin_x/y`).

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
