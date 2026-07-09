//! Pobieranie danych pogodowych z dwóch niezależnych, darmowych API bez
//! kluczy: aktualny odczyt z prawdziwej stacji IMGW w Lublinie (dane
//! publiczne `danepubliczne.imgw.pl`) oraz krótka prognoza z Open-Meteo
//! liczona dla współrzędnych Lublina (modele ECMWF/ICON, bez stacji).
//! Obie części pobierane równolegle; awaria jednej nie blokuje drugiej
//! (patrz `WeatherData::warning`).

use chrono::NaiveDate;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct CurrentWeather {
    pub temperature: f64,
    pub wind_speed: f64,
    pub wind_dir_deg: f64,
    pub humidity: f64,
    pub pressure: f64,
    pub precip_sum: f64,
    /// Godzina pomiaru wg IMGW (np. "15"), czas lokalny.
    pub measured_hour: String,
}

#[derive(Debug, Clone)]
pub struct ForecastDay {
    pub date: NaiveDate,
    pub code: i64,
    pub tmax: f64,
    pub tmin: f64,
    pub precip_prob: f64,
}

#[derive(Debug, Clone)]
pub struct WeatherData {
    pub current: Option<CurrentWeather>,
    pub forecast: Vec<ForecastDay>,
    /// Błąd częściowy (np. padło tylko jedno z dwóch API) — pokazywany,
    /// ale nie blokuje wyświetlenia tego co się udało pobrać.
    pub warning: Option<String>,
}

pub async fn fetch_weather(
    lat: f64,
    lon: f64,
    station: String,
    forecast_days: u32,
) -> Result<WeatherData, String> {
    let (cur_res, fc_res) = tokio::join!(fetch_current(station), fetch_forecast(lat, lon, forecast_days));

    let mut warnings = Vec::new();
    let current = match cur_res {
        Ok(c) => Some(c),
        Err(e) => {
            warnings.push(format!("stacja IMGW: {e}"));
            None
        }
    };
    let forecast = match fc_res {
        Ok(f) => f,
        Err(e) => {
            warnings.push(format!("prognoza: {e}"));
            Vec::new()
        }
    };

    if current.is_none() && forecast.is_empty() {
        return Err(warnings.join("; "));
    }

    Ok(WeatherData {
        current,
        forecast,
        warning: if warnings.is_empty() { None } else { Some(warnings.join("; ")) },
    })
}

// =========================================================================
// IMGW — aktualny odczyt z prawdziwej stacji synoptycznej
// =========================================================================

async fn fetch_current(station: String) -> Result<CurrentWeather, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get("https://danepubliczne.imgw.pl/api/data/synop")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let stations: Vec<serde_json::Value> = resp.json().await.map_err(|e| e.to_string())?;

    let needle = station.to_lowercase();
    let entry = stations
        .iter()
        .find(|s| {
            s.get("stacja")
                .and_then(|v| v.as_str())
                .map(|n| n.to_lowercase() == needle)
                .unwrap_or(false)
        })
        .ok_or_else(|| format!("brak stacji '{station}' w odpowiedzi IMGW"))?;

    let parse_f64 = |key: &str| -> Option<f64> {
        entry.get(key).and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok())
    };

    Ok(CurrentWeather {
        temperature: parse_f64("temperatura").ok_or_else(|| "brak temperatury w odpowiedzi".to_string())?,
        wind_speed: parse_f64("predkosc_wiatru").unwrap_or(0.0),
        wind_dir_deg: parse_f64("kierunek_wiatru").unwrap_or(0.0),
        humidity: parse_f64("wilgotnosc_wzgledna").unwrap_or(0.0),
        pressure: parse_f64("cisnienie").unwrap_or(0.0),
        precip_sum: parse_f64("suma_opadu").unwrap_or(0.0),
        measured_hour: entry
            .get("godzina_pomiaru")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

// =========================================================================
// Open-Meteo — krótka prognoza dla współrzędnych Lublina
// =========================================================================

#[derive(Debug, Deserialize)]
struct OpenMeteoResponse {
    daily: DailyBlock,
}

#[derive(Debug, Deserialize)]
struct DailyBlock {
    time: Vec<String>,
    weather_code: Vec<i64>,
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
    precipitation_probability_max: Vec<f64>,
}

async fn fetch_forecast(lat: f64, lon: f64, days: u32) -> Result<Vec<ForecastDay>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max&timezone=Europe%2FWarsaw&forecast_days={days}"
    );
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let parsed: OpenMeteoResponse = resp.json().await.map_err(|e| e.to_string())?;
    let d = parsed.daily;
    let n = d.time.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let date = NaiveDate::parse_from_str(&d.time[i], "%Y-%m-%d").map_err(|e| e.to_string())?;
        out.push(ForecastDay {
            date,
            code: *d.weather_code.get(i).unwrap_or(&0),
            tmax: *d.temperature_2m_max.get(i).unwrap_or(&0.0),
            tmin: *d.temperature_2m_min.get(i).unwrap_or(&0.0),
            precip_prob: *d.precipitation_probability_max.get(i).unwrap_or(&0.0),
        });
    }
    Ok(out)
}

// =========================================================================
// Pomocnicze — opis WMO i kierunek wiatru po polsku
// =========================================================================

pub fn wmo_desc(code: i64) -> &'static str {
    match code {
        0 => "Bezchmurnie",
        1 | 2 => "Częściowe zachmurzenie",
        3 => "Pochmurno",
        45 | 48 => "Mgła",
        51 | 53 | 55 => "Mżawka",
        56 | 57 => "Marznąca mżawka",
        61 | 63 | 65 => "Deszcz",
        66 | 67 => "Marznący deszcz",
        71 | 73 | 75 | 77 => "Śnieg",
        80 | 81 | 82 => "Przelotne opady",
        85 | 86 => "Przelotny śnieg",
        95 => "Burza",
        96 | 99 => "Burza z gradem",
        _ => "Zmiennie",
    }
}

pub fn wind_compass(deg: f64) -> &'static str {
    const DIRS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let idx = (((deg % 360.0) + 22.5) / 45.0) as usize % 8;
    DIRS[idx]
}
