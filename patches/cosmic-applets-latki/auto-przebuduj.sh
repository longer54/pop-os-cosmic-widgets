#!/bin/sh
# Pilnuje, zeby latane aplety panelu COSMIC w ~/.local/bin byly zbudowane pod
# te sama wersje pakietu cosmic-applets, ktora jest zainstalowana. Uruchamiany
# z timera systemd --user (cosmic-applets-latki.timer, patrz README),
# wzorowany na cosmic-comp-pamiec-okien/auto-przebuduj.sh.
#
# Po aktualizacji cosmic-applets przebudowuje w tle kazda latke, ktora jest na
# tym urzadzeniu (binarka w ~/.local/bin), odswieza panel i powiadamia.
# Stara latka dziala do konca buildu — aplety to nie kompozytor, rozjazd
# wersji najwyzej psuje popup, nie logowanie.
#
# Recznie: sh auto-przebuduj.sh   (nic nie robi, gdy wersja sie nie zmienila)
# Wymus:   rm ~/.local/state/cosmic-applets-latki.wersja && sh auto-przebuduj.sh

COODE="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$HOME/.local/bin"
STAN="$HOME/.local/state/cosmic-applets-latki.wersja"
LOG="$HOME/.local/state/cosmic-applets-latki.log"
BLOKADA="$HOME/.cache/cosmic-applets-latki.lock"

# binarka w ~/.local/bin  ->  skrypt budujacy jej latke
LATKI="cosmic-applet-battery:tryby-mocy-cpu
cosmic-applet-minimize:cosmic-applet-minimize-napisy
cosmic-applet-time:cosmic-applet-zegar-obsidian"

mkdir -p "$(dirname "$LOG")"
powiedz() { echo "$(date '+%F %T') $*" >> "$LOG"; }

WERSJA="$(pacman -Q cosmic-applets 2>/dev/null | awk '{print $2}')"
[ -n "$WERSJA" ] || WERSJA="$(dpkg-query -W -f='${Version}' cosmic-applets 2>/dev/null)"
[ -n "$WERSJA" ] || exit 0

# Ktore latki sa na tym urzadzeniu?
DO_ZROBIENIA=""
for para in $LATKI; do
    [ -x "$BIN/${para%%:*}" ] && DO_ZROBIENIA="$DO_ZROBIENIA $para"
done
[ -n "$DO_ZROBIENIA" ] || exit 0

# Pierwsze uruchomienie: zakladamy, ze obecne binarki sa swieze.
if [ ! -f "$STAN" ]; then
    echo "$WERSJA" > "$STAN"
    powiedz "start pilnowania, wersja $WERSJA"
    exit 0
fi

OSTATNIA="$(cat "$STAN")"
# "blad:<wersja>" = ta wersja juz raz sie nie zbudowala — nie mielimy co 15 min.
{ [ "$OSTATNIA" = "$WERSJA" ] || [ "$OSTATNIA" = "blad:$WERSJA" ]; } && exit 0

exec 9>"$BLOKADA"
flock -n 9 || { powiedz "build juz trwa - pomijam"; exit 0; }

powiedz "cosmic-applets $OSTATNIA -> $WERSJA, przebudowuje:$DO_ZROBIENIA"
NIEUDANE=""
for para in $DO_ZROBIENIA; do
    aplet="${para%%:*}"
    powiedz "buduje $aplet..."
    if sh "$COODE/${para#*:}/build-latka.sh" >> "$LOG" 2>&1; then
        powiedz "ok: $aplet"
    else
        powiedz "BLAD: $aplet - wracam do stockowego apletu"
        rm -f "$BIN/$aplet"
        NIEUDANE="$NIEUDANE $aplet"
    fi
done

pkill -x cosmic-panel   # panel wstaje sam i odpala nowe aplety

if [ -z "$NIEUDANE" ]; then
    echo "$WERSJA" > "$STAN"
    powiedz "gotowe"
    notify-send "Łatki panelu odbudowane" \
        "COSMIC się zaktualizował — kalendarzyk, napisy i tryby mocy zbudowane na nowo." 2>/dev/null
else
    echo "blad:$WERSJA" > "$STAN"
    powiedz "nieudane:$NIEUDANE"
    notify-send -u critical "Łatki panelu — błąd buildu" \
        "Nie przebudowało się:$NIEUDANE (wróciła wersja stockowa). Log: $LOG" 2>/dev/null
fi
