# cosmic-applet-pobor — pobór prądu w panelu

🇬🇧 [English version](README.md)

Aplet panelu COSMIC pokazujący pobór prądu laptopa (z `/sys/class/power_supply/BAT0`) teraz i średnie z ostatnich godzin.

## Funkcje
- próbka co 30 s, historia 12 h w `~/.local/state/pobor-pradu/`
- popup ze średnimi

## Wymagania
COSMIC, laptop z baterią `BAT0`, Rust.

## Budowanie
```sh
cd cosmic-applet-pobor
sh install.sh   # buduje, instaluje do ~/.local i dopina aplet do panelu
```

## Konfiguracja
Stałe w `src/main.rs` (`BAT`, `CO_ILE_S`, `HISTORIA_H`).

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.
