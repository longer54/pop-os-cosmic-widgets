# cosmic-widget-note — notatka na pulpicie

🇬🇧 [English version](README.md)

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
cp data/*.desktop ~/.config/autostart/   # opcjonalnie
```

## Konfiguracja
`~/.config/cosmic-widget-note/config.toml`: `vault_path` (domyślnie `~/Obsidian`), `vault_name`, `note_file`, `archive_folder`; `remote_host`/`remote_vault_path` puste = zapis lokalny.

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.
