# cosmic-applet-nocny-filtr — nocny filtr w panelu

🇬🇧 [English version](README.md)

Aplet panelu z włącznikiem, automatem godzinowym i suwakiem temperatury (2500–6500 K). Sam nie dotyka ekranu — steruje demonem `drm-colortemp` przez pomocnika roota z wąską regułą sudo.

## Funkcje
- zmiana stosowana po puszczeniu suwaka (każda zmiana = krótkie mignięcie, bo COSMIC oddaje ekran tylko przy przełączeniu VT)
- reguła sudo wylicza tylko dozwolone argumenty; pomocnik waliduje je ponownie

## Wymagania
COSMIC, demon `drm-colortemp` (AUR / własny build), Rust, sudo do jednorazowej instalacji pomocnika.

## Budowanie
```sh
cd cosmic-applet-nocny-filtr
sh install.sh            # aplet (bez roota)
sudo sh install-root.sh  # pomocnik + reguła sudo
```

## Konfiguracja
Harmonogram w `/etc/default/drm-colortemp.conf` (`SUNSET_HOUR`/`SUNRISE_HOUR`). Odinstalowanie: `sudo rm /usr/local/bin/nocny-filtr-apply /etc/sudoers.d/nocny-filtr`.

---

**Zastrzeżenie:** Prywatny projekt hobbystyczny, tworzony do nauki i własnego użytku. Nie jest powiązany z System76 ani projektem COSMIC. Korzysta z otwartych narzędzi i publicznych źródeł danych. Oprogramowanie udostępniane „tak jak jest”, bez gwarancji.
