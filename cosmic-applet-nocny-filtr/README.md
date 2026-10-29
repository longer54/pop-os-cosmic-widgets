# cosmic-applet-nocny-filtr — night light in the panel

🇵🇱 [Polska wersja](README.pl.md)

A panel applet with an on/off switch, an hourly schedule toggle and a colour temperature slider (2500–6500 K). It doesn't touch the display itself — it drives the `drm-colortemp` daemon through a root helper with a narrow sudo rule.

## Features
- the change is applied when you release the slider (each change = a brief flicker, because COSMIC only releases the display on a VT switch)
- the sudo rule enumerates only the allowed arguments; the helper validates them again

## Requirements
COSMIC, the `drm-colortemp` daemon (AUR / your own build), Rust, sudo for the one-time helper installation.

## Building
```sh
cd cosmic-applet-nocny-filtr
sh install.sh            # applet (no root)
sudo sh install-root.sh  # helper + sudo rule
```

## Configuration
Schedule in `/etc/default/drm-colortemp.conf` (`SUNSET_HOUR`/`SUNRISE_HOUR`). Uninstall: `sudo rm /usr/local/bin/nocny-filtr-apply /etc/sudoers.d/nocny-filtr`.

---

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
