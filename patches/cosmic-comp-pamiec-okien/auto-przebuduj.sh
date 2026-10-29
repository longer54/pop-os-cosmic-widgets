#!/bin/sh
# Pilnuje, zeby latany ~/.local/bin/cosmic-comp byl zawsze tej samej wersji co
# cosmic-comp z pakietu. Uruchamiany z timera systemd --user
# (cosmic-comp-latka.timer), instalowanego recznie (patrz README).
#
# Po aktualizacji systemu (updateall / pacman -Syu) wersje sie rozjezdzaja i
# przy NASTEPNYM logowaniu sesja graficzna moze nie wstac. Skrypt:
#   1. natychmiast usuwa nieaktualna latke  -> logowanie jest bezpieczne
#      zawsze, nawet gdy build nie zdazy (leci wtedy stockowy kompozytor,
#      tyle ze bez pamieci okien),
#   2. przebudowuje latke pod nowa wersje (ok. 11 min, nice 10, w tle),
#   3. wgrywa ja z powrotem i powiadamia, ze przy okazji warto sie przelogowac.
#
# Recznie: sh auto-przebuduj.sh   (nic nie robi, gdy wersje sa zgodne)

KATALOG="$(cd "$(dirname "$0")" && pwd)"
BIN="$HOME/.local/bin/cosmic-comp"
LOG="$HOME/.local/state/cosmic-comp-latka.log"
BLOKADA="$HOME/.cache/cosmic-comp-latka.lock"

mkdir -p "$(dirname "$LOG")"
powiedz() { echo "$(date '+%F %T') $*" >> "$LOG"; }

# Nie ma latki na tym urzadzeniu = nie ma czego pilnowac.
# Znacznik zostaje takze wtedy, gdy nieudany build skasowal binarke - dzieki
# temu timer probuje ponownie, zamiast na zawsze zostac na stockowym.
ZNACZNIK="$HOME/.local/state/cosmic-comp-latka.wlaczona"
[ -x "$BIN" ] || [ -f "$ZNACZNIK" ] || exit 0

SYSTEMOWY="$(command -v -- /usr/bin/cosmic-comp 2>/dev/null || echo /usr/bin/cosmic-comp)"
[ -x "$SYSTEMOWY" ] || exit 0

W_LATKA="$("$BIN" --version 2>/dev/null | head -1)"
W_SYSTEM="$("$SYSTEMOWY" --version 2>/dev/null | head -1)"
[ -n "$W_SYSTEM" ] || exit 0
[ -n "$W_LATKA" ] || W_LATKA="(brak latki)"
[ "$W_LATKA" != "$W_SYSTEM" ] || exit 0      # zgodne - cisza

# Po nieudanym buildzie nie mielimy co 15 min - ta sama wersja systemu
# najwyzej raz na 6 h (build trwa ~20 min i grzeje laptopa).
NIEUDANY="$HOME/.local/state/cosmic-comp-latka.nieudany"
if [ -f "$NIEUDANY" ] && [ "$(head -1 "$NIEUDANY")" = "$W_SYSTEM" ] \
   && [ -n "$(find "$NIEUDANY" -mmin -360 2>/dev/null)" ]; then
    exit 0
fi

powiedz "rozjazd: latka=[$W_LATKA] system=[$W_SYSTEM]"

# Jeden build na raz (timer moze zdazyc odpalic sie ponownie w trakcie).
exec 9>"$BLOKADA"
if command -v flock >/dev/null 2>&1; then
    flock -n 9 || { powiedz "build juz trwa - pomijam"; exit 0; }
fi

# KROK 1: zdejmij nieaktualna latke, zeby logowanie bylo bezpieczne.
rm -f "$BIN"
powiedz "usunieta nieaktualna latka (do czasu buildu dziala stockowy kompozytor)"

# KROK 2: przebuduj pod nowa wersje.
powiedz "buduje..."
if LATKI="${LATKI:-pamiec-okien.patch bez-ramki.patch}" sh "$KATALOG/build.sh" >> "$LOG" 2>&1; then
    rm -f "$NIEUDANY"
    powiedz "gotowe: $("$BIN" --version 2>/dev/null | head -1)"
    command -v notify-send >/dev/null 2>&1 && notify-send \
        "Pamiec okien odbudowana" \
        "cosmic-comp zaktualizowal sie, latka zbudowana na nowo. Zadziala po nastepnym zalogowaniu."
else
    echo "$W_SYSTEM" > "$NIEUDANY"
    powiedz "BUILD SIE NIE UDAL - zostaje stockowy kompozytor (bez pamieci okien)"
    command -v notify-send >/dev/null 2>&1 && notify-send -u critical \
        "Pamiec okien wylaczona" \
        "Nie udalo sie przebudowac latki po aktualizacji. Log: $LOG"
fi
