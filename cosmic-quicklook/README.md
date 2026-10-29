# cosmic-quicklook — Quick Look for COSMIC

🇵🇱 [Polska wersja](README.pl.md)

Preview the selected file (image, PDF, text, video) in a lightweight window above everything else, like Quick Look on macOS. Triggered by a shortcut (Super+Space by default). Uses the shared `../cosmic-widget-common` crate (build it from within this repo).

## Features
- reads the selection from the file manager (COSMIC Files and others on the list)
- PDF (`pdftoppm`), video (`ffmpegthumbnailer`), images (`magick`), text
- daemon + client (`--toggle`, file path)

## Requirements
COSMIC (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`. Optional: `poppler-utils`, `ffmpegthumbnailer`, `ffmpeg`, `imagemagick`, `wl-clipboard`.

## Building
```sh
cd cosmic-quicklook
cargo build --release
cp target/release/cosmic-quicklook ~/.local/bin/
```

## Configuration
`~/.config/cosmic-quicklook/config.toml`: `width_pct`, `height_pct`, `file_managers`, `pdf_dpi`, `text_max_kb`.

---

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
