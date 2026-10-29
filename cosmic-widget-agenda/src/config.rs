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
    /// Maksymalna wysokość karty w px. Gdy lista jest dłuższa, karta
    /// przestaje rosnąć, a treść przewija się kółkiem myszy (jak w
    /// widgecie notatki). `0` = bez limitu, karta rośnie jak dawniej.
    pub max_height: u32,
    /// Adres otwierany po kliknięciu w widget.
    pub calendar_url: String,
    /// Nazwa widgetu-rodzica (np. "calendar") — trzymaj się pod nim w
    /// kolumnie; pusty string = pozycja niezależna z margin_x/margin_y.
    pub stack_below: String,
    /// Odstęp od rodzica w stacku (px).
    pub stack_gap: i32,
    /// Ręczna korekta pionu względem pozycji wyliczonej ze stacku
    /// (zapisywana automatycznie po przeciągnięciu widgetu).
    pub stack_offset_y: i32,
    /// Numer pulpitu wirtualnego (jak w Plasmie, od 1), na którym widget ma
    /// być widoczny; poza nim chowa się sam. `0` = pokazuj wszędzie.
    /// Działa tylko na KDE Plasma (KWin wystawia stan pulpitów przez D-Bus).
    pub only_on_desktop: u32,
    /// Poziom oszklenia karty (szkło w stylu COSMIC):
    /// `"dock"` — tyle, ile ma dok (suwak przezroczystości w ustawieniach
    /// doka, zmiany łapane na żywo), `"off"` — karta nieprzezroczysta,
    /// liczba `0.0`–`1.0` — na sztywno.
    pub glass: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ics_url: String::new(),
            refresh_secs: 600,
            width: 330,
            // Pod kalendarzem Obsidian w tej samej kolumnie top-right
            // (kalendarz: margin_y=150, WIDGET_HEIGHT=430 -> kończy się
            // ~580, więc 600 zostawia mały odstęp). Zamienione miejscami
            // z cosmic-widget-tasks 2026-07-09 na życzenie użytkownika (układ:
            // status+zadania w górnym rzędzie, kalendarz+harmonogram niżej).
            anchor: "top-right".into(),
            margin_x: 20,
            margin_y: 600,
            days_ahead: 14,
            max_events: 30,
            max_height: 360,
            calendar_url: "https://calendar.google.com/calendar/r".into(),
            stack_below: String::new(),
            stack_gap: 12,
            stack_offset_y: 0,
            only_on_desktop: 1,
            glass: "dock".into(),
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

    /// Zapisz konfigurację (np. nową pozycję po przeciągnięciu widgetu).
    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, toml::to_string_pretty(self).unwrap_or_default());
    }
}
