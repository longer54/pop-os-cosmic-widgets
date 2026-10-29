# cosmic-comp — własne łatki do kompozytora COSMIC

🇬🇧 [English version](README.md)

W katalogu żyją łatki do `cosmic-comp` i wspólny `build.sh`, który buduje
kompozytor ze **wszystkimi** plikami `*.patch` stąd i wgrywa binarkę do
`~/.local/bin` (bez sudo, stockowa binarka z `/usr/bin` zostaje nietknięta).

- `pamiec-okien.patch` — rozmiar i pozycja okien per aplikacja
- `bez-ramki.patch` (2026-10-04) — bez żółtej ramki aktywnego okna dla wybranych appek;
  lista app_id w `~/.config/cosmic-bez-ramki` (po jednym w linii), domyślnie pusta.
  Budowanie: `LATKI="pamiec-okien.patch bez-ramki.patch" sh build.sh`; cofnięcie:
  `mv ~/.local/bin/cosmic-comp.bak-przed-bez-ramki ~/.local/bin/cosmic-comp` + przelogowanie.

---

## Łatka 1: pamięć okien (rozmiar i pozycja per aplikacja)

Odpowiednik KDE-owego „Zapamiętaj rozmiar i pozycję" z reguł okien, którego
COSMIC nie ma. Łatka do kompozytora `cosmic-comp`.

## Problem

Telegram i Claude Desktop przy każdym uruchomieniu otwierały się mniejsze,
niż zostały ustawione. Inne aplikacje pozornie działały.

Przyczyna jest w samym cosmic-comp, `src/shell/layout/floating/mod.rs`,
funkcja `map_internal()` — nowe okno pływające jest przycinane do **2/3
ekranu**:

```rust
let mut width = output_geometry.size.w / 3 * 2;
```

Zmierzone na żywo (ekran 1920×1080): okno prosiło o 1700×950, dostało
**1280×698** — dokładnie 2/3 obszaru bez panelu. Dlatego „reszta działa":
mniejsze okna nigdy nie zawadzają o ten limit.

Drugi problem: cosmic-comp pamięta geometrię tylko przez czas życia okna
(`CosmicMapped::last_geometry`). Po zamknięciu aplikacji wiedza ginie.

Naprawy z zewnątrz **nie da się zrobić** — protokół `zcosmic_toplevel_manager_v1`
ma tylko `close`, `activate`, `set_maximized/minimized/fullscreen/sticky`
i `move_to_workspace`. Nie ma requestu na zmianę rozmiaru ani pozycji, więc
żaden demon w stylu KWin-owych reguł nie jest możliwy.

## Co robi łatka

- nowy moduł `src/utils/window_memory.rs` — trzyma ostatnią geometrię pod
  kluczem `app_id` w `~/.local/state/cosmic-comp/window-geometry.ron`
  (plik jest czytelny i można go ręcznie edytować/skasować);
- `FloatingLayout::unmap()` — zapisuje geometrię przy zamknięciu okna
  (pomija okna zmaksymalizowane i kafelkowane);
- `FloatingLayout::map_internal()` — przy pierwszym mapowaniu okna odtwarza
  zapamiętany rozmiar i pozycję, **z pominięciem limitu 2/3**, dociskając
  wynik do obszaru ekranu;
- `CosmicMapped` dostaje pole `saved_app_id` — `app_id` musi być zapamiętane
  przy mapowaniu, bo przy zamykaniu okna toplevel bywa już martwy i nie da
  się go odczytać z powierzchni.

Działa dla wszystkich aplikacji, niezależnie od tego, czy same zapisują
swoją geometrię.

## Instalacja

```
sh build.sh
```

Skrypt sam sprawdza `pacman -Q cosmic-comp`, klonuje źródła w tej samej
wersji (tag `epoch-X.Y.Z`), nakłada łatkę, buduje i wgrywa binarkę do
`~/.local/bin/cosmic-comp`. Potem wyloguj się i zaloguj.

**Bez sudo.** `cosmic-session` uruchamia `cosmic-comp` po nazwie z PATH,
a `~/.local/bin` jest w tym PATH pierwsze (sprawdzone w `/proc/<pid>/environ`
żywego kompozytora). Stockowa binarka w `/usr/bin` zostaje nietknięta.

## Cofnięcie

```
rm ~/.local/bin/cosmic-comp
```

i ponowne zalogowanie. Wraca oryginalny kompozytor z `/usr/bin`.

## Po aktualizacji cosmic-comp

Pacman podbija `/usr/bin/cosmic-comp`, ale **nie** rusza tego w `~/.local/bin`.
Wersje kompozytora i sesji muszą się zgadzać (`cosmic-session` sypie wtedy
„Unknown session socket message, are you using incompatible cosmic-session
and cosmic-comp versions?"), więc po każdej aktualizacji trzeba puścić
`sh build.sh` jeszcze raz. Gdyby coś poszło nie tak — `rm ~/.local/bin/cosmic-comp`
i jest jak było.

## Inne urządzenia

Build i `cp` binarki trzeba powtórzyć na każdym komputerze — tak samo jak
przy widgetach COSMIC.


---

## Łatka 2: nocny filtr — NIE dołączona

Wcześniejsza łatka „nocny filtr” (gamma wewnątrz kompozytora) miała błąd
(zły rozmiar tablicy gamma → zamglony ekran do restartu), dlatego nie ma jej
w tym repo. Nocny filtr obsługuje teraz aplet `../../cosmic-applet-nocny-filtr/`
przez zewnętrzny demon `drm-colortemp`.

## Automat: przebudowa po aktualizacji systemu (2026-09-16)

Latana binarka musi byc tej samej wersji co `cosmic-session` z pakietu.
Zeby nie trzeba bylo o tym pamietac przy kazdym `updateall` / `pacman -Syu`,
timer `systemd --user` sprawdza to co 15 minut:

- `auto-przebuduj.sh` — porownuje `--version` latki i `/usr/bin/cosmic-comp`;
  przy rozjezdzie **najpierw usuwa nieaktualna latke** (dzieki temu logowanie
  jest bezpieczne nawet gdy build nie zdazy — wstaje stockowy kompozytor,
  tyle ze bez pamieci okien), potem przebudowuje i wgrywa nowa, na koniec
  `notify-send`. Log: `~/.local/state/cosmic-comp-latka.log`.
- Unity: `~/.config/systemd/user/cosmic-comp-latka.{timer,service}` —
  trzeba je utworzyc samodzielnie (service uruchamia `auto-przebuduj.sh`,
  timer co 15 min) na kazdym komputerze, ktory ma `~/.local/bin/cosmic-comp`.
- Podglad: `systemctl --user list-timers cosmic-comp-latka.timer`
  Wylaczenie: `systemctl --user disable --now cosmic-comp-latka.timer`

Szczelina, ktora zostaje: aktualizacja systemu i **natychmiastowy** restart w
tym samym kwadransie. Wtedy ratunek po omacku: Ctrl+Alt+F3,
`rm ~/.local/bin/cosmic-comp`, zaloguj sie normalnie.
