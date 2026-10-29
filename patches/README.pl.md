# patches — łatki do komponentów COSMIC

🇬🇧 [English version](README.md)

Łatki (`.patch`) na upstreamowe repozytoria System76 + skrypty, które klonują
upstream, nakładają łatki, budują i instalują binarkę do `~/.local/bin`
(bez ruszania pakietów systemowych). Używasz na własne ryzyko — podmieniasz
własny kompozytor / aplety panelu.

| Katalog | Na co | Co robi |
|---|---|---|
| [`cosmic-comp-pamiec-okien`](cosmic-comp-pamiec-okien/) | `cosmic-comp` | zapamiętywanie rozmiaru/pozycji okien pływających, opcjonalnie okna bez ramki |
| [`cosmic-applet-minimize-napisy`](cosmic-applet-minimize-napisy/) | `cosmic-applet-minimize` | podpisy pod zminimalizowanymi oknami |
| [`cosmic-applet-zegar-obsidian`](cosmic-applet-zegar-obsidian/) | `cosmic-applet-time` | klik w dzień kalendarza otwiera notatkę dzienną Obsidian |
| [`cosmic-applets-latki`](cosmic-applets-latki/) | `cosmic-applets` | wspólny skrypt budujący aplety z łatkami + automat przebudowy po aktualizacji |

Każdy katalog ma własny README (po angielsku, polska wersja w `README.pl.md`). Automaty przebudowy to opcjonalne
jednostki `systemd --user` (timer), które sam tworzysz.
