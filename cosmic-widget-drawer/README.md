# cosmic-widget-drawer — szuflady od krawędzi ekranu

Dwie wysuwane szuflady layer-shell: prawa z treścią widgetów (pogoda, harmonogram, zadania, kursy), lewa z drzewem vaulta Obsidian. Wywołanie skrótem, gestem gładzika lub najechaniem na krawędź. Korzysta z biblioteki `../cosmic-widget-common` (buduj z katalogu w tym repo). Moduły zadań są współdzielone z `../cosmic-widget-tasks`.

## Funkcje
- demon + klient: `--toggle/--show/--hide`, `--toggle-files`, `--swipe-left/--swipe-right`
- zamykanie `Esc` lub kliknięciem obok, animacja, przyciemnienie tła
- gorący pasek przy krawędzi (konfigurowalny)

## Wymagania
COSMIC (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`.

## Budowanie
```sh
cd cosmic-widget-drawer
cargo build --release
cp target/release/cosmic-widget-drawer ~/.local/bin/
cp data/*.desktop ~/.config/autostart/   # opcjonalnie / optional
```

## Konfiguracja
`~/.config/cosmic-widget-drawer/config.toml`: `sections`, `width`, `edge_trigger`… Ścieżkę vaulta bierze z configu `cosmic-widget-tasks`. Skrót: np. Super+N → `cosmic-widget-drawer --toggle`.

---

## English

Two slide-in layer-shell drawers: the right one with widget content (weather, agenda, tasks, markets), the left one with an Obsidian vault file tree. Opened by shortcut, touchpad gesture or screen-edge hover. Uses the shared crate `../cosmic-widget-common` (build inside this repo). Task modules are shared with `../cosmic-widget-tasks`.

### Features
- daemon + client: `--toggle/--show/--hide`, `--toggle-files`, `--swipe-left/--swipe-right`
- close with `Esc` or click outside, animation, dimmed backdrop
- edge hot-zone (configurable)

### Requirements
COSMIC desktop (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`.

### Building
Same as above.

### Configuration
`~/.config/cosmic-widget-drawer/config.toml`: `sections`, `width`, `edge_trigger`… The vault path is read from the `cosmic-widget-tasks` config. Shortcut example: Super+N → `cosmic-widget-drawer --toggle`.

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
