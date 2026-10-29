#!/bin/sh
# Instaluje pomocnika roota dla apletu nocnego filtra + regule sudo.
# Uruchamiac przez sudo:   sudo sh install-root.sh
#
# Regula sudo NIE jest otwarta: wylicza dokladne polecenia, ktore aplet moze
# uruchomic (temperatury co 100 K oraz "off"/"auto"). Nic innego nie przejdzie.

set -eu

KATALOG="$(cd "$(dirname "$0")" && pwd)"
BIN=/usr/local/bin/nocny-filtr-apply
SUDOERS=/etc/sudoers.d/nocny-filtr
UZYTKOWNIK="${SUDO_USER:-$(id -un)}"

[ "$(id -u)" -eq 0 ] || { echo "uruchom przez sudo"; exit 1; }

install -Dm755 "$KATALOG/helper/nocny-filtr-apply" "$BIN"
echo ">> wgrany pomocnik: $BIN"

# Lista dozwolonych polecen: 2500..6500 co 100, plus off i auto.
{
    printf '# Aplet nocnego filtra: tylko te dokladne polecenia, zero ogolnego roota.\n'
    printf '%s ALL=(root) NOPASSWD: %s off, %s auto' "$UZYTKOWNIK" "$BIN" "$BIN"
    K=2500
    while [ "$K" -le 6500 ]; do
        printf ', %s %s' "$BIN" "$K"
        K=$((K + 100))
    done
    printf '\n'
} > "$SUDOERS.tmp"

chmod 440 "$SUDOERS.tmp"
if visudo -cf "$SUDOERS.tmp" >/dev/null; then
    mv "$SUDOERS.tmp" "$SUDOERS"
    echo ">> regula sudo zainstalowana dla uzytkownika: $UZYTKOWNIK"
else
    rm -f "$SUDOERS.tmp"
    echo "!! regula sudo odrzucona przez visudo — nic nie zmieniono" >&2
    exit 1
fi

echo ">> gotowe. Odinstalowanie:  sudo rm $BIN $SUDOERS"
