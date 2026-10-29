//! Dwa zrodla stanu:
//!   * `/etc/default/drm-colortemp.conf` — ustawienia demona (tylko do odczytu,
//!     zapisuje je pomocnik roota),
//!   * maly plik w `~/.config` — czym aplet ostatnio sterowal, zeby popup
//!     pokazywal to samo po ponownym otwarciu.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const KONFIG_DEMONA: &str = "/etc/default/drm-colortemp.conf";
pub const POMOCNIK: &str = "/usr/local/bin/nocny-filtr-apply";

/// Zakres i krok suwaka. Krok 100 K, bo dokladnie te wartosci dopuszcza
/// regula sudo — patrz install-root.sh.
pub const MIN_KELWINY: u32 = 2500;
pub const MAX_KELWINY: u32 = 6500;
pub const KROK_KELWINY: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tryb {
    /// Filtr trzyma temperature ustawiona suwakiem, niezaleznie od pory dnia.
    Wlaczony,
    /// Bez filtra.
    Wylaczony,
    /// Steruje harmonogram godzinowy demona.
    Automatyczny,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stan {
    pub tryb: Tryb,
    pub temperatura: u32,
}

impl Default for Stan {
    fn default() -> Self {
        Self {
            tryb: Tryb::Automatyczny,
            temperatura: 3500,
        }
    }
}

/// Ustawienia odczytane z konfiguracji demona.
#[derive(Debug, Clone, PartialEq)]
pub struct Demon {
    pub night_temp: u32,
    pub day_temp: u32,
    pub sunset_hour: u32,
    pub sunrise_hour: u32,
}

impl Default for Demon {
    fn default() -> Self {
        Self {
            night_temp: 3500,
            day_temp: 6500,
            sunset_hour: 20,
            sunrise_hour: 8,
        }
    }
}

pub fn wczytaj_demona() -> Demon {
    let mut demon = Demon::default();
    let Ok(tekst) = std::fs::read_to_string(KONFIG_DEMONA) else {
        return demon;
    };

    for linia in tekst.lines() {
        let linia = linia.trim();
        if linia.starts_with('#') {
            continue;
        }
        let Some((klucz, wartosc)) = linia.split_once('=') else {
            continue;
        };
        let wartosc = wartosc.trim().trim_matches(['"', '\'']);
        let Ok(liczba) = wartosc.parse::<u32>() else {
            continue;
        };
        match klucz.trim() {
            "NIGHT_TEMP" => demon.night_temp = liczba,
            "DAY_TEMP" => demon.day_temp = liczba,
            "SUNSET_HOUR" => demon.sunset_hour = liczba,
            "SUNRISE_HOUR" => demon.sunrise_hour = liczba,
            _ => {}
        }
    }
    demon
}

fn sciezka_stanu() -> PathBuf {
    let baza = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
        });
    baza.join("cosmic-nocny-filtr.ron")
}

pub fn wczytaj_stan() -> Stan {
    std::fs::read_to_string(sciezka_stanu())
        .ok()
        .and_then(|tekst| ron::from_str(&tekst).ok())
        .unwrap_or_default()
}

pub fn zapisz_stan(stan: &Stan) {
    let sciezka = sciezka_stanu();
    if let Some(katalog) = sciezka.parent() {
        let _ = std::fs::create_dir_all(katalog);
    }
    if let Ok(tekst) = ron::ser::to_string_pretty(stan, ron::ser::PrettyConfig::default()) {
        let _ = std::fs::write(sciezka, tekst);
    }
}

/// Odpala pomocnika roota. Nie czekamy na wynik — przelaczenie konsoli i
/// powrot trwaja okolo poltorej sekundy, a popup ma zostac responsywny.
pub fn zastosuj(argument: &str) {
    let _ = std::process::Command::new("sudo")
        .args(["-n", POMOCNIK, argument])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
