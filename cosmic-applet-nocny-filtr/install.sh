#!/bin/sh
# Buduje aplet nocnego filtra, wgrywa go do ~/.local/bin, instaluje plik
# .desktop i dopina aplet do prawego skrzydla panelu COSMIC.
#
# Bezpieczny do wielokrotnego uruchamiania.

set -e

KATALOG="$(cd "$(dirname "$0")" && pwd)"
TARGET="${CARGO_TARGET_DIR:-$HOME/.cache/cargo-target/cosmic-applet-nocny-filtr}"
APLET_ID="io.github.longer54.CosmicAppletNocnyFiltr"
WINGS="$HOME/.config/cosmic/com.system76.CosmicPanel.Panel/v1/plugins_wings"

echo ">> buduje aplet"
cd "$KATALOG"
CARGO_TARGET_DIR="$TARGET" nice -n 10 cargo build --release

mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"
cp "$TARGET/release/cosmic-applet-nocny-filtr" "$HOME/.local/bin/cosmic-applet-nocny-filtr"

cat > "$HOME/.local/share/applications/$APLET_ID.desktop" <<'DESKTOP'
[Desktop Entry]
Name=Nocny filtr
Name[pl]=Nocny filtr
Comment=Filtr swiatla niebieskiego - wlacznik i temperatura barwowa
Comment[pl]=Filtr światła niebieskiego — włącznik i temperatura barwowa
Type=Application
Exec=cosmic-applet-nocny-filtr
Terminal=false
Categories=COSMIC;
Keywords=COSMIC;Applet;Night;Light;Nocny;Filtr;
Icon=weather-clear-night-symbolic
StartupNotify=true
NoDisplay=true
X-CosmicApplet=true
X-CosmicHoverPopup=Start
X-OverflowPriority=8
DESKTOP

echo ">> wgrany aplet i plik .desktop"

# Dopiecie do prawego skrzydla panelu (druga lista w plugins_wings).
if [ -f "$WINGS" ] && ! grep -q "$APLET_ID" "$WINGS"; then
    cp "$WINGS" "$WINGS.bak-przed-nocnym-filtrem"
    python3 - "$WINGS" "$APLET_ID" <<'PY'
import sys
sciezka, aplet_id = sys.argv[1], sys.argv[2]
tekst = open(sciezka).read()
kotwica = '    "com.system76.CosmicAppletA11y",\n'
if kotwica in tekst:
    tekst = tekst.replace(kotwica, kotwica + f'    "{aplet_id}",\n', 1)
else:  # awaryjnie: na poczatek prawego skrzydla
    tekst = tekst.replace('], [\n', '], [\n' + f'    "{aplet_id}",\n', 1)
open(sciezka, "w").write(tekst)
PY
    echo ">> aplet dopiety do panelu (kopia: $WINGS.bak-przed-nocnym-filtrem)"
else
    echo ">> aplet juz jest w panelu (albo brak configu panelu)"
fi

echo ">> gotowe. Jesli ikonka nie pojawila sie od razu:"
echo "   pkill cosmic-panel   (panel wstaje sam)"
