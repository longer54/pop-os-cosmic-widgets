# cosmic-quicklook — Quick Look dla COSMIC

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

## English

Preview of the selected file (image, PDF, text, video) in a lightweight overlay, like macOS Quick Look. Triggered by a shortcut (Super+Space suggested). Uses the shared crate `../cosmic-widget-common` (build inside this repo).

### Features
- reads the selection from the file manager (COSMIC Files and others)
- PDF (`pdftoppm`), video (`ffmpegthumbnailer`), images (`magick`), text
- daemon + client (`--toggle`, file path)

### Requirements
COSMIC desktop (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`. Optional: `poppler-utils`, `ffmpegthumbnailer`, `ffmpeg`, `imagemagick`, `wl-clipboard`.

### Building
Same as above.

### Configuration
`~/.config/cosmic-quicklook/config.toml`: `width_pct`, `height_pct`, `file_managers`, `pdf_dpi`, `text_max_kb`.

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
