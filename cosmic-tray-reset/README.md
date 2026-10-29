# cosmic-tray-reset — restart widgetów z zasobnika

Ikonka w zasobniku (StatusNotifierItem). Kliknięcie restartuje usługi autostartu widgetów z tego repo, gdy stracą łączność z portalem i przestaną reagować.

## Funkcje
- zabija zdublowane instancje (`pkill`), potem restartuje jednostki `systemctl --user` po kolei

## Wymagania
Panel COSMIC z apletem zasobnika, Rust.

## Budowanie
```sh
cd cosmic-tray-reset
cargo build --release
cp target/release/cosmic-tray-reset ~/.local/bin/
```

## Konfiguracja
Lista jednostek: stała `UNITS` w `src/main.rs`.

---

## English

A tray icon (StatusNotifierItem). Clicking it restarts the autostart units of the widgets in this repo when they lose the portal connection and stop responding.

### Features
- kills duplicate instances (`pkill`), then restarts `systemctl --user` units one by one

### Requirements
COSMIC panel with a system tray applet, Rust.

### Building
Same as above.

### Configuration
Unit list: the `UNITS` constant in `src/main.rs`.

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
