//! Konfiguracja widgetu — `~/.config/cosmic-widget-weather/config.toml`.
//! Współrzędne i nazwa stacji domyślnie ustawione na Lublin; do zmiany
//! lokalizacji wystarczy podmienić `lat`/`lon`/`imgw_station`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Szerokość geograficzna (Open-Meteo — prognoza).
    pub lat: f64,
    /// Długość geograficzna (Open-Meteo — prognoza).
    pub lon: f64,
    /// Nazwa stacji w API IMGW (`stacja` w danepubliczne.imgw.pl/api/data/synop)
    /// — realny odczyt "na żywo", nie model.
    pub imgw_station: String,
    pub refresh_secs: u64,
    pub forecast_days: u32,
    pub width: u32,
    /// top-left | top-right | bottom-left | bottom-right
    pub anchor: String,
    pub margin_x: i32,
    pub margin_y: i32,
    /// Adres otwierany po kliknięciu w widget.
    pub weather_url: String,
    /// Nazwa widgetu-rodzica — trzymaj się pod nim w kolumnie;
    /// pusty string = pozycja niezależna z margin_x/margin_y.
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
            lat: 51.2465,
            lon: 22.5684,
            imgw_station: "Lublin".into(),
            // IMGW aktualizuje odczyty co godzinę; 600s wystarcza żeby
            // złapać nową godzinę bez zbędnego pollowania.
            refresh_secs: 600,
            forecast_days: 4,
            width: 330,
            // Wolny róg — status/kalendarz/harmonogram/zadania zajmują
            // już top-right (patrz cosmic-widget-panel).
            anchor: "top-left".into(),
            margin_x: 20,
            margin_y: 20,
            weather_url: "https://www.google.com/search?q=pogoda+Lublin".into(),
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
            .join("cosmic-widget-weather")
            .join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_else(|e| {
                eprintln!("cosmic-widget-weather: błąd w {}: {e}", path.display());
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
