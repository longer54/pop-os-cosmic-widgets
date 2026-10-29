# cosmic-okno — pokaż/schowaj okna skrótem

🇬🇧 [English version](README.md)

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

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.
