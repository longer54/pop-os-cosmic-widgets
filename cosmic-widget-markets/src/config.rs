//! Konfiguracja widgetu — `~/.config/cosmic-widget-markets/config.toml`.
//! Lista aktywów jest edytowalna z poziomu widgetu (przycisk +, usuwanie,
//! gwiazdka ulubionych) — dlatego `save()` musi być atomowy (tmp + rename),
//! żeby Syncthing/awaria nie zostawiły połówki pliku.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Jedno aktywo na liście. `id` jest unikalne: dla CoinGecko to id monety
/// (np. "bitcoin"), dla Yahoo symbol (np. "AAPL", "CDR.WA", "GC=F").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetCfg {
    pub id: String,
    pub name: String,
    /// "coingecko" | "yahoo"
    pub source: String,
    pub symbol: String,
    /// "krypto" | "us" | "gpw" | "surowce" | "etf"
    pub category: String,
    pub favorite: bool,
}

impl AssetCfg {
    fn new(id: &str, name: &str, source: &str, symbol: &str, category: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            source: source.into(),
            symbol: symbol.into(),
            category: category.into(),
            favorite: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub refresh_secs: u64,
    pub width: u32,
    /// top-left | top-right | bottom-left | bottom-right
    pub anchor: String,
    pub margin_x: i32,
    pub margin_y: i32,
    /// Nazwa widgetu-rodzica (np. "tasks") — trzymaj się pod nim w kolumnie;
    /// pusty string = pozycja niezależna z margin_x/margin_y.
    /// (Musi być przed `assets` — TOML wymaga skalarów przed tablicą tabel.)
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
    pub assets: Vec<AssetCfg>,
    /// Poziom oszklenia karty (szkło w stylu COSMIC):
    /// `"dock"` — tyle, ile ma dok (suwak przezroczystości w ustawieniach
    /// doka, zmiany łapane na żywo), `"off"` — karta nieprzezroczysta,
    /// liczba `0.0`–`1.0` — na sztywno.
    pub glass: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_secs: 120,
            width: 330,
            // Ta sama kolumna co cosmic-widget-tasks (margin_x=370), tuż pod
            // kartą "Do zapamiętania" z proporcjonalnym odstępem (feedback
            // użytkownika 2026-07-14: dziura przy kotwicy dolnej była za duża).
            // Powierzchnia zadań przy 6 zadaniach + 5 wpisach pamięci kończy
            // się ok. 558px (margin 20 + 274 + gap 20 + 244) — stare 480
            // wpadało W NIĄ i dlatego ginęły kliknięcia, nie przez "hojną"
            // wysokość. Jak listy urosną, podbić margin_y w
            // ~/.config/cosmic-widget-markets/config.toml.
            anchor: "top-right".into(),
            margin_x: 370,
            margin_y: 580,
            stack_below: String::new(),
            stack_gap: 12,
            stack_offset_y: 0,
            only_on_desktop: 1,
            assets: default_assets(),
            glass: "dock".into(),
        }
    }
}

/// Przykładowa watchlista startowa (krypto + kilka akcji/surowców) — edytuj
/// w `~/.config/cosmic-widget-markets/config.toml`.
fn default_assets() -> Vec<AssetCfg> {
    let cg = |id: &str, name: &str| AssetCfg::new(id, name, "coingecko", id, "krypto");
    let y = |sym: &str, name: &str, cat: &str| AssetCfg::new(sym, name, "yahoo", sym, cat);
    vec![
        // ---- Krypto (CoinGecko) ----
        cg("bitcoin", "Bitcoin"),
        cg("ethereum", "Ethereum"),
        // ---- Akcje US (Yahoo) ----
        y("AAPL", "Apple", "us"),
        y("MSFT", "Microsoft", "us"),
        y("NVDA", "Nvidia", "us"),
        // ---- GPW ----
        y("CDR.WA", "CD Projekt", "gpw"),
        // ---- Surowce (futures) ----
        y("GC=F", "Złoto", "surowce"),
    ]
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cosmic-widget-markets")
            .join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_else(|e| {
                eprintln!("cosmic-widget-markets: błąd w {}: {e}", path.display());
                Self::default()
            }),
            Err(_) => {
                let cfg = Self::default();
                let _ = cfg.save();
                cfg
            }
        }
    }

    /// Zapis atomowy: najpierw plik tymczasowy, potem rename.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let raw = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, raw).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ma_pelna_watchliste_ulubionych() {
        let cfg = Config::default();
        assert_eq!(cfg.assets.len(), 31);
        assert!(cfg.assets.iter().all(|a| a.favorite));
        assert_eq!(cfg.assets.iter().filter(|a| a.category == "krypto").count(), 8);
        assert!(cfg.assets.iter().all(|a| a.source == "coingecko" || a.source == "yahoo"));
    }

    #[test]
    fn config_toml_round_trip() {
        let cfg = Config::default();
        let raw = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&raw).unwrap();
        assert_eq!(back.assets, cfg.assets);
        assert_eq!(back.refresh_secs, cfg.refresh_secs);
    }
}
