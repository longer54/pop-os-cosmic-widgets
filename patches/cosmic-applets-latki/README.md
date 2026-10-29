# Strażnik łatek apletów panelu COSMIC

Łatane aplety siedzą w `~/.local/bin` i muszą pasować do wersji pakietu
`cosmic-applets`:

| binarka | łatka |
|---|---|
| `cosmic-applet-battery` | `tryby-mocy-cpu/` (nie ma w tym repo — pomijane, jeśli brak) |
| `cosmic-applet-minimize` | `cosmic-applet-minimize-napisy/` |
| `cosmic-applet-time` | `cosmic-applet-zegar-obsidian/` |

Timer `cosmic-applets-latki.timer` (systemd --user, co 15 min + 3 min po
starcie) odpala `auto-przebuduj.sh`. Gdy wersja pakietu się zmieniła,
przebudowuje w tle każdą łatkę obecną na urządzeniu, robi `pkill -x cosmic-panel`
i wysyła powiadomienie. Gdy build padnie: ta łatka wraca do stockowej wersji,
powiadomienie krytyczne, bez ponawiania do następnej aktualizacji.

- Instalacja timera: utwórz `~/.config/systemd/user/cosmic-applets-latki.{service,timer}`
  (service uruchamia `auto-przebuduj.sh`, timer `OnBootSec=3min`, `OnUnitActiveSec=15min`).
- Stan: `~/.local/state/cosmic-applets-latki.wersja`, log: `~/.local/state/cosmic-applets-latki.log`.
- Wymuszenie przebudowy: wpisz do pliku stanu dowolną inną wersję i
  `systemctl --user start cosmic-applets-latki.service`.
