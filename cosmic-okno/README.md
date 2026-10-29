# cosmic-okno — pokaż/schowaj okna skrótem

Małe narzędzie CLI do skrótów klawiszowych: przełącza okno aplikacji (pokaż/zminimalizuj/uruchom), chowa okna z listy zamiast je zamykać.

## Funkcje
- `cosmic-okno lista`
- `cosmic-okno przelacz <app_id> [polecenie]`
- `cosmic-okno zamknij-lub-schowaj <app_id>[,…]`
- `cosmic-okno schowaj-gdy-sie-pojawi <app_id> [sekundy]`

## Wymagania
COSMIC (protokoły `cosmic-toplevel-*`), Rust.

## Budowanie
```sh
cd cosmic-okno
cargo build --release
cp target/release/cosmic-okno ~/.local/bin/
```

## Konfiguracja
Brak pliku konfiguracyjnego — podpinasz polecenia pod skróty w Ustawieniach COSMIC.

---

## English

Small CLI for keyboard shortcuts: toggles an app window (show/minimize/launch), hides listed windows instead of closing them.

### Features
- `cosmic-okno lista` (list)
- `cosmic-okno przelacz <app_id> [command]` (toggle)
- `cosmic-okno zamknij-lub-schowaj <app_id>[,…]` (close-or-hide)
- `cosmic-okno schowaj-gdy-sie-pojawi <app_id> [secs]` (hide when it appears)

### Requirements
COSMIC (`cosmic-toplevel-*` protocols), Rust.

### Building
Same as above.

### Configuration
No config file — bind the commands to shortcuts in COSMIC Settings.

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
