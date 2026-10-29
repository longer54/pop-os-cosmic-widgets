# cosmic-widget-markets — kursy na pulpicie

🇬🇧 [English version](README.md)

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

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.
