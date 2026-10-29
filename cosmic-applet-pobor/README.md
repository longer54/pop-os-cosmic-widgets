# cosmic-applet-pobor — pobór prądu w panelu

Aplet panelu COSMIC pokazujący pobór prądu laptopa (z `/sys/class/power_supply/BAT0`) teraz i średnie z ostatnich godzin.

## Funkcje
- próbka co 30 s, historia 12 h w `~/.local/state/pobor-pradu/`
- popup ze średnimi

## Wymagania
COSMIC, laptop z baterią `BAT0`, Rust.

## Budowanie
```sh
cd cosmic-applet-pobor
sh install.sh   # buduje, instaluje do ~/.local i dopina aplet do panelu / builds, installs, adds to panel
```

## Konfiguracja
Stałe w `src/main.rs` (`BAT`, `CO_ILE_S`, `HISTORIA_H`).

---

## English

COSMIC panel applet showing laptop power draw (from `/sys/class/power_supply/BAT0`) now and averages over the last hours.

### Features
- sample every 30 s, 12 h history in `~/.local/state/pobor-pradu/`
- popup with averages

### Requirements
COSMIC, a laptop with a `BAT0` battery, Rust.

### Building
Same as above.

### Configuration
Constants in `src/main.rs` (`BAT`, `CO_ILE_S`, `HISTORIA_H`).

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
