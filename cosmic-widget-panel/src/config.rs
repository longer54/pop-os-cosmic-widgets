//! Konfiguracja widgetu — `~/.config/cosmic-widget-panel/config.toml`.
//! Plik powstaje z domyślnymi wartościami przy pierwszym uruchomieniu;
//! do działania trzeba w nim uzupełnić `api_key`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub api_url: String,
    pub panel_url: String,
    pub api_key: String,
    pub refresh_secs: u64,
    pub width: u32,
    /// top-left | top-right | bottom-left | bottom-right
    pub anchor: String,
    pub margin_x: i32,
    pub margin_y: i32,
    /// true = pokazuj wszystkie usługi, false = tylko problemy + podsumowanie
    pub show_all: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_url: "https://your-panel.example.com/api/widget-status".into(),
            panel_url: "https://your-panel.example.com/panel/".into(),
            api_key: String::new(),
            refresh_secs: 120,
            width: 330,
            anchor: "top-right".into(),
            margin_x: 20,
            margin_y: 20,
            show_all: false,
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cosmic-widget-panel")
            .join("config.toml")
    }

    /// Wczytaj konfigurację; przy braku pliku zapisz szablon z domyślnymi
    /// wartościami, żeby było co edytować.
    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_else(|e| {
                eprintln!("cosmic-widget-panel: błąd w {}: {e}", path.display());
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
