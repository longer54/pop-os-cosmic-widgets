# cosmic-tray-reset — restart widgetów z zasobnika

🇬🇧 [English version](README.md)

Ikonka w zasobniku (StatusNotifierItem). Kliknięcie restartuje usługi autostartu widgetów z tego repo, gdy stracą łączność z portalem i przestaną reagować.

## Funkcje
- zabija zdublowane instancje (`pkill`), potem restartuje jednostki `systemctl --user` po kolei

## Wymagania
Panel COSMIC z apletem zasobnika, Rust.

## Budowanie
```sh
cd cosmic-tray-reset
cargo build --release
cp target/release/cosmic-tray-reset ~/.local/bin/
```

## Konfiguracja
Lista jednostek: stała `UNITS` w `src/main.rs`.

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.
