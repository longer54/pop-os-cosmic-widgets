# Aplet zegara — kalendarzyk Obsidiana (COSMIC)

Klik w datę/godzinę w górnym panelu otwiera kalendarzyk, który działa jak
wtyczka Calendar w Obsidianie (i jak widget `cosmic-applet-calendar`):

- **klik w dzień** → tworzy `Kalendarz/RRRR-MM-DD.md` z szablonu
  (jeśli nie ma) i ją otwiera; popup się chowa. Działają też dni
  sąsiednich miesięcy (przygaszone).
- **kropki pod dniem** = długość notatki (1 kropka / 250 słów, max 5;
  istniejąca pusta notatka = 1 kropka).
- dziś podświetlone akcentem.

Ustawienia (vault, folder, szablon, kropki, **Obsidian vs plik .md**) czyta
z tego samego pliku co widget: `~/.config/cosmic-calendar-widget/config.toml`
— klucz `open_in_obsidian` (przełącznik „Obsidian” w widgecie) decyduje też tutaj.
Plik czytany przy każdym otwarciu popupu, restart niepotrzebny.

- Łatka: `latka/kalendarzyk-obsidiana.patch` (na tag `epoch-<wersja pakietu>`).
- Budowa i wgranie: `sh build-latka.sh` → `~/.local/bin/cosmic-applet-time`
  (PATH przykrywa /usr/bin, bez sudo). Potem `pkill -x cosmic-panel`.
- **Po `pacman -Syu` ruszającym `cosmic-applets` przebudować** (jak łatki
  minimalizacji i baterii).
- Cofnięcie: `rm ~/.local/bin/cosmic-applet-time && pkill -x cosmic-panel`.
