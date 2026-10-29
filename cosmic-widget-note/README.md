# cosmic-widget-note — notatka na pulpicie

Kartka na pulpicie z jedną ogólną notatką trzymaną jako plik `.md` w vaultcie Obsidian — ta sama treść widoczna w Obsidianie. Korzysta z biblioteki `../cosmic-widget-common` (buduj z katalogu w tym repo).

## Funkcje
- podgląd bez kradzenia fokusu; klik → edycja, autozapis, `Esc` kończy
- archiwizacja notatki do folderu, przycisk „otwórz w Obsidianie”
- opcjonalny zapis przez SSH na serwer z kanoniczną kopią vaulta (unika konfliktów Syncthing)
- `--new` / `--edit` do podpięcia pod skróty klawiszowe

## Wymagania
COSMIC (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`. Vault Obsidian (lokalny katalog).

## Budowanie
```sh
cd cosmic-widget-note
cargo build --release
cp target/release/cosmic-widget-note ~/.local/bin/
cp data/*.desktop ~/.config/autostart/   # opcjonalnie / optional
```

## Konfiguracja
`~/.config/cosmic-widget-note/config.toml`: `vault_path` (domyślnie `~/Obsidian`), `vault_name`, `note_file`, `archive_folder`; `remote_host`/`remote_vault_path` puste = zapis lokalny.

---

## English

A sticky note on the desktop backed by a single `.md` file in an Obsidian vault — the same content is visible in Obsidian. Uses the shared crate `../cosmic-widget-common` (build inside this repo).

### Features
- read-only view that doesn't steal focus; click → edit, autosave, `Esc` to finish
- archive the note to a folder, “open in Obsidian” button
- optional write-through over SSH to a server holding the canonical vault copy (avoids sync conflicts)
- `--new` / `--edit` for keyboard shortcuts

### Requirements
COSMIC desktop (cosmic-comp ≥ 1.0, layer-shell), Rust (rustup, edition 2024), `libxkbcommon-dev`. An Obsidian vault (local folder).

### Building
Same as above.

### Configuration
`~/.config/cosmic-widget-note/config.toml`: `vault_path` (default `~/Obsidian`), `vault_name`, `note_file`, `archive_folder`; empty `remote_host`/`remote_vault_path` = local writes.

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
