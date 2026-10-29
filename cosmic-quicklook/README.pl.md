# cosmic-quicklook — Quick Look dla COSMIC

🇬🇧 [English version](README.md)

Podgląd zaznaczonego pliku (obraz, PDF, tekst, wideo) w lekkim oknie nad wszystkim, jak Quick Look w macOS. Wywołanie skrótem (domyślnie Super+Spacja). Korzysta z biblioteki `../cosmic-widget-common` (buduj z katalogu w tym repo).

## Funkcje
- odczyt zaznaczenia z menedżera plików (COSMIC Files i inne z listy)
- PDF (`pdftoppm`), wideo (`ffmpegthumbnailer`), obrazy (`magick`), tekst
- demon + klient (`--toggle`, ścieżka pliku)

## Wymagania
COSMIC (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`. Opcjonalnie: `poppler-utils`, `ffmpegthumbnailer`, `ffmpeg`, `imagemagick`, `wl-clipboard`.

## Budowanie
```sh
cd cosmic-quicklook
cargo build --release
cp target/release/cosmic-quicklook ~/.local/bin/
```

## Konfiguracja
`~/.config/cosmic-quicklook/config.toml`: `width_pct`, `height_pct`, `file_managers`, `pdf_dpi`, `text_max_kb`.

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.
