#!/bin/sh
# Buduje cosmic-comp z latkami z tego katalogu (*.patch) i wgrywa go do
# ~/.local/bin. Latki: "pamiec okien" + "nocny filtr" (night light).
#
# Bez sudo: cosmic-session odpala "cosmic-comp" po nazwie z PATH, a ~/.local/bin
# jest w tym PATH pierwsze. Stockowa binarka w /usr/bin zostaje nietknieta,
# wiec aktualizacje systemu niczego nie psuja.
#
# Cofniecie zmiany:  rm ~/.local/bin/cosmic-comp  (i wyloguj/zaloguj)
#
# Wybor latek:   LATKI="pamiec-okien.patch" sh build.sh
#   (domyslnie wszystkie *.patch z tego katalogu; na Pop!_OS/COSMIC 1.0
#    nocny filtr moze sie nie nakladac - tam buduj sama pamiec okien)

set -e

KATALOG="$(cd "$(dirname "$0")" && pwd)"
ZRODLA="${ZRODLA:-$HOME/.cache/cosmic-comp-custom/src}"
TARGET="${CARGO_TARGET_DIR:-$HOME/.cache/cargo-target/cosmic-comp-custom}"

if [ -n "$LATKI" ]; then
    LISTA_LATEK=""
    for l in $LATKI; do LISTA_LATEK="$LISTA_LATEK $KATALOG/$l"; done
else
    LISTA_LATEK="$KATALOG"/*.patch
fi

# --- Ktora wersje zrodel wziac -------------------------------------------
# Wersja MUSI sie zgadzac z zainstalowana - cosmic-session i cosmic-comp
# rozmawiaja prywatnym protokolem i wersje musza byc te same.
if command -v pacman >/dev/null 2>&1; then
    # CachyOS/Arch: pakiety sa tagowane epoch-X.Y.Z
    WERSJA="$(pacman -Q cosmic-comp | awk '{print $2}' | sed 's/^[0-9]*://; s/-[0-9.]*$//')"
    REF="epoch-$WERSJA"
else
    # Pop!_OS/Debian: wersja pakietu nie mowi nic o tagu, ale binarka zna
    # swoj commit:  "cosmic-comp 1.0.0 (git commit f8344126...)"
    WERSJA="$(cosmic-comp --version 2>/dev/null | head -1)"
    REF="$(printf '%s' "$WERSJA" | sed -n 's/.*git commit \([0-9a-f]\{7,40\}\).*/\1/p')"
    [ -n "$REF" ] || { echo "!! nie umiem ustalic commita z: $WERSJA"; exit 1; }
fi
echo ">> zainstalowana wersja: $WERSJA  (zrodla: $REF)"

# --- Naglowki dev bez roota (Pop!_OS, gdy nie ma sudo) --------------------
# ~/.cache/cosmic-builddeps/root to rozpakowane pakiety -dev (dpkg -x), z .pc
# przestawionymi na ten prefix i symlinkami .so na systemowe biblioteki.
DEPS="$HOME/.cache/cosmic-builddeps/root"
if [ -d "$DEPS" ]; then
    echo ">> uzywam lokalnych naglowkow dev z $DEPS"
    L="$DEPS/usr/lib/x86_64-linux-gnu"
    PKG_CONFIG_PATH="$L/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
    PATH="$DEPS/usr/bin:$PATH"
    LD_LIBRARY_PATH="$L${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    CPATH="$DEPS/usr/include${CPATH:+:$CPATH}"
    LIBRARY_PATH="$L${LIBRARY_PATH:+:$LIBRARY_PATH}"
    export PKG_CONFIG_PATH PATH LD_LIBRARY_PATH CPATH LIBRARY_PATH
fi

if [ ! -d "$ZRODLA/.git" ]; then
    echo ">> klonuje zrodla cosmic-comp"
    mkdir -p "$(dirname "$ZRODLA")"
    git clone --filter=blob:none https://github.com/pop-os/cosmic-comp.git "$ZRODLA"
fi

cd "$ZRODLA"
git fetch --tags origin
# NAJPIERW zdejmij poprzednia latke, potem przelacz wersje - odwrotnie git
# odmawia ("Wybranie galezi nadpisaloby lokalne zmiany").
git checkout -q -- .          # skasuj poprzednia latke
git clean -qfd src/ 2>/dev/null || true     # nowe pliki z poprzedniej latki
git checkout -q -f "$REF"

for latka in $LISTA_LATEK; do
    echo ">> nakladam latke: $(basename "$latka")"
    git apply "$latka"
done

echo ">> buduje (to potrwa kilkanascie minut)"
CARGO_TARGET_DIR="$TARGET" nice -n 10 cargo build --release

mkdir -p "$HOME/.local/bin"
# przez rename: cp na dzialajaca binarke konczy sie "Plik wykonywalny zajety"
cp "$TARGET/release/cosmic-comp" "$HOME/.local/bin/cosmic-comp.new"
mv -f "$HOME/.local/bin/cosmic-comp.new" "$HOME/.local/bin/cosmic-comp"
mkdir -p "$HOME/.local/state"; touch "$HOME/.local/state/cosmic-comp-latka.wlaczona"
echo ">> gotowe: $HOME/.local/bin/cosmic-comp"
echo ">> wyloguj sie i zaloguj ponownie, zeby wystartowal nowy kompozytor"

# CLI do sterowania nocnym filtrem (tylko gdy latka filtra byla w buildzie)
case "$LISTA_LATEK" in
  *nocny-filtr*)
    if [ -f "$KATALOG/nocny-filtr" ]; then
        cp "$KATALOG/nocny-filtr" "$HOME/.local/bin/nocny-filtr"
        chmod +x "$HOME/.local/bin/nocny-filtr"
        echo ">> wgrany: $HOME/.local/bin/nocny-filtr  (sterowanie filtrem)"
    fi
    ;;
esac
