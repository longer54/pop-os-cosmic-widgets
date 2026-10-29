#!/bin/sh
# Buduje aplet minimalizacji COSMIC z latka "napisy zamiast miniatur" i wgrywa go do ~/.local/bin.
#
# Bez sudo: panel odpala "cosmic-applet-minimize" po nazwie z PATH, a
# ~/.local/bin jest w tym PATH pierwsze. Stockowa binarka w /usr/bin zostaje
# nietknieta, wiec aktualizacje systemu niczego nie psuja — ale po aktualizacji
# pakietu cosmic-applets trzeba ten skrypt uruchomic ponownie, inaczej w panelu
# siedzi stara (zalatana) wersja przy nowym reszcie systemu.
#
# Cofniecie zmiany:  rm ~/.local/bin/cosmic-applet-minimize  (i pkill cosmic-panel)

set -eu

KATALOG="$(cd "$(dirname "$0")" && pwd)"
ZRODLA="${ZRODLA:-$HOME/.cache/cosmic-applets-custom/src}"
TARGET="${CARGO_TARGET_DIR:-$HOME/.cache/cargo-target/cosmic-applets-custom}"
LATKA="$KATALOG/latka/napisy-zamiast-miniatur.patch"

# Wersja MUSI zgadzac sie z zainstalowanym pakietem — aplet rozmawia z panelem
# i z cosmic-config, wiec rozjazd wersji potrafi skonczyc sie pustym popupem.
WERSJA="$(pacman -Q cosmic-applets 2>/dev/null | awk '{print $2}' | sed 's/^[0-9]*://; s/-.*//')"
[ -n "$WERSJA" ] || WERSJA="$(dpkg-query -W -f='${Version}' cosmic-applets 2>/dev/null | sed 's/^[0-9]*://; s/[~-].*//')"
[ -n "$WERSJA" ] || { echo "!! nie wykryto wersji pakietu cosmic-applets" >&2; exit 1; }
TAG="epoch-$WERSJA"
echo ">> wersja systemowa: $WERSJA  (tag $TAG)"

if [ -d "$ZRODLA/.git" ]; then
    echo ">> odswiezam zrodla"
    git -C "$ZRODLA" fetch --depth 1 origin "refs/tags/$TAG:refs/tags/$TAG" 2>/dev/null || true
    git -C "$ZRODLA" checkout -f "$TAG" 2>/dev/null || {
        echo "!! brak tagu $TAG w repo — czy pakiet wyprzedza upstream?" >&2; exit 1; }
    git -C "$ZRODLA" clean -fd >/dev/null
else
    echo ">> klonuje zrodla ($TAG)"
    mkdir -p "$(dirname "$ZRODLA")"
    git clone --depth 1 --branch "$TAG" https://github.com/pop-os/cosmic-applets "$ZRODLA"
fi

echo ">> nakladam latke"
git -C "$ZRODLA" apply --whitespace=nowarn "$LATKA"

echo ">> buduje (kilka minut)"
cd "$ZRODLA"
CARGO_TARGET_DIR="$TARGET" nice -n 10 cargo build --release -p cosmic-applet-minimize

mkdir -p "$HOME/.local/bin"
cp "$TARGET/release/cosmic-applet-minimize" "$HOME/.local/bin/cosmic-applet-minimize.new"
mv -f "$HOME/.local/bin/cosmic-applet-minimize.new" "$HOME/.local/bin/cosmic-applet-minimize"
echo ">> wgrane: ~/.local/bin/cosmic-applet-minimize"
echo ">> zeby zobaczyc zmiane:  pkill cosmic-panel   (panel wstaje sam)"
