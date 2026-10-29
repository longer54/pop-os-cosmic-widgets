//! Konfiguracja widgetu — `~/.config/cosmic-widget-note/config.toml`.
//! Plik powstaje z domyślnymi wartościami przy pierwszym uruchomieniu.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Ścieżka do vaulta Obsidian na dysku (stąd widget CZYTA).
    pub vault_path: PathBuf,
    /// Host SSH kanonicznej kopii vaulta (alias z `~/.ssh/config`). Tam idą
    /// wszystkie ZAPISY — tak samo jak w widgetcie zadań — żeby dwa
    /// komputery nie zapisały dwóch wersji tej samej notatki i nie zrobiły
    /// `*.sync-conflict-*`. Pusty = zapis lokalny.
    pub remote_host: String,
    /// Ścieżka vaulta na tamtej maszynie — odpowiednik `vault_path`.
    pub remote_vault_path: PathBuf,
    /// Nazwa vaulta zarejestrowana w Obsidianie (do URI `obsidian://`).
    pub vault_name: String,
    /// Plik notatki WZGLĘDEM vaulta. Jedna, ogólna notatka — nie dzienna.
    pub note_file: String,
    /// Katalog (względem vaulta), do którego Super+Shift+N odkłada
    /// dotychczasową treść kartki przed wyczyszczeniem jej na nową notatkę.
    pub archive_folder: String,
    /// Nagłówek na karcie.
    pub title: String,
    /// Co robi przycisk otwierania: `false` (domyślnie) = otwiera plik `.md`
    /// systemowo (`xdg-open`, u autora COSMIC Text Editor), `true` = otwiera
    /// notatkę w Obsidianie przez `obsidian://`. Tak samo jak przełącznik
    /// „Obsidian" w widgecie kalendarza, który użytkownik trzyma wyłączony.
    pub open_in_obsidian: bool,
    pub width: u32,
    pub height: u32,
    /// top-left | top-right | bottom-left | bottom-right
    pub anchor: String,
    pub margin_x: i32,
    pub margin_y: i32,
    /// Nazwa widgetu-rodzica (np. "weather") — trzymaj się pod nim w
    /// kolumnie; pusty string = pozycja niezależna z margin_x/margin_y.
    pub stack_below: String,
    /// Odstęp od rodzica w stacku (px).
    pub stack_gap: i32,
    /// Ręczna korekta pionu względem pozycji wyliczonej ze stacku
    /// (zapisywana automatycznie po przeciągnięciu widgetu).
    pub stack_offset_y: i32,
    /// Numer pulpitu wirtualnego (jak w Plasmie, od 1), poza którym widget
    /// się chowa. `0` = pokazuj wszędzie. Działa tylko na KDE Plasma —
    /// na sesji COSMIC zostawić 0.
    pub only_on_desktop: u32,
    /// Wygląd karty: `false` (domyślnie) = szkło jak reszta widgetów pulpitu,
    /// `true` = kremowa kartka jak Notatki w macOS.
    pub paper: bool,
    /// Poziom oszklenia przy `paper = false`: `"dock"` · `"off"` · `0.0`–`1.0`.
    pub glass: String,
    /// Rozmiar tekstu notatki (px).
    pub font_size: u16,
    /// Po ilu sekundach bezczynności zapisać to, co użytkownik napisał.
    pub autosave_secs: u64,
    /// Co ile sekund sprawdzać, czy notatka zmieniła się poza widgetem
    /// (edycja w Obsidianie, Syncthing z innego komputera).
    pub reload_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vault_path: dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join("Obsidian"),
            remote_host: String::new(),
            remote_vault_path: PathBuf::new(),
            vault_name: "MyVault".into(),
            note_file: "Notatnik.md".into(),
            archive_folder: "zzzNotatnik".into(),
            title: "Notatka".into(),
            open_in_obsidian: false,
            width: 330,
            height: 300,
            // Lewa kolumna, pod widgetem pogody (ten sam anchor i szerokość).
            anchor: "top-left".into(),
            margin_x: 22,
            margin_y: 330,
            stack_below: "weather".into(),
            stack_gap: 20,
            stack_offset_y: 0,
            only_on_desktop: 0,
            paper: false,
            glass: "dock".into(),
            font_size: 14,
            autosave_secs: 2,
            reload_secs: 15,
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cosmic-widget-note")
            .join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_else(|e| {
                eprintln!("cosmic-widget-note: błąd w {}: {e}", path.display());
                Self::default()
            }),
            Err(_) => {
                let cfg = Self::default();
                cfg.save();
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
        if let Err(e) = std::fs::write(&path, toml::to_string_pretty(self).unwrap_or_default()) {
            eprintln!("cosmic-widget-note: nie mogę zapisać {}: {e}", path.display());
        }
    }
}
