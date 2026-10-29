//! Generuje statyczny HTML z pełnym wykresem (TradingView Lightweight
//! Charts, wendorowane offline) dla klikniętego aktywa. Otwierany w
//! domyślnej przeglądarce zamiast rysowania mini-wykresu w widgecie.

pub struct ChartSeries {
    pub label: &'static str,
    pub points: Vec<(i64, f64)>,
}

const TEMPLATE: &str = include_str!("../assets/chart_template.html");
const LIGHTWEIGHT_CHARTS_JS: &str = include_str!("../assets/lightweight-charts.js");

pub const UP_HEX: &str = "#4dd973";
pub const DOWN_HEX: &str = "#f26666";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ChartPayload<'a> {
    asset_name: &'a str,
    symbol: &'a str,
    currency: &'a str,
    series: Vec<SeriesPayload<'a>>,
}

#[derive(serde::Serialize)]
struct SeriesPayload<'a> {
    label: &'a str,
    points: &'a [(i64, f64)],
}

/// JSON osadzany w `<script>` strony. `</` → `<\/` zabezpiecza przed
/// przedwczesnym zamknięciem tagu, gdyby nazwa aktywa (z zewnętrznego API
/// / wyszukiwarki) zawierała taki ciąg — `\/` to poprawny escape w JSON,
/// więc wynik nadal parsuje się bez zmian.
fn build_json(asset_name: &str, symbol: &str, currency: &str, series: &[ChartSeries]) -> String {
    let payload = ChartPayload {
        asset_name,
        symbol,
        currency,
        series: series
            .iter()
            .map(|s| SeriesPayload { label: s.label, points: &s.points })
            .collect(),
    };
    let json = serde_json::to_string(&payload).unwrap_or_else(|_| r#"{"series":[]}"#.to_string());
    json.replace("</", "<\\/")
}

/// Buduje samodzielną stronę HTML z wbudowaną biblioteką i danymi —
/// zapisywana na dysk i otwierana w przeglądarce (patrz `app.rs`).
pub fn render(asset_name: &str, symbol: &str, currency: &str, series: &[ChartSeries]) -> String {
    let json_safe = build_json(asset_name, symbol, currency, series);
    // Dzielimy szablon RAZ wokół __CHART_JSON__ i podstawiamy pozostałe
    // placeholdery tylko w statycznych połówkach — json_safe (dane z
    // zewnętrznych API) trafia do wyniku bez ponownego skanowania przez
    // .replace(), więc nie może przypadkiem trafić we wzorzec kolejnego
    // placeholdera (np. gdyby nazwa aktywa zawierała "__UP_COLOR__").
    let (before, after) = TEMPLATE
        .split_once("__CHART_JSON__")
        .expect("szablon musi zawierać __CHART_JSON__");
    let fill = |s: &str| {
        s.replace("__LWC_LIB__", LIGHTWEIGHT_CHARTS_JS)
            .replace("__UP_COLOR__", UP_HEX)
            .replace("__DOWN_COLOR__", DOWN_HEX)
    };
    format!("{}{}{}", fill(before), json_safe, fill(after))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_json_parsuje_sie_i_zawiera_dane() {
        let series = vec![ChartSeries { label: "24h", points: vec![(1000, 10.5), (2000, 11.0)] }];
        let json = build_json("Bitcoin", "BTC", "USD", &series);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["assetName"], "Bitcoin");
        assert_eq!(v["symbol"], "BTC");
        assert_eq!(v["series"][0]["label"], "24h");
        assert_eq!(v["series"][0]["points"][1][1], 11.0);
    }

    #[test]
    fn build_json_ucieka_zamkniecie_scriptu() {
        let series = vec![ChartSeries { label: "24h", points: vec![(1, 1.0), (2, 2.0)] }];
        let json = build_json("</script><script>alert(1)</script>", "X", "USD", &series);
        assert!(!json.contains("</script>"));
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["assetName"], "</script><script>alert(1)</script>");
    }

    #[test]
    fn render_osadza_biblioteke_i_dane_bez_placeholderow() {
        let series = vec![ChartSeries { label: "24h", points: vec![(1, 1.0), (2, 2.0)] }];
        let html = render("Bitcoin", "BTC", "USD", &series);
        assert!(html.contains("LightweightCharts"));
        assert!(html.contains("\"assetName\":\"Bitcoin\""));
        assert!(!html.contains("__LWC_LIB__"));
        assert!(!html.contains("__CHART_JSON__"));
        assert!(!html.contains("__UP_COLOR__"));
        assert!(!html.contains("__DOWN_COLOR__"));
    }

    #[test]
    fn render_pustej_historii_nie_panikuje() {
        let html = render("Nic", "N", "USD", &[]);
        assert!(html.contains("\"series\":[]"));
    }

    #[test]
    fn render_nie_koliduje_z_placeholderami_w_danych_zewnetrznych() {
        let series = vec![ChartSeries { label: "24h", points: vec![(1, 1.0), (2, 2.0)] }];
        let html = render("__UP_COLOR__ token", "X", "USD", &series);
        assert!(html.contains("\"assetName\":\"__UP_COLOR__ token\""));
    }

    #[test]
    fn wendorowana_biblioteka_nie_koliduje_z_placeholderami_szablonu() {
        for token in ["__LWC_LIB__", "__CHART_JSON__", "__UP_COLOR__", "__DOWN_COLOR__"] {
            assert!(
                !LIGHTWEIGHT_CHARTS_JS.contains(token),
                "wendorowana biblioteka zawiera token szablonu {token} — podmiana upgrade'u biblioteki nadpisze/skoryguje ten fragment"
            );
        }
    }
}
