//! Formularze ustawień widżetów: najważniejsze klucze config.toml.
//! Zapis przez toml_edit — komentarze i pozostałe klucze zostają w pliku.
//! Nazwy kluczy i wartości domyślne pochodzą z `src/config.rs` każdego widżetu.

use crate::catalog::config_path;
use crate::i18n::t;
use std::fs;
use toml_edit::{DocumentMut, Item, value};

pub const ANCHORS: [&str; 4] = ["top-left", "top-right", "bottom-left", "bottom-right"];
pub const ANCHORS_PL: [&str; 4] = ["lewy górny", "prawy górny", "lewy dolny", "prawy dolny"];
pub const ANCHORS_EN: [&str; 4] = ["top left", "top right", "bottom left", "bottom right"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Anchor,
    Int,
    Float,
    Text,
    Bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub key: &'static str,
    pub kind: Kind,
    /// Wartość domyślna z config.rs widżetu (jako tekst).
    pub default: &'static str,
    pub pl: &'static str,
    pub en: &'static str,
}

impl Field {
    pub fn label(&self) -> &'static str {
        t(self.pl, self.en)
    }
}

const fn f(key: &'static str, kind: Kind, default: &'static str, pl: &'static str, en: &'static str) -> Field {
    Field { key, kind, default, pl, en }
}

fn anchor(d: &'static str) -> Field { f("anchor", Kind::Anchor, d, "Róg ekranu", "Screen corner") }
fn mx(d: &'static str) -> Field { f("margin_x", Kind::Int, d, "Odstęp poziomy (px)", "Horizontal margin (px)") }
fn my(d: &'static str) -> Field { f("margin_y", Kind::Int, d, "Odstęp pionowy (px)", "Vertical margin (px)") }
fn width(d: &'static str) -> Field { f("width", Kind::Int, d, "Szerokość (px)", "Width (px)") }
fn refresh(d: &'static str) -> Field { f("refresh_secs", Kind::Int, d, "Odświeżanie (sekundy)", "Refresh interval (seconds)") }

/// Pola formularza dla widżetu `bin`.
pub fn fields(bin: &str) -> Vec<Field> {
    match bin {
        "cosmic-calendar-widget" => vec![
            anchor("top-right"), mx("20"), my("150"), width("330"),
            f("open_in_obsidian", Kind::Bool, "true", "Otwieraj notatki w Obsidianie", "Open notes in Obsidian"),
        ],
        "cosmic-widget-tasks" => vec![
            anchor("top-right"), mx("370"), my("20"), width("460"), refresh("90"),
            f("max_tasks", Kind::Int, "20", "Maks. liczba zadań", "Max tasks shown"),
            f("daily_notes_folder", Kind::Text, "Kalendarz", "Folder notatek dziennych", "Daily notes folder"),
        ],
        "cosmic-widget-note" => vec![
            anchor("top-left"), mx("22"), my("330"), width("330"),
            f("height", Kind::Int, "300", "Wysokość (px)", "Height (px)"),
            f("title", Kind::Text, "Notatka", "Tytuł karteczki", "Note title"),
            f("note_file", Kind::Text, "Notatnik.md", "Plik notatki w vaulcie", "Note file in the vault"),
            f("font_size", Kind::Int, "14", "Rozmiar czcionki", "Font size"),
        ],
        "cosmic-widget-drawer" => vec![
            width("420"),
            f("margin", Kind::Int, "10", "Odstęp od krawędzi (px)", "Edge margin (px)"),
            refresh("120"),
        ],
        "cosmic-widget-agenda" => vec![
            f("ics_url", Kind::Text, "", "Adres prywatnego kalendarza ICS", "Private calendar ICS URL"),
            anchor("top-right"), mx("20"), my("600"), width("330"), refresh("600"),
            f("days_ahead", Kind::Int, "14", "Ile dni do przodu", "Days ahead"),
            f("max_events", Kind::Int, "30", "Maks. liczba wydarzeń", "Max events"),
        ],
        "cosmic-widget-weather" => vec![
            f("lat", Kind::Float, "51.2465", "Szerokość geograficzna", "Latitude"),
            f("lon", Kind::Float, "22.5684", "Długość geograficzna", "Longitude"),
            f("imgw_station", Kind::Text, "Lublin", "Stacja IMGW", "IMGW station"),
            anchor("top-left"), mx("20"), my("20"), width("330"), refresh("600"),
            f("forecast_days", Kind::Int, "4", "Dni prognozy", "Forecast days"),
        ],
        "cosmic-widget-markets" => vec![anchor("top-right"), mx("370"), my("580"), width("330"), refresh("120")],
        "cosmic-widget-panel" => vec![
            f("api_url", Kind::Text, "https://your-panel.example.com/api/widget-status", "Adres API (JSON)", "API URL (JSON)"),
            f("panel_url", Kind::Text, "https://your-panel.example.com/panel/", "Adres panelu (klik)", "Panel URL (on click)"),
            f("api_key", Kind::Text, "", "Klucz API", "API key"),
            anchor("top-right"), mx("20"), my("20"), width("330"), refresh("120"),
        ],
        _ => vec![],
    }
}

fn item_to_string(it: &Item) -> Option<String> {
    let v = it.as_value()?;
    if let Some(s) = v.as_str() { return Some(s.to_owned()); }
    if let Some(i) = v.as_integer() { return Some(i.to_string()); }
    if let Some(x) = v.as_float() { return Some(x.to_string()); }
    if let Some(b) = v.as_bool() { return Some(b.to_string()); }
    None
}

fn read_doc(bin: &str) -> Result<DocumentMut, String> {
    let cfg = config_path(bin);
    match fs::read_to_string(&cfg) {
        Ok(text) => text.parse().map_err(|e| format!("{}: {e}", cfg.display())),
        Err(_) => Ok(DocumentMut::new()),
    }
}

/// Aktualne wartości pól (z configu albo domyślne z config.rs).
pub fn load(bin: &str) -> Vec<String> {
    let doc = read_doc(bin).unwrap_or_default();
    fields(bin)
        .iter()
        .map(|fl| doc.get(fl.key).and_then(item_to_string).unwrap_or_else(|| fl.default.to_owned()))
        .collect()
}

/// Zapisuje wartości. Klucze nieobecne w pliku i równe domyślnym nie są dopisywane.
/// Zwraca liczbę zmienionych kluczy.
pub fn save(bin: &str, values: &[String]) -> Result<usize, String> {
    let fl = fields(bin);
    if fl.len() != values.len() {
        return Err("internal: field count mismatch".into());
    }
    let mut doc = read_doc(bin)?;
    let mut changed = 0;
    for (fld, raw) in fl.iter().zip(values) {
        let raw = raw.trim();
        let cur = doc.get(fld.key).and_then(item_to_string);
        if cur.is_none() && raw == fld.default { continue; }
        if cur.as_deref() == Some(raw) { continue; }
        let bad = || format!("{}: «{raw}»", fld.label());
        match fld.kind {
            Kind::Anchor => {
                if !ANCHORS.contains(&raw) { return Err(bad()); }
                doc[fld.key] = value(raw);
            }
            Kind::Int => doc[fld.key] = value(raw.parse::<i64>().map_err(|_| bad())?),
            Kind::Float => doc[fld.key] = value(raw.parse::<f64>().map_err(|_| bad())?),
            Kind::Bool => doc[fld.key] = value(raw.parse::<bool>().map_err(|_| bad())?),
            Kind::Text => doc[fld.key] = value(raw),
        }
        changed += 1;
    }
    if changed > 0 {
        let cfg = config_path(bin);
        if let Some(dir) = cfg.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        fs::write(&cfg, doc.to_string()).map_err(|e| format!("{}: {e}", cfg.display()))?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::WIDGETS;

    #[test]
    fn kazdy_widzet_ma_pola_i_unikalne_klucze() {
        for w in WIDGETS {
            let fl = fields(w.bin);
            assert!(!fl.is_empty(), "{}", w.bin);
            let mut keys: Vec<_> = fl.iter().map(|x| x.key).collect();
            keys.sort();
            keys.dedup();
            assert_eq!(keys.len(), fl.len(), "{}", w.bin);
        }
    }

    #[test]
    fn zapis_zachowuje_komentarze_i_reszte() {
        let _g = crate::catalog::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let base = std::env::temp_dir().join(format!("cwm-set-{}", std::process::id()));
        // SAFETY: tylko ten test w tym module ustawia XDG_CONFIG_HOME (testy obsidian.rs robią to samo ze swoją ścieżką)
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &base) };
        let bin = "cosmic-widget-agenda";
        let cfg = config_path(bin);
        fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        fs::write(&cfg, "# mój komentarz\nics_url = \"\"\nanchor = \"top-left\"\nglass = \"off\" # szkło\n").unwrap();

        // brak pliku/kluczy -> domyślne
        let mut v = load(bin);
        let names: Vec<_> = fields(bin).iter().map(|x| x.key).collect();
        let idx = |k: &str| names.iter().position(|n| *n == k).unwrap();
        assert_eq!(v[idx("anchor")], "top-left");
        assert_eq!(v[idx("margin_y")], "600");
        assert_eq!(v[idx("days_ahead")], "14");

        v[idx("ics_url")] = "https://example.com/x.ics".into();
        v[idx("anchor")] = "bottom-right".into();
        v[idx("margin_x")] = "55".into();
        assert_eq!(save(bin, &v).unwrap(), 3);

        let t = fs::read_to_string(&cfg).unwrap();
        assert!(t.contains("# mój komentarz"));
        assert!(t.contains("glass = \"off\" # szkło"));
        assert!(t.contains("ics_url = \"https://example.com/x.ics\""));
        assert!(t.contains("anchor = \"bottom-right\""));
        assert!(t.contains("margin_x = 55"));
        assert!(!t.contains("margin_y")); // wartość domyślna nie jest dopisywana
        assert_eq!(load(bin)[idx("margin_x")], "55");

        // walidacja
        v[idx("days_ahead")] = "abc".into();
        assert!(save(bin, &v).is_err());
        v[idx("days_ahead")] = "14".into();
        v[idx("anchor")] = "middle".into();
        assert!(save(bin, &v).is_err());

        // pogoda: float i brak pliku
        let wbin = "cosmic-widget-weather";
        let mut wv = load(wbin);
        assert_eq!(wv[0], "51.2465");
        wv[0] = "52.5".into();
        assert_eq!(save(wbin, &wv).unwrap(), 1);
        assert!(fs::read_to_string(config_path(wbin)).unwrap().contains("lat = 52.5"));
        // bez zmian nie zapisuje
        assert_eq!(save(wbin, &load(wbin)).unwrap(), 0);
        let _ = fs::remove_dir_all(&base);
    }
}
