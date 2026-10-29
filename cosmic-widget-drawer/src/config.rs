//! Konfiguracja szuflady — `~/.config/cosmic-widget-drawer/config.toml`.
//!
//! Szuflada świadomie NIE powtarza ustawień źródeł danych (adres iCal,
//! ścieżka vaulta, współrzędne pogody, lista aktywów) — czyta je wprost
//! z configów odpowiednich widgetów pulpitowych (patrz `sources.rs`).
//! Tutaj są tylko rzeczy własne: wygląd i sposób otwierania.
//!
//! Szuflad są dwie: prawa (treść widgetów) i lewa (drzewo vaulta). Klucze
//! bez przedrostka opisują prawą — była pierwsza i to jej dotyczy większość
//! ustawień; lewa ma własną tabelę `[files]` na końcu pliku.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Szerokość szuflady w px logicznych.
    pub width: u32,
    /// Odstęp karty od krawędzi ekranu (px). 0 = przyklejona do krawędzi.
    pub margin: u32,
    /// Które sekcje i w jakiej kolejności: `weather` · `agenda` · `tasks`
    /// · `markets`. Usunięcie nazwy z listy wyłącza sekcję (i jej pobieranie).
    pub sections: Vec<String>,
    /// Co ile odświeżać dane, gdy szuflada jest OTWARTA (s).
    pub refresh_secs: u64,
    /// Po ilu sekundach dane uznajemy za nieświeże i pobieramy je ponownie
    /// przy otwarciu. Świeże dane pokazują się natychmiast, bez „Ładowanie…”.
    pub stale_secs: u64,
    /// Ile wpisów kursów pokazać (lista z widgetu kursów bywa długa).
    pub max_assets: usize,
    /// Przyciemnienie reszty ekranu przy otwartej szufladzie (0.0–1.0).
    pub scrim_opacity: f32,
    /// Czas wysuwania/chowania w ms. 0 = bez animacji.
    pub animation_ms: u64,
    /// Otwieranie najechaniem na prawą krawędź ekranu. Dotyczy wyłącznie
    /// prawej szuflady — lewą krawędź zajmuje u autora dok, więc tam gorący
    /// pasek nie miałby gdzie stanąć (zostaje skrót i gest).
    pub edge_trigger: bool,
    /// Szerokość gorącego paska przy krawędzi (px). Trzymać małą — pasek
    /// leży NAD oknami, więc zjada tyle pikseli z prawej krawędzi.
    pub edge_width: u32,
    /// Jaki procent wysokości ekranu zajmuje gorący pasek (wyśrodkowany
    /// w pionie). Mniej = mniejsza szansa, że zahaczy o pasek przewijania.
    pub edge_height_pct: u32,
    /// Ile ms kursor musi postać przy krawędzi, zanim szuflada wyjedzie.
    pub edge_dwell_ms: u64,
    /// Poziom oszklenia karty — `"dock"` (jak suwak przezroczystości doka),
    /// `"off"` albo liczba `0.0`–`1.0`.
    ///
    /// Tu domyślnie liczba, a NIE `"dock"` jak w widgetach pulpitowych.
    /// Widgety leżą na tapecie, więc rozmycie tła (`ext-background-effect`)
    /// robi im czytelne tło nawet przy suwaku doka na 0.1 — tyle ma go użytkownik.
    /// Szuflada wjeżdża NAD okna, a tam rozmycie tapety nic nie daje: przy
    /// 0.1 przez kartę widać film na YouTubie. Kto chce mieć jak dok —
    /// wpisuje `"dock"`.
    pub glass: String,
    /// Lewa szuflada — drzewo katalogów vaulta Obsidiana.
    ///
    /// MUSI zostać ostatnim polem: TOML wymaga, żeby tabela stała po
    /// wszystkich kluczach skalarnych (ta sama pułapka, co z `[[assets]]`
    /// w widgecie kursów).
    pub files: FilesConfig,
}

/// Lewa szuflada. Ścieżki vaulta nie ma tu celowo — bierze się ją z configu
/// widgetu zadań (`vault_path`, `vault_name`), żeby nie trzymać dwóch
/// prawd o tym samym vaultcie.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FilesConfig {
    /// `false` = lewej szuflady nie ma wcale (skrót i gest nic nie robią).
    pub enabled: bool,
    pub width: u32,
    /// Czym otwierać notatkę: `"system"` (domyślnie — `xdg-open`, czyli
    /// skojarzony edytor tekstu, u autora COSMIC Text Editor) albo
    /// `"obsidian"` (URI `obsidian://`, notatka ląduje w Obsidianie
    /// z wikilinkami i wtyczkami). użytkownik woli goły plik `.md` — 2026-09-18.
    pub open_with: String,
    /// Pokazywać też załączniki (PDF-y, obrazki), nie tylko `.md`.
    pub show_all_files: bool,
    /// Czy po otwarciu notatki szuflada ma się schować.
    pub close_after_open: bool,
}

impl Default for FilesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            width: 360,
            open_with: "system".into(),
            show_all_files: false,
            close_after_open: true,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            width: 420,
            margin: 10,
            sections: vec![
                "weather".into(),
                "agenda".into(),
                "tasks".into(),
                "markets".into(),
            ],
            refresh_secs: 120,
            stale_secs: 300,
            max_assets: 12,
            scrim_opacity: 0.25,
            animation_ms: 140,
            edge_trigger: true,
            edge_width: 3,
            edge_height_pct: 50,
            edge_dwell_ms: 250,
            glass: "0.92".into(),
            files: FilesConfig::default(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cosmic-widget-drawer")
            .join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_else(|e| {
                eprintln!("cosmic-widget-drawer: błąd w {}: {e}", path.display());
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

    pub fn has_section(&self, name: &str) -> bool {
        self.sections.iter().any(|s| s == name)
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn domyslnie_wszystkie_cztery_sekcje() {
        let cfg = Config::default();
        for s in ["weather", "agenda", "tasks", "markets"] {
            assert!(cfg.has_section(s), "brak sekcji {s}");
        }
    }

    #[test]
    fn lewa_szuflada_jest_domyslnie_wlaczona_i_otwiera_plik_md() {
        let cfg = Config::default();
        assert!(cfg.files.enabled);
        assert_eq!(cfg.files.open_with, "system");
    }

    #[test]
    fn round_trip_toml() {
        let cfg = Config::default();
        let raw = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&raw).unwrap();
        assert_eq!(back.width, cfg.width);
        assert_eq!(back.sections, cfg.sections);
        assert_eq!(back.files.width, cfg.files.width);
    }
}
