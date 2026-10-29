#!/bin/sh
# Pop!_OS: naglowki dev do buildu cosmic-comp BEZ sudo.
#
# Build-Depends pakietu cosmic-comp to m.in. libinput-dev, libudev-dev,
# libseat-dev, libgbm-dev, libdisplay-info-dev, cmake - normalnie
# "sudo apt install ...". Bez sudo (np. w sandboksie z no_new_privs), wiec
# zamiast instalowac systemowo pobieramy .deb i rozpakowujemy do ~/.cache:
#   - naglowki ida lokalnie,
#   - biblioteki .so zostaja SYSTEMOWE (symlinki .so wskazuja na /usr/lib),
#   - pliki .pc dostaja lokalny prefix.
# Nic w systemie sie nie zmienia; cofniecie = rm -rf ~/.cache/cosmic-builddeps
#
# Jesli masz sudo, prosciej:
#   sudo apt install cmake libgbm-dev libinput-dev libseat-dev libudev-dev libdisplay-info-dev

set -e
DEPS="$HOME/.cache/cosmic-builddeps"
L="$DEPS/root/usr/lib/x86_64-linux-gnu"
mkdir -p "$DEPS/debs" "$DEPS/root"

cd "$DEPS/debs"
echo ">> pobieram pakiety -dev (bez instalacji)"
apt-get download cmake cmake-data libuv1t64 libjsoncpp25 librhash0 \
    libdisplay-info-dev libgbm-dev libudev-dev libgudev-1.0-dev gir1.2-gudev-1.0 \
    libinput-dev libmtdev-dev libseat-dev libevdev-dev libwacom-dev

echo ">> rozpakowuje do $DEPS/root"
for d in *.deb; do dpkg -x "$d" "$DEPS/root"; done

echo ">> przestawiam zerwane symlinki .so na systemowe biblioteki"
cd "$L"
for s in *.so; do
    [ -L "$s" ] && [ ! -e "$s" ] || continue
    tgt="$(readlink "$s")"
    if [ -e "/usr/lib/x86_64-linux-gnu/$tgt" ]; then
        ln -sf "/usr/lib/x86_64-linux-gnu/$tgt" "$s"
    else
        echo "!! brak systemowego $tgt (dla $s) - build moze nie zlinkowac"
    fi
done

echo ">> ustawiam prefix w plikach .pc"
cd "$L/pkgconfig"
sed -i "s|^prefix=/usr\$|prefix=$DEPS/root/usr|" *.pc

echo ">> gotowe. Teraz: LATKI=\"pamiec-okien.patch\" sh build.sh"
