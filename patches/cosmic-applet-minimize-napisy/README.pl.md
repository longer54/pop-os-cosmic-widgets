# Aplet „zminimalizowane” — napisy zamiast miniatur (COSMIC)

🇬🇧 [English version](README.md)

Stockowy aplet `cosmic-applet-minimize` pokazuje w panelu miniaturkę okna.
Łatka zamienia ją na **napis z nazwą aplikacji** (np. „Gmail”) w stylu
przycisków „Obszary robocze” / „Aplikacje”. Klik przywraca okno, dymek
z nazwą zostaje. Na pionowym panelu zostają miniaturki. Nazwy dłuższe
niż 20 znaków kończą się „…”.

- Łatka: `latka/napisy-zamiast-miniatur.patch` (na tag `epoch-<wersja pakietu>`).
- Budowa i wgranie: `sh build-latka.sh` → `~/.local/bin/cosmic-applet-minimize`
  (PATH przykrywa /usr/bin, bez sudo). Potem `pkill -x cosmic-panel`.
- **Po `pacman -Syu` ruszającym `cosmic-applets` przebudować** (jak pozostałe łatki apletów). Uwaga: oba skrypty używają tego samego drzewa źródeł
  i robią `checkout -f`, więc każdy buduje tylko swój aplet — to OK.
- Cofnięcie: `rm ~/.local/bin/cosmic-applet-minimize && pkill -x cosmic-panel`.
