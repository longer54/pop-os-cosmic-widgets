//! Klient endpointu `GET /api/widget-status` (klucz w nagłówku X-Api-Key).

use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
pub struct StatusResponse {
    /// host -> stan; BTreeMap daje stałą kolejność hostów w UI
    pub hosts: BTreeMap<String, HostStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HostStatus {
    pub services: Vec<Service>,
    pub metrics: Option<Metrics>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Service {
    pub service: String,
    pub status: String,
    #[allow(dead_code)]
    pub last_check: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Metrics {
    pub ram_pct: Option<f64>,
    pub hdd_pct: Option<f64>,
    pub temp_c: Option<f64>,
}

pub async fn fetch_status(url: String, api_key: String) -> Result<StatusResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&url)
        .header("X-Api-Key", api_key)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }

    resp.json::<StatusResponse>().await.map_err(|e| e.to_string())
}
