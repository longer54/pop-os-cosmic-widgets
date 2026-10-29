//! Konfiguracja podglądu — `~/.config/cosmic-quicklook/config.toml`.
//!
//! Plik powstaje sam przy pierwszym uruchomieniu (z komentarzami), żeby było
//! co edytować bez zaglądania do kodu.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Ile procent szerokości / wysokości obszaru roboczego zajmuje karta
    /// podglądu. Karta i tak kurczy się do treści (obrazek w pionie nie
    /// rozciąga się na całą szerokość).
    pub width_pct: u32,
    pub height_pct: u32,
    /// Przyciemnienie reszty ekranu (0.0–1.0).
    pub scrim_opacity: f32,
    /// Oszklenie karty — `"dock"` (jak suwak przezroczystości doka), `"off"`
    /// albo liczba `0.0`–`1.0`. Domyślnie prawie nieprzezroczysta: podgląd
    /// wchodzi NAD okna, a przez półprzezroczystą kartę nie widać zdjęcia.
    pub glass: String,

    /// Które okna wolno „zapytać” o zaznaczenie przez Ctrl+C. Wpis pasuje
    /// do całego `app_id` albo do jego ostatniego członu, więc `dolphin`
    /// łapie `org.kde.dolphin`.
    ///
    /// **Nie dopisywać tu terminali ani edytorów.** Ctrl+C wysłane do
    /// terminala to SIGINT — ubity build zamiast podglądu.
    pub file_managers: Vec<String>,
    /// Ile ms czekać, aż menedżer plików odpowie na Ctrl+C i przejmie
    /// schowek. Za mało = „nie widzę zaznaczonego pliku” przy obciążonym
    /// systemie.
    pub settle_ms: u64,

    /// Ile kilobajtów pliku tekstowego wczytać do podglądu.
    pub text_max_kb: usize,
    /// Rozdzielczość renderowania stron PDF (DPI). Więcej = ostrzej i wolniej.
    pub pdf_dpi: u32,
    /// Czy po otwarciu pliku w docelowej aplikacji (Enter) zamykać podgląd.
    pub close_after_open: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            width_pct: 62,
            height_pct: 78,
            scrim_opacity: 0.45,
            glass: "0.96".into(),
            file_managers: vec![
                "com.system76.CosmicFiles".into(),
                "org.kde.dolphin".into(),
                "org.gnome.Nautilus".into(),
                "nemo".into(),
                "pcmanfm".into(),
                "thunar".into(),
            ],
            settle_ms: 180,
            text_max_kb: 256,
            pdf_dpi: 120,
            close_after_open: true,
        }
    }
}

impl Config {
    pub fn path() -> std::path::PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("cosmic-quicklook/config.toml")
    }

    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(s) => match toml::from_str(&s) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("cosmic-quicklook: {} — {e}; biorę domyślne", path.display());
                    Self::default()
                }
            },
            Err(_) => {
                let cfg = Self::default();
                cfg.write_default();
                cfg
            }
        }
    }

    fn write_default(&self) {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let Ok(body) = toml::to_string_pretty(self) else {
            return;
        };
        let _ = std::fs::write(
            &path,
            format!(
                "# Quick Look dla COSMIC — podgląd zaznaczonego pliku.\n\
                 # Skrót: Super+Spacja. Zamyka: Spacja, Esc albo klik obok.\n\
                 # Do `file_managers` NIE dopisywać terminali: podgląd pyta\n\
                 # okno na wierzchu o zaznaczenie, udając Ctrl+C.\n\n{body}"
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn domyslny_config_przechodzi_przez_toml() {
        let c = Config::default();
        let s = toml::to_string(&c).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back.width_pct, c.width_pct);
        assert!(back.file_managers.iter().any(|f| f.contains("CosmicFiles")));
    }
}
