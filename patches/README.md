# patches — łatki do komponentów COSMIC

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

Każdy katalog ma własny README (po polsku). Automaty przebudowy to opcjonalne
jednostki `systemd --user` (timer), które sam tworzysz.

## English

Patches against upstream System76 repositories plus scripts that clone upstream,
apply the patches, build and install the binary into `~/.local/bin` (system
packages are left untouched). Use at your own risk — you replace your own
compositor / panel applets. See the table above; per-directory READMEs are in Polish.
