# cosmic-applet-nocny-filtr — nocny filtr w panelu

Aplet panelu z włącznikiem, automatem godzinowym i suwakiem temperatury (2500–6500 K). Sam nie dotyka ekranu — steruje demonem `drm-colortemp` przez pomocnika roota z wąską regułą sudo.

## Funkcje
- zmiana stosowana po puszczeniu suwaka (każda zmiana = krótkie mignięcie, bo COSMIC oddaje ekran tylko przy przełączeniu VT)
- reguła sudo wylicza tylko dozwolone argumenty; pomocnik waliduje je ponownie

## Wymagania
COSMIC, demon `drm-colortemp` (AUR / własny build), Rust, sudo do jednorazowej instalacji pomocnika.

## Budowanie
```sh
cd cosmic-applet-nocny-filtr
sh install.sh            # aplet (bez roota) / applet (no root)
sudo sh install-root.sh  # pomocnik + reguła sudo / helper + sudo rule
```

## Konfiguracja
Harmonogram w `/etc/default/drm-colortemp.conf` (`SUNSET_HOUR`/`SUNRISE_HOUR`). Odinstalowanie: `sudo rm /usr/local/bin/nocny-filtr-apply /etc/sudoers.d/nocny-filtr`.

---

## English

Panel applet with an on/off switch, schedule toggle and a temperature slider (2500–6500 K). It doesn't touch the screen itself — it drives the `drm-colortemp` daemon through a root helper with a narrow sudo rule.

### Features
- applied on slider release (each change = a short flicker, COSMIC only releases the display on a VT switch)
- the sudo rule lists only allowed arguments; the helper re-validates them

### Requirements
COSMIC, the `drm-colortemp` daemon (AUR / own build), Rust, sudo for one-time helper install.

### Building
Same as above.

### Configuration
Schedule in `/etc/default/drm-colortemp.conf` (`SUNSET_HOUR`/`SUNRISE_HOUR`). Uninstall: `sudo rm /usr/local/bin/nocny-filtr-apply /etc/sudoers.d/nocny-filtr`.

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
