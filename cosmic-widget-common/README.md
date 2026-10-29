# cosmic-widget-common — wspólny kod widgetów

Biblioteka (crate) używana przez widgety z tego repo — nie jest osobnym programem.

## Funkcje
- `drag` — przeciąganie powierzchni layer-shell myszką i zapis pozycji
- `geometry`/`WidgetPos` — auto-układanie widgetów w kolumnie (`stack_below`)
- `glass` — przezroczystość z suwaka doka + rozmycie tła (`ext-background-effect-v1`)
- `vault_io` — zapis do vaulta lokalnie lub przez SSH na serwer

## Wymagania
COSMIC (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`.

## Budowanie
Budowana automatycznie jako zależność (`path = "../cosmic-widget-common"`). / Built automatically as a path dependency.

## Konfiguracja
Brak — konfigurują ją widgety.

---

## English

Library crate used by the widgets in this repo — not a standalone program.

### Features
- `drag` — mouse dragging of layer-shell surfaces, position saved to config
- `geometry`/`WidgetPos` — automatic column stacking (`stack_below`)
- `glass` — transparency from the dock slider + background blur (`ext-background-effect-v1`)
- `vault_io` — vault writes locally or over SSH to a server

### Requirements
COSMIC desktop (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`.

### Building
Same as above.

### Configuration
None — configured by the widgets.

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
