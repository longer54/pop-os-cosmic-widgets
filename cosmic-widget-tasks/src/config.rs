//! Konfiguracja widgetu — `~/.config/cosmic-widget-tasks/config.toml`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub vault_path: PathBuf,
    /// Nazwa vaulta zarejestrowana we flatpak Obsidianie (do URI obsidian://).
    pub vault_name: String,
    pub refresh_secs: u64,
    pub width: u32,
    /// top-left | top-right | bottom-left | bottom-right
    pub anchor: String,
    pub margin_x: i32,
    pub margin_y: i32,
    /// Maksymalna liczba zadań widocznych na widgetcie.
    pub max_tasks: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vault_path: dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join("Obsidian"),
            vault_name: "MyVault".into(),
            refresh_secs: 90,
            width: 460,
            // Domyślnie ta sama kolumna top-right co cosmic-widget-panel,
            // tuż obok niego: jeśli panel ma margin_x=20 i width=330, to
            // 20+330+20(odstęp)=370 stawia ten widget bezpośrednio na lewo
            // od niego w tym samym rzędzie. Dostosuj do własnego układu.
            anchor: "top-right".into(),
            margin_x: 370,
            margin_y: 20,
            max_tasks: 20,
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cosmic-widget-tasks")
            .join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_else(|e| {
                eprintln!("cosmic-widget-tasks: błąd w {}: {e}", path.display());
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
