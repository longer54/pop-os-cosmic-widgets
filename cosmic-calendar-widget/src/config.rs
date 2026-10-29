//! Konfiguracja widgetu — `~/.config/cosmic-calendar-widget/config.toml`.
//! Plik powstaje z domyślnymi wartościami przy pierwszym uruchomieniu.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Ścieżka do vaulta Obsidian na dysku.
    pub vault_path: PathBuf,
    /// Nazwa vaulta zarejestrowana w Obsidianie (używana w `obsidian://`).
    pub vault_name: String,
    /// Folder dziennych notatek wewnątrz vaulta.
    pub daily_notes_folder: String,
    /// Ścieżka do szablonu dnia względem vaulta (bez rozszerzenia), jak w
    /// ustawieniach Daily Notes Obsidiana. Puste = brak szablonu.
    pub daily_note_template: String,
    /// Ile słów w notatce = jedna kropka (jak `wordsPerDot` we wtyczce Calendar).
    pub words_per_dot: u32,
    /// Maksymalna liczba kropek pod dniem.
    pub max_dots: u32,
    pub width: u32,
    /// top-left | top-right | bottom-left | bottom-right
    pub anchor: String,
    pub margin_x: i32,
    /// Górny odstęp — domyślnie na tyle duży, żeby widget wylądował pod
    /// widgetem statusu usług (cosmic-widget-panel) w tym samym rogu.
    pub margin_y: i32,
    /// Co robi klik w dzień: `true` = otwiera notatkę w Obsidianie
    /// (`obsidian://`), `false` = otwiera plik `.md` bezpośrednio
    /// (xdg-open, bez Obsidiana). Przełącznik w widgetcie zapisuje tu
    /// swój stan, więc wybór przeżywa restart.
    pub open_in_obsidian: bool,
    /// Nazwa widgetu-rodzica (np. "panel") — trzymaj się pod nim w kolumnie;
    /// pusty string = pozycja niezależna z margin_x/margin_y.
    pub stack_below: String,
    /// Odstęp od rodzica w stacku (px).
    pub stack_gap: i32,
    /// Ręczna korekta pionu względem pozycji wyliczonej ze stacku
    /// (zapisywana automatycznie po przeciągnięciu widgetu).
    pub stack_offset_y: i32,
    /// Numer pulpitu wirtualnego (jak w Plasmie, od 1), na którym widget ma
    /// być widoczny; poza nim chowa się sam. `0` = pokazuj wszędzie
    /// (tak działa layer-shell z natury — patrz `desktop.rs`).
    /// Działa tylko na KDE Plasma, gdzie KWin wystawia stan pulpitów
    /// przez D-Bus.
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
            vault_path: dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join("Obsidian"),
            vault_name: "MyVault".into(),
            daily_notes_folder: "Kalendarz".into(),
            daily_note_template: "Kalendarz/Szablony/Szablon Dnia".into(),
            words_per_dot: 250,
            max_dots: 5,
            width: 330,
            anchor: "top-right".into(),
            margin_x: 20,
            margin_y: 150,
            open_in_obsidian: true,
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
            .join("cosmic-calendar-widget")
            .join("config.toml")
    }

    /// Wczytaj konfigurację; przy braku pliku zapisz szablon z domyślnymi
    /// wartościami, żeby było co edytować.
    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_else(|e| {
                eprintln!("cosmic-calendar-widget: błąd w {}: {e}", path.display());
                Self::default()
            }),
            Err(_) => {
                let cfg = Self::default();
                cfg.save();
                cfg
            }
        }
    }

    /// Zapisz bieżącą konfigurację (np. po przestawieniu przełącznika w
    /// widgetcie). Plik jest w całości generowany z tej struktury, więc
    /// nadpisanie niczego nie gubi.
    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&path, toml::to_string_pretty(self).unwrap_or_default()) {
            eprintln!("cosmic-calendar-widget: nie mogę zapisać {}: {e}", path.display());
        }
    }
}
