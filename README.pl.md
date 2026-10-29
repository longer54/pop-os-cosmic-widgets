# Widgety pulpitu COSMIC dla Pop!_OS

🇬🇧 [English version](README.md)

Lekkie widgety pulpitowe, aplety panelu i drobne narzędzia dla [pulpitu COSMIC](https://github.com/pop-os/cosmic-epoch) (Pop!_OS), napisane w Ruście z użyciem [libcosmic](https://github.com/pop-os/libcosmic)/iced. Każdy widget to osobny program, który przypina się do rogu ekranu przez waylandowy protokół layer-shell i odświeża treść według zegara — bez potrzeby API apletów panelu.

## Widgety

| Widget | Co pokazuje | Źródło danych |
|---|---|---|
| [`cosmic-widget-panel`](cosmic-widget-panel/) | Stan Twoich własnych usług/serwerów | Twój własny endpoint HTTP zwracający JSON |
| [`cosmic-calendar-widget`](cosmic-calendar-widget/) | Kalendarz notatek dziennych Obsidiana (klik w dzień otwiera/tworzy jego notatkę) | Lokalne pliki vaulta Obsidiana |
| [`cosmic-widget-agenda`](cosmic-widget-agenda/) | Najbliższe wydarzenia z Kalendarza Google | Prywatny kanał ICS (bez OAuth) |
| [`cosmic-widget-tasks`](cosmic-widget-tasks/) | Zadania zaległe / na dziś (składnia wtyczki [Tasks](https://github.com/obsidian-tasks-group/obsidian-tasks) do Obsidiana) | Lokalne pliki vaulta Obsidiana |
| [`cosmic-widget-weather`](cosmic-widget-weather/) | Bieżąca pogoda (domyślnie Lublin) + krótka prognoza | Publiczny kanał stacji IMGW + Open-Meteo |
| [`cosmic-widget-markets`](cosmic-widget-markets/) | Ceny aktywów + wykresy (krypto, akcje, surowce, ETF) | CoinGecko + Yahoo Finance (publiczne) |
| [`cosmic-widget-note`](cosmic-widget-note/) | Karteczka na pulpicie, zapisywana jako plik `.md` | Lokalny vault Obsidiana |
| [`cosmic-widget-drawer`](cosmic-widget-drawer/) | Wysuwane szuflady: treść widgetów (prawa) i drzewo plików vaulta (lewa) | Źródła danych pozostałych widgetów |

### Aplety panelu i narzędzia

| Projekt | Co robi |
|---|---|
| [`cosmic-quicklook`](cosmic-quicklook/) | Quick Look jak w macOS: podgląd zaznaczonego pliku (obraz, PDF, tekst, wideo) |
| [`cosmic-okno`](cosmic-okno/) | CLI do pokazywania/chowania/przełączania okien skrótami klawiszowymi |
| [`cosmic-tray-reset`](cosmic-tray-reset/) | Ikona w zasobniku restartująca widgety pulpitu |
| [`cosmic-applet-pobor`](cosmic-applet-pobor/) | Aplet panelu: bieżący pobór prądu laptopa + średnie godzinowe |
| [`cosmic-applet-nocny-filtr`](cosmic-applet-nocny-filtr/) | Aplet panelu: suwak nocnego filtra (steruje `drm-colortemp`) |
| [`cosmic-widget-common`](cosmic-widget-common/) | Wspólny crate: przeciąganie myszką, auto-układanie, szkło/rozmycie, zapis do vaulta |
| [`patches/`](patches/) | Łatki do `cosmic-comp` i apletów panelu COSMIC (pamięć rozmiaru okien, napisy zminimalizowanych okien, zegar z Obsidianem) |

Każdy widget ma własne README ze szczegółami konfiguracji i instalacji.

## Wspólna architektura

- **Powierzchnia layer-shell** (`Layer::Bottom`: nad tapetą, pod oknami, widoczna na każdym obszarze roboczym), zakotwiczona w rogu ekranu i przesunięta o konfigurowalny margines — pozycja jest powtarzalna między restartami.
- **Jawny rozmiar + zmiana rozmiaru przy odświeżeniu.** Swobodne powierzchnie layer-shell nie dopasowują się same do treści tak jak aplety panelu; każdy widget wylicza szacowaną wysokość z bieżącej treści i wywołuje `set_size` po każdym odświeżeniu danych.
- **Karta w stylu szkła** — ciemna, półprzezroczysta, zaokrąglona karta, spójna we wszystkich widgetach.
- **Plik konfiguracyjny** w `~/.config/<nazwa-widgetu>/config.toml`, tworzony automatycznie z wartościami zastępczymi przy pierwszym uruchomieniu — edytujesz go, potem restartujesz widget.
- **Autostart** przez plik `.desktop`, który kopiujesz do `~/.config/autostart/`.

## Wymagania

- Pop!_OS z pulpitem COSMIC (`cosmic-comp` ≥ 1.0 — to on udostępnia `zwlr_layer_shell_v1` zwykłym aplikacjom, nie tylko apletom).
- Rust przez [rustup](https://rustup.rs) — edition 2024, `rust-version` 1.85.
- `libxkbcommon-dev` (pakiet systemowy). Jeśli nie możesz instalować pakietów systemowych (brak sudo), zob. Rozwiązywanie problemów niżej.

## Budowanie i instalacja widgetu

```sh
cd cosmic-widget-panel   # albo dowolny inny widget
cargo build --release
cp target/release/cosmic-widget-panel ~/.local/bin/
cp data/*.desktop ~/.config/autostart/    # opcjonalnie: uruchamianie automatycznie po zalogowaniu
~/.local/bin/cosmic-widget-panel &        # pierwsze uruchomienie zapisuje domyślny config.toml
```

Zmień świeżo utworzony `~/.config/<nazwa-widgetu>/config.toml` (każdy widget wymaga uzupełnienia co najmniej jednej wartości — klucza API, adresu ICS, ścieżki vaulta itp.), a potem zrestartuj widget, żeby zmiana zadziałała.

## Kilka widgetów w tym samym rogu

Wszystkie widgety mają te same pola konfiguracji `anchor` (`top-left` / `top-right` / `bottom-left` / `bottom-right`) oraz `margin_x` / `margin_y`. Żeby ułożyć dwa widgety w tym samym rogu, daj drugiemu `margin_y` (albo `margin_x`, jeśli stoją obok siebie) na tyle duży, żeby ominąć wysokość (lub szerokość) pierwszego plus mały odstęp. Domyślny config każdego widgetu w tym repo opisuje rachunek, jaki zakłada dla swojej domyślnej pozycji — traktuj go jako punkt wyjścia, nie sztywny układ.

## Rozwiązywanie problemów

- **Brak `libxkbcommon-dev` i brak sudo**: zrób dowiązanie `~/.local/lib/libxkbcommon.so -> libxkbcommon.so.0` (tam, gdzie biblioteka faktycznie leży w Twoim systemie) i napisz pasujący plik `.pc` w `~/.local/lib/pkgconfig`, a potem `export PKG_CONFIG_PATH=$HOME/.local/lib/pkgconfig` przed `cargo build`.
- **Widget nie dopasowuje rozmiaru do treści / treść jest ucięta na dole**: szacowanie wysokości w `app.rs` danego widgetu zaniża rzeczywistą wysokość wiersza. Zwiększ stałą pikseli na wiersz w `estimated_height()` — lepiej dać za dużo (trochę pustego miejsca) niż uciąć treść, bo powierzchnie layer-shell się nie przewijają.
- **Program zbudowany na jednym komputerze nie działa na innym**: `target/` zależy od architektury i toolchainu i nie służy do kopiowania między maszynami — przebuduj `cargo build --release` na każdej z nich.

## Licencja

GPL-3.0-or-later — zob. [LICENSE](LICENSE).

## Zastrzeżenie

Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji. Teksty interfejsu programów są po polsku.
