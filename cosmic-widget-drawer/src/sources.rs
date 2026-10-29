//! Źródła danych szuflady — dokładnie te same moduły, z których korzystają
//! widgety pulpitowe.
//!
//! **Zero kopiowania kodu:** moduły pobierające dane i ich konfiguracje są
//! WŁĄCZONE do tego crate'a przez `#[path = "..."]`, wprost z katalogów
//! tamtych widgetów. Dzięki temu poprawka w parserze iCal-a albo w skanerze
//! zadań działa w widgecie i w szufladzie naraz, a szuflada nie ma własnych
//! ustawień typu „adres kalendarza” — czyta config widgetu.
//!
//! Konsekwencja, o której trzeba pamiętać: zmiana sygnatury w
//! `cosmic-widget-*/src/{ics,tasks,weather,market}.rs` łamie też build
//! szuflady.

// ---- Harmonogram (Kalendarz Google przez prywatny feed iCal) ----
#[allow(dead_code)]
#[path = "../../cosmic-widget-agenda/src/ics.rs"]
pub mod ics;
#[allow(dead_code)]
#[path = "../../cosmic-widget-agenda/src/config.rs"]
pub mod agenda_config;

// ---- Zadania (skan vaulta Obsidiana, składnia wtyczki Tasks) ----
#[allow(dead_code)]
#[path = "../../cosmic-widget-tasks/src/tasks.rs"]
pub mod tasks;
#[allow(dead_code)]
#[path = "../../cosmic-widget-tasks/src/config.rs"]
pub mod tasks_config;

// ---- Pogoda (stacja IMGW + prognoza Open-Meteo) ----
#[allow(dead_code)]
#[path = "../../cosmic-widget-weather/src/weather.rs"]
pub mod weather;
#[allow(dead_code)]
#[path = "../../cosmic-widget-weather/src/config.rs"]
pub mod weather_config;

// ---- Kursy aktywów (CoinGecko + Yahoo Finance) ----
#[allow(dead_code)]
#[path = "../../cosmic-widget-markets/src/market.rs"]
pub mod market;
#[allow(dead_code)]
#[path = "../../cosmic-widget-markets/src/config.rs"]
pub mod markets_config;

use std::collections::HashMap;

/// Jeden wiersz sekcji kursów — już rozwiązany (nazwa + cena + zmiana),
/// żeby widok nie musiał grzebać w dwóch mapach naraz.
#[derive(Debug, Clone)]
pub struct MarketRow {
    pub name: String,
    pub price: f64,
    pub change_pct: f64,
    pub currency: String,
}

/// Pobierz kursy dla listy aktywów z configu widgetu kursów.
///
/// Krypto leci jednym zbiorczym strzałem do CoinGecko (tak jak w widgecie),
/// reszta po jednym zapytaniu do Yahoo na symbol. Aktywo, którego nie udało
/// się pobrać, po prostu nie wchodzi na listę — szuflada ma pokazać to, co
/// wiadomo, a nie czerwienić się na jednym nieudanym symbolu.
pub async fn fetch_markets(
    assets: Vec<markets_config::AssetCfg>,
    limit: usize,
) -> Result<Vec<MarketRow>, String> {
    // Ulubione najpierw — tak samo, jak sortuje je widget kursów.
    let mut wanted: Vec<markets_config::AssetCfg> = assets;
    wanted.sort_by_key(|a| !a.favorite);
    wanted.truncate(limit);
    if wanted.is_empty() {
        return Ok(Vec::new());
    }

    let crypto_ids: Vec<String> = wanted
        .iter()
        .filter(|a| market::Source::from_str(&a.source) == market::Source::Coingecko)
        .map(|a| a.symbol.clone())
        .collect();

    let crypto: HashMap<String, market::Quote> = if crypto_ids.is_empty() {
        HashMap::new()
    } else {
        market::fetch_crypto_quotes(crypto_ids).await.unwrap_or_default()
    };

    let mut rows = Vec::with_capacity(wanted.len());
    for asset in wanted {
        let quote = match market::Source::from_str(&asset.source) {
            market::Source::Coingecko => crypto.get(&asset.symbol).cloned(),
            market::Source::Yahoo => market::fetch_yahoo_quote(asset.symbol.clone())
                .await
                .ok()
                .map(|(q, _history)| q),
        };
        if let Some(q) = quote {
            rows.push(MarketRow {
                name: asset.name,
                price: q.price,
                change_pct: q.change_pct,
                currency: q.currency,
            });
        }
    }

    if rows.is_empty() {
        return Err("żadne aktywo się nie pobrało".into());
    }
    Ok(rows)
}

/// Skan zadań z vaulta. Robota na dysku, więc woła się to przez
/// `spawn_blocking` — inaczej zamula pętlę interfejsu przy dużym vaultcie.
pub async fn fetch_tasks(
    vault_path: std::path::PathBuf,
    max_tasks: usize,
) -> Result<Vec<tasks::TaskItem>, String> {
    tokio::task::spawn_blocking(move || tasks::scan_tasks(&vault_path, max_tasks))
        .await
        .map_err(|e| e.to_string())?
}
