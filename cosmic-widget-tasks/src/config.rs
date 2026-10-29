//! Konfiguracja widgetu — `~/.config/cosmic-widget-tasks/config.toml`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub vault_path: PathBuf,
    /// Host SSH kanonicznej kopii vaulta (alias z `~/.ssh/config`). Tam idą
    /// WSZYSTKIE zapisy widgetu — odhaczanie zadań i dopisywanie `#pamięć` —
    /// żeby dwa komputery nie mogły zapisać dwóch różnych wersji tej samej
    /// notatki i wyprodukować `*.sync-conflict-*`. Czytanie zostaje lokalne,
    /// więc widget działa też bez sieci. Pusty = zapis lokalny, jak dawniej.
    pub remote_host: String,
    /// Ścieżka vaulta na tamtej maszynie — odpowiednik `vault_path`.
    pub remote_vault_path: PathBuf,
    /// Nazwa vaulta zarejestrowana we flatpak Obsidianie (do URI obsidian://).
    pub vault_name: String,
    /// Folder dziennych notatek — tam trafiają wpisy dodane przyciskiem "+"
    /// w sekcji "Do zapamiętania".
    pub daily_notes_folder: String,
    /// Szablon dnia (bez rozszerzenia, względem vaulta) używany gdy notatka
    /// dzisiejsza jeszcze nie istnieje.
    pub daily_note_template: String,
    pub refresh_secs: u64,
    pub width: u32,
    /// top-left | top-right | bottom-left | bottom-right
    pub anchor: String,
    pub margin_x: i32,
    pub margin_y: i32,
    /// Maksymalna liczba zadań widocznych na widgetcie.
    pub max_tasks: usize,
    /// Maksymalna liczba wpisów w sekcji "Do zapamiętania" (#pamięć).
    pub max_memory_items: usize,
    // ---- Karta "Do zapamiętania" ----
    // Od 2026-08-26 ma własną powierzchnię layer-shell, czyli własną
    // pozycję i własny uchwyt ⠿ — przesuwa się niezależnie od zadań.
    /// top-left | top-right | bottom-left | bottom-right
    pub memory_anchor: String,
    pub memory_margin_x: i32,
    pub memory_margin_y: i32,
    /// Widget-rodzic karty pamięci. Domyślnie `"tasks"`, czyli trzyma się
    /// pod kartą zadań i zjeżdża, gdy lista zadań rośnie. Pusty string =
    /// całkiem wolna pozycja z `memory_margin_x`/`memory_margin_y`.
    pub memory_stack_below: String,
    /// Odstęp od karty zadań w stacku (px).
    pub memory_stack_gap: i32,
    /// Ręczna korekta pionu karty pamięci względem pozycji ze stacku
    /// (zapisywana po przeciągnięciu).
    pub memory_stack_offset_y: i32,
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
            vault_path: dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join("Obsidian"),
            remote_host: String::new(),
            remote_vault_path: PathBuf::new(),
            vault_name: "MyVault".into(),
            daily_notes_folder: "Kalendarz".into(),
            daily_note_template: "Kalendarz/Szablony/Szablon Dnia".into(),
            refresh_secs: 90,
            width: 460,
            // Ta sama kolumna top-right co status usług, tuż obok niego:
            // status ma margin_x=20, width=330, więc 20+330+20(odstęp)=370
            // stawia zadania bezpośrednio na lewo od statusu w tym samym
            // rzędzie. Zamienione miejscami z cosmic-widget-agenda 2026-07-09
            // na życzenie użytkownika.
            anchor: "top-right".into(),
            margin_x: 370,
            margin_y: 20,
            max_tasks: 20,
            max_memory_items: 20,
            memory_anchor: "top-right".into(),
            memory_margin_x: 370,
            memory_margin_y: 500,
            memory_stack_below: "tasks".into(),
            memory_stack_gap: 20,
            memory_stack_offset_y: 0,
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

    /// Zapisz konfigurację (np. nową pozycję po przeciągnięciu widgetu).
    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, toml::to_string_pretty(self).unwrap_or_default());
    }
}
