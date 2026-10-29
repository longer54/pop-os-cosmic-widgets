//! Dane rynkowe z dwóch darmowych źródeł bez kluczy:
//! - CoinGecko — krypto (zbiorcze ceny + historia + wyszukiwarka),
//! - Yahoo Finance v8 chart — akcje/GPW/surowce/ETF-y (cena i historia
//!   z tego samego endpointu; wymaga przeglądarkowego User-Agent).
//! Parsowanie oddzielone od HTTP (czyste funkcje `parse_*` — testowalne
//! na fixture'ach), awaria jednego aktywa nie blokuje pozostałych.

use std::collections::HashMap;
use std::time::{Duration, Instant};

const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36";
const TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    Coingecko,
    Yahoo,
}

impl Source {
    pub fn from_str(s: &str) -> Self {
        if s == "coingecko" { Source::Coingecko } else { Source::Yahoo }
    }
}

#[derive(Debug, Clone)]
pub struct Quote {
    pub price: f64,
    pub change_pct: f64,
    /// Kod waluty ("USD", "PLN", "EUR", "DKK"…).
    pub currency: String,
    pub fetched_at: Instant,
}

#[derive(Debug, Clone)]
pub struct History {
    /// (znacznik czasu w sekundach, cena) — rosnąco.
    pub points: Vec<(i64, f64)>,
    pub fetched_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Range {
    H24,
    D7,
    M1,
    Y1,
}

impl Range {
    /// (range, interval) dla Yahoo v8 chart.
    fn yahoo_params(&self) -> (&'static str, &'static str) {
        match self {
            Range::H24 => ("1d", "5m"),
            Range::D7 => ("7d", "60m"),
            Range::M1 => ("1mo", "1d"),
            Range::Y1 => ("1y", "1d"),
        }
    }

    fn coingecko_days(&self) -> &'static str {
        match self {
            Range::H24 => "1",
            Range::D7 => "7",
            Range::M1 => "30",
            Range::Y1 => "365",
        }
    }

    /// Ważność cache'u historii. Dłuższe zakresy trzymamy dłużej — rzadziej
    /// się zmieniają, a przy krypto (rate limit CoinGecko) każdy oszczędzony
    /// strzał to mniejsze ryzyko 429.
    pub fn cache_ttl(&self) -> Duration {
        match self {
            Range::H24 => Duration::from_secs(5 * 60),
            Range::D7 => Duration::from_secs(15 * 60),
            Range::M1 => Duration::from_secs(20 * 60),
            Range::Y1 => Duration::from_secs(30 * 60),
        }
    }
}

/// Szerokość okna, jaką dla D7/M1 wycinamy lokalnie z szerszego zakresu
/// (Y1) zamiast dociągać osobno z API — te same wartości, jakich niegdyś
/// używał mini-wykres widgetu (dziś zastąpiony pełnym wykresem w
/// przeglądarce).
pub const WINDOW_D7_SECS: i64 = 604_800;
pub const WINDOW_M1_SECS: i64 = 2_678_400;

/// Wycina okno kończące się na OSTATNIM punkcie serii (nie „teraz" — giełda
/// bywa zamknięta, ostatni punkt to ostatnia sesja). Jeśli okno zostawia
/// mniej niż 2 punkty (zbyt wąskie względem gęstości danych), zwraca całą
/// serię zamiast prawie pustego wykresu.
pub fn window_slice(points: &[(i64, f64)], window_secs: i64) -> Vec<(i64, f64)> {
    let Some(&(last_ts, _)) = points.last() else {
        return Vec::new();
    };
    let cut = last_ts - window_secs;
    let sliced: Vec<(i64, f64)> = points.iter().filter(|(t, _)| *t >= cut).copied().collect();
    if sliced.len() < 2 {
        points.to_vec()
    } else {
        sliced
    }
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub name: String,
    pub symbol: String,
    /// "coingecko" | "yahoo"
    pub source: String,
    /// id dla configu (CoinGecko id albo symbol Yahoo)
    pub id: String,
    /// zgadnięta kategoria: krypto/us/gpw/surowce/etf
    pub category: String,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(UA)
        .build()
        .map_err(|e| e.to_string())
}

// =========================================================================
// CoinGecko
// =========================================================================

/// Zbiorcze ceny wszystkich krypto jednym strzałem (klucz = CoinGecko id).
pub async fn fetch_crypto_quotes(ids: Vec<String>) -> Result<HashMap<String, Quote>, String> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd&include_24hr_change=true",
        ids.join(",")
    );
    let raw = get_text(&url).await?;
    parse_cg_prices(&raw)
}

pub fn parse_cg_prices(raw: &str) -> Result<HashMap<String, Quote>, String> {
    let v: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let obj = v.as_object().ok_or("odpowiedź CoinGecko nie jest obiektem")?;
    let now = Instant::now();
    let mut out = HashMap::new();
    for (id, entry) in obj {
        let Some(price) = entry.get("usd").and_then(|x| x.as_f64()) else { continue };
        let change = entry.get("usd_24h_change").and_then(|x| x.as_f64()).unwrap_or(0.0);
        out.insert(
            id.clone(),
            Quote { price, change_pct: change, currency: "USD".into(), fetched_at: now },
        );
    }
    Ok(out)
}

pub fn parse_cg_chart(raw: &str) -> Result<Vec<(i64, f64)>, String> {
    let v: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let prices = v
        .get("prices")
        .and_then(|p| p.as_array())
        .ok_or("brak 'prices' w odpowiedzi CoinGecko")?;
    let mut out = Vec::with_capacity(prices.len());
    for p in prices {
        let (Some(ts), Some(price)) = (
            p.get(0).and_then(|x| x.as_f64()),
            p.get(1).and_then(|x| x.as_f64()),
        ) else {
            continue;
        };
        out.push(((ts / 1000.0) as i64, price));
    }
    Ok(out)
}

// =========================================================================
// Yahoo Finance
// =========================================================================

/// Cena + zmiana + historia 24h jednym strzałem (v8 chart, range=1d).
/// Zwraca (Quote, punkty_24h) — punkty idą od razu do cache'u H24.
pub async fn fetch_yahoo_quote(symbol: String) -> Result<(Quote, Vec<(i64, f64)>), String> {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?range=1d&interval=5m",
        urlencode(&symbol)
    );
    let raw = get_text(&url).await?;
    parse_yahoo_chart(&raw)
}

pub async fn fetch_yahoo_history(symbol: String, range: Range) -> Result<Vec<(i64, f64)>, String> {
    let (r, i) = range.yahoo_params();
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?range={r}&interval={i}",
        urlencode(&symbol)
    );
    let raw = get_text(&url).await?;
    parse_yahoo_chart(&raw).map(|(_, points)| points)
}

/// Parsuje odpowiedź v8 chart: meta (cena, poprzednie zamknięcie, waluta)
/// + serie punktów (timestampy w sekundach, close z możliwymi null-ami).
pub fn parse_yahoo_chart(raw: &str) -> Result<(Quote, Vec<(i64, f64)>), String> {
    let v: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let chart = v.get("chart").ok_or("brak 'chart' w odpowiedzi Yahoo")?;
    if let Some(err) = chart.get("error").filter(|e| !e.is_null()) {
        let code = err.get("code").and_then(|c| c.as_str()).unwrap_or("?");
        return Err(format!("Yahoo: {code}"));
    }
    let result = chart
        .get("result")
        .and_then(|r| r.get(0))
        .ok_or("pusty 'result' w odpowiedzi Yahoo")?;
    let meta = result.get("meta").ok_or("brak 'meta' w odpowiedzi Yahoo")?;

    let price = meta
        .get("regularMarketPrice")
        .and_then(|x| x.as_f64())
        .ok_or("brak ceny w odpowiedzi Yahoo")?;
    let prev = meta
        .get("chartPreviousClose")
        .and_then(|x| x.as_f64())
        .filter(|p| *p > 0.0);
    let change_pct = prev.map(|p| (price - p) / p * 100.0).unwrap_or(0.0);
    let currency = meta
        .get("currency")
        .and_then(|c| c.as_str())
        .unwrap_or("USD")
        .to_string();

    let timestamps: Vec<i64> = result
        .get("timestamp")
        .and_then(|t| t.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
        .unwrap_or_default();
    let closes = result
        .pointer("/indicators/quote/0/close")
        .and_then(|c| c.as_array());

    let mut points = Vec::new();
    if let Some(closes) = closes {
        for (i, c) in closes.iter().enumerate() {
            if let (Some(ts), Some(price)) = (timestamps.get(i), c.as_f64()) {
                points.push((*ts, price));
            }
        }
    }

    Ok((
        Quote { price, change_pct, currency, fetched_at: Instant::now() },
        points,
    ))
}

// =========================================================================
// Historia (wspólne wejście dla obu źródeł)
// =========================================================================

pub async fn fetch_history(source: Source, symbol: String, range: Range) -> Result<History, String> {
    let points = match source {
        Source::Yahoo => fetch_yahoo_history(symbol, range).await?,
        Source::Coingecko => {
            let url = format!(
                "https://api.coingecko.com/api/v3/coins/{}/market_chart?vs_currency=usd&days={}",
                urlencode(&symbol),
                range.coingecko_days()
            );
            let raw = get_text(&url).await?;
            parse_cg_chart(&raw)?
        }
    };
    if points.len() < 2 {
        return Err("za mało punktów do wykresu".into());
    }
    Ok(History { points, fetched_at: Instant::now() })
}

// =========================================================================
// Wyszukiwarka (przycisk +): CoinGecko i Yahoo równolegle
// =========================================================================

pub async fn search(query: String) -> Result<Vec<SearchHit>, String> {
    let q = query.trim().to_string();
    if q.len() < 2 {
        return Ok(Vec::new());
    }
    let cg_url = format!("https://api.coingecko.com/api/v3/search?query={}", urlencode(&q));
    let y_url = format!(
        "https://query1.finance.yahoo.com/v1/finance/search?q={}&quotesCount=6&newsCount=0",
        urlencode(&q)
    );
    let (cg_raw, y_raw) = tokio::join!(get_text(&cg_url), get_text(&y_url));

    let mut hits = Vec::new();
    if let Ok(raw) = cg_raw {
        hits.extend(parse_cg_search(&raw));
    }
    if let Ok(raw) = y_raw {
        hits.extend(parse_yahoo_search(&raw));
    }
    if hits.is_empty() {
        return Err("nic nie znaleziono".into());
    }
    hits.truncate(5);
    Ok(hits)
}

pub fn parse_cg_search(raw: &str) -> Vec<SearchHit> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else { return Vec::new() };
    let Some(coins) = v.get("coins").and_then(|c| c.as_array()) else { return Vec::new() };
    coins
        .iter()
        .take(2)
        .filter_map(|c| {
            let id = c.get("id")?.as_str()?.to_string();
            let name = c.get("name")?.as_str()?.to_string();
            let symbol = c.get("symbol").and_then(|s| s.as_str()).unwrap_or("").to_string();
            Some(SearchHit {
                name,
                symbol,
                source: "coingecko".into(),
                id: id.clone(),
                category: "krypto".into(),
            })
        })
        .collect()
}

pub fn parse_yahoo_search(raw: &str) -> Vec<SearchHit> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else { return Vec::new() };
    let Some(quotes) = v.get("quotes").and_then(|q| q.as_array()) else { return Vec::new() };
    quotes
        .iter()
        .filter_map(|q| {
            let symbol = q.get("symbol")?.as_str()?.to_string();
            let name = q
                .get("shortname")
                .or_else(|| q.get("longname"))
                .and_then(|n| n.as_str())
                .unwrap_or(&symbol)
                .trim()
                .to_string();
            let qtype = q.get("quoteType").and_then(|t| t.as_str()).unwrap_or("");
            let exchange = q.get("exchange").and_then(|e| e.as_str()).unwrap_or("");
            let category = match (qtype, exchange) {
                ("ETF", _) => "etf",
                ("FUTURE", _) | ("COMMODITY", _) => "surowce",
                ("CRYPTOCURRENCY", _) => return None, // krypto bierzemy z CoinGecko
                (_, "WSE") => "gpw",
                _ => "us",
            };
            Some(SearchHit {
                name,
                symbol: symbol.clone(),
                source: "yahoo".into(),
                id: symbol,
                category: category.into(),
            })
        })
        .take(4)
        .collect()
}

// =========================================================================
// Pomocnicze
// =========================================================================

async fn get_text(url: &str) -> Result<String, String> {
    // Retry na błędy przejściowe (429/5xx/sieć). CoinGecko trzyma 429 do końca
    // minutowego okna, więc to nie ratuje serii kliknięć (od tego jest kolaps
    // zakresów krypto w app.rs) — łapie za to pojedyncze zbiegi w czasie.
    const RETRY_DELAYS_MS: [u64; 2] = [1500, 4000];
    let client = client()?;
    let mut last_err = String::new();
    for attempt in 0..=RETRY_DELAYS_MS.len() {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(RETRY_DELAYS_MS[attempt - 1])).await;
        }
        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return resp.text().await.map_err(|e| e.to_string());
                }
                last_err = format!("HTTP {}", status.as_u16());
                // Ponawiamy tylko to, co ma sens — 429 i błędy serwera.
                if status.as_u16() != 429 && !status.is_server_error() {
                    return Err(last_err);
                }
            }
            Err(e) => last_err = e.to_string(),
        }
    }
    Err(last_err)
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsuje_ceny_coingecko() {
        let raw = r#"{"bitcoin":{"usd":64189,"usd_24h_change":0.5},"solana":{"usd":78.01,"usd_24h_change":-1.15}}"#;
        let q = parse_cg_prices(raw).unwrap();
        assert_eq!(q.len(), 2);
        assert!((q["bitcoin"].price - 64189.0).abs() < 1e-9);
        assert!(q["solana"].change_pct < 0.0);
        assert_eq!(q["bitcoin"].currency, "USD");
    }

    #[test]
    fn parsuje_historie_coingecko() {
        let raw = r#"{"prices":[[1783148475369,62451.8],[1783152088864,62532.2]]}"#;
        let pts = parse_cg_chart(raw).unwrap();
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0].0, 1783148475);
        assert!((pts[1].1 - 62532.2).abs() < 1e-9);
    }

    #[test]
    fn parsuje_yahoo_chart_z_nullami() {
        let raw = r#"{"chart":{"result":[{"meta":{"regularMarketPrice":315.32,"chartPreviousClose":289.36,"currency":"USD"},"timestamp":[100,200,300],"indicators":{"quote":[{"close":[310.0,null,315.32]}]}}],"error":null}}"#;
        let (q, pts) = parse_yahoo_chart(raw).unwrap();
        assert!((q.price - 315.32).abs() < 1e-9);
        assert!(q.change_pct > 8.9 && q.change_pct < 9.0);
        assert_eq!(q.currency, "USD");
        // null w środku serii jest pomijany
        assert_eq!(pts, vec![(100, 310.0), (300, 315.32)]);
    }

    #[test]
    fn yahoo_blad_daje_err() {
        let raw = r#"{"chart":{"result":null,"error":{"code":"Not Found","description":"No data"}}}"#;
        let err = parse_yahoo_chart(raw).unwrap_err();
        assert!(err.contains("Not Found"));
    }

    #[test]
    fn parsuje_wyszukiwarke_yahoo_z_kategoria() {
        let raw = r#"{"quotes":[
            {"symbol":"PCO.WA","shortname":"PEPCO","quoteType":"EQUITY","exchange":"WSE"},
            {"symbol":"SPYD.DE","shortname":"SPDR Divi","quoteType":"ETF","exchange":"GER"},
            {"symbol":"GC=F","shortname":"Gold","quoteType":"FUTURE","exchange":"CMX"}
        ]}"#;
        let hits = parse_yahoo_search(raw);
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].category, "gpw");
        assert_eq!(hits[1].category, "etf");
        assert_eq!(hits[2].category, "surowce");
    }

    #[test]
    fn urlencode_symboli() {
        assert_eq!(urlencode("GC=F"), "GC%3DF");
        assert_eq!(urlencode("NOVO-B.CO"), "NOVO-B.CO");
    }

    #[test]
    fn window_slice_tnie_do_okna_od_konca_serii() {
        let pts = vec![(0, 1.0), (100, 2.0), (200, 3.0), (300, 4.0)];
        // okno 150s od ostatniego punktu (300) → cut=150 → zostają t>=150
        let sliced = window_slice(&pts, 150);
        assert_eq!(sliced, vec![(200, 3.0), (300, 4.0)]);
    }

    #[test]
    fn window_slice_za_waskie_okno_zwraca_cala_serie() {
        let pts = vec![(0, 1.0), (300, 4.0)];
        // okno 10s zostawiłoby tylko 1 punkt → fallback do pełnej serii
        let sliced = window_slice(&pts, 10);
        assert_eq!(sliced, pts);
    }

    #[test]
    fn window_slice_pustej_serii_nie_panikuje() {
        assert_eq!(window_slice(&[], 100), Vec::<(i64, f64)>::new());
    }
}
