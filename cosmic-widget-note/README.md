# cosmic-widget-note — a note on the desktop

🇵🇱 [Polska wersja](README.pl.md)

A sticky note on the desktop with a single general-purpose note stored as a `.md` file in your Obsidian vault — the same content you see in Obsidian. Uses the shared `../cosmic-widget-common` crate (build it from within this repo).

## Features
- preview without stealing focus; click → edit, autosave, `Esc` to finish
- archive the note into a folder, an “open in Obsidian” button
- optional saving over SSH to a server holding the canonical copy of the vault (avoids Syncthing conflicts)
- `--new` / `--edit` for binding to keyboard shortcuts

## Requirements
COSMIC (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`. An Obsidian vault (local directory).

## Building
```sh
cd cosmic-widget-note
cargo build --release
cp target/release/cosmic-widget-note ~/.local/bin/
cp data/*.desktop ~/.config/autostart/   # optional
```

## Configuration
`~/.config/cosmic-widget-note/config.toml`: `vault_path` (default `~/Obsidian`), `vault_name`, `note_file`, `archive_folder`; empty `remote_host`/`remote_vault_path` = save locally.

---

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
