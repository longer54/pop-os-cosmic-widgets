# cosmic-widget-common — wspólny kod widgetów

🇬🇧 [English version](README.md)

Biblioteka (crate) używana przez widgety z tego repo — nie jest osobnym programem.

## Funkcje
- `drag` — przeciąganie powierzchni layer-shell myszką i zapis pozycji
- `geometry`/`WidgetPos` — auto-układanie widgetów w kolumnie (`stack_below`)
- `glass` — przezroczystość z suwaka doka + rozmycie tła (`ext-background-effect-v1`)
- `vault_io` — zapis do vaulta lokalnie lub przez SSH na serwer

## Wymagania
COSMIC (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`.

## Budowanie
Budowana automatycznie jako zależność (`path = "../cosmic-widget-common"`).

## Konfiguracja
Brak — konfigurują ją widgety.

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.
