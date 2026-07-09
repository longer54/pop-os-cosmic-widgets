//! Konfiguracja widgetu — `~/.config/cosmic-widget-agenda/config.toml`.
//! Plik powstaje z domyślnymi wartościami przy pierwszym uruchomieniu;
//! do działania trzeba w nim uzupełnić `ics_url` (prywatny adres iCal
//! z ustawień kalendarza Google: Ustawienia -> "Integracja kalendarza"
//! -> "Tajny adres w formacie iCal").

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub ics_url: String,
    pub refresh_secs: u64,
    pub width: u32,
    /// top-left | top-right | bottom-left | bottom-right
    pub anchor: String,
    pub margin_x: i32,
    pub margin_y: i32,
    /// Ile dni naprzód (licząc dzisiaj) pokazywać w harmonogramie.
    pub days_ahead: i64,
    /// Maksymalna liczba wydarzeń widocznych na widgetcie.
    pub max_events: usize,
    /// Adres otwierany po kliknięciu w widget.
    pub calendar_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ics_url: String::new(),
            refresh_secs: 600,
            width: 330,
            // Domyślnie pod cosmic-calendar-widget w tej samej kolumnie
            // top-right (jeśli kalendarz ma margin_y=150 i WIDGET_HEIGHT=430,
            // kończy się ~580, więc 600 zostawia mały odstęp). Dostosuj do
            // własnego układu widgetów.
            anchor: "top-right".into(),
            margin_x: 20,
            margin_y: 600,
            days_ahead: 14,
            max_events: 30,
            calendar_url: "https://calendar.google.com/calendar/r".into(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cosmic-widget-agenda")
            .join("config.toml")
    }

    /// Wczytaj konfigurację; przy braku pliku zapisz szablon z domyślnymi
    /// wartościami, żeby było co edytować.
    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_else(|e| {
                eprintln!("cosmic-widget-agenda: błąd w {}: {e}", path.display());
                Self::default()
            }),
            Err(_) => {
                let cfg = Self::default();
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&path, toml::to_string_pretty(&cfg).unwrap_or_default());
                cfg
            }
        }
    }
}
