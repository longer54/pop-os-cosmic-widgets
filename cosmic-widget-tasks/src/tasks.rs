//! Skanowanie vaulta Obsidian pod kątem zadań wtyczki Tasks (`- [ ] tekst 📅
//! 2026-07-10 ...`), odtwarzające filtr "not done, due on or before today".
//! Statusy ("co znaczy zrobione") wczytywane z rzeczywistej konfiguracji
//! wtyczki (`data.json`), żeby niestandardowe statusy (np. numeryczne
//! priorytety-jako-statusy) działały tak samo jak w Obsidianie — nie z
//! zahardkodowanego domysłu.

use chrono::{Local, NaiveDate};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TaskItem {
    pub text: String,
    pub due: NaiveDate,
    pub overdue_days: i64,
    /// Symbol statusu z `- [x]` (np. ' ', '/', '1'..'5') — do sortowania
    /// jak w zapytaniu Tasks: `sort by function task.status.symbol === ' '
    /// ? '9' : task.status.symbol` (spacja/Todo sortuje się na końcu).
    pub status_symbol: char,
    /// Ścieżka względem vaulta, bez rozszerzenia — do `obsidian://open`.
    pub file_rel: String,
}

/// Odwzorowanie 1:1 sortowania z zapytania Tasks: spacja (Todo) sortuje się
/// tak jakby miała symbol '9' (czyli na końcu, po '1'..'5'), pozostałe
/// symbole porównywane wprost.
fn sort_key(symbol: char) -> char {
    if symbol == ' ' {
        '9'
    } else {
        symbol
    }
}

const SKIP_DIRS: &[&str] = &["node_modules", "venv", "target", "__pycache__", "dist", "build"];

fn collect_md_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if path.is_dir() {
            if name_str.starts_with('.') || SKIP_DIRS.contains(&name_str.as_ref()) {
                continue;
            }
            collect_md_files(&path, out);
        } else if name_str.ends_with(".md") {
            out.push(path);
        }
    }
}

#[derive(Deserialize, Default)]
struct StatusEntry {
    symbol: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StatusSettings {
    #[serde(default)]
    core_statuses: Vec<StatusEntry>,
    #[serde(default)]
    custom_statuses: Vec<StatusEntry>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PluginData {
    #[serde(default)]
    status_settings: Option<StatusSettings>,
}

/// symbol -> typ statusu ("DONE", "CANCELLED", "TODO", "IN_PROGRESS", ...).
/// Domyślne wpisy na wypadek braku/uszkodzenia configu wtyczki.
fn load_status_types(vault_path: &Path) -> HashMap<char, String> {
    let mut map = HashMap::new();
    map.insert('x', "DONE".to_string());
    map.insert('X', "DONE".to_string());
    map.insert('-', "CANCELLED".to_string());

    let path = vault_path
        .join(".obsidian/plugins/obsidian-tasks-plugin/data.json");
    let Ok(raw) = std::fs::read_to_string(path) else { return map };
    let Ok(data) = serde_json::from_str::<PluginData>(&raw) else { return map };
    if let Some(ss) = data.status_settings {
        for entry in ss.core_statuses.into_iter().chain(ss.custom_statuses) {
            if let Some(c) = entry.symbol.chars().next() {
                map.insert(c, entry.kind);
            }
        }
    }
    map
}

/// `- [x] tekst` -> (`x`, "tekst"); None jeśli linia nie jest checkboxem.
fn parse_checkbox(line: &str) -> Option<(char, &str)> {
    let trimmed = line.trim_start();
    let after_dash = trimmed.strip_prefix("- [")?;
    let mut chars = after_dash.chars();
    let symbol = chars.next()?;
    let rest = chars.as_str().strip_prefix(']')?;
    Some((symbol, rest.strip_prefix(' ').unwrap_or(rest)))
}

const METADATA_MARKERS: &[&str] = &[
    "📅", "⏳", "➕", "🛫", "✅", "🔺", "⏫", "🔼", "🔽", "⏬", "🔁", "🆔", "⛔", "🔗",
];

fn clean_text(rest: &str) -> String {
    let mut cut = rest.len();
    for m in METADATA_MARKERS {
        if let Some(idx) = rest.find(m) {
            cut = cut.min(idx);
        }
    }
    rest[..cut].trim().to_string()
}

fn extract_due(rest: &str) -> Option<NaiveDate> {
    let idx = rest.find('📅')?;
    let after = rest[idx + '📅'.len_utf8()..].trim_start();
    let date_str: String = after.chars().take(10).collect();
    NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()
}

pub fn scan_tasks(vault_path: &Path, max_tasks: usize) -> Result<Vec<TaskItem>, String> {
    if !vault_path.is_dir() {
        return Err(format!("brak folderu vaulta: {}", vault_path.display()));
    }
    let status_types = load_status_types(vault_path);
    let mut files = Vec::new();
    collect_md_files(vault_path, &mut files);
    let today = Local::now().date_naive();

    let mut items = Vec::new();
    for file in &files {
        let Ok(content) = std::fs::read_to_string(file) else { continue };
        for line in content.lines() {
            let Some((symbol, rest)) = parse_checkbox(line) else { continue };
            let status_type = status_types.get(&symbol).map(String::as_str).unwrap_or("TODO");
            // Tasks plugin traktuje Cancelled tak samo jak Done przy filtrze
            // "not done" (potwierdzone na realnym vaultcie: "- [-] ... ❌" nie
            // pojawia się w zapytaniu Tasks mimo że typ statusu to CANCELLED,
            // nie DONE).
            if status_type == "DONE" || status_type == "CANCELLED" {
                continue;
            }
            let Some(due) = extract_due(rest) else { continue };
            if due > today {
                continue;
            }
            let Ok(rel) = file.strip_prefix(vault_path) else { continue };
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let file_rel = rel_str.strip_suffix(".md").unwrap_or(&rel_str).to_string();
            items.push(TaskItem {
                text: clean_text(rest),
                due,
                overdue_days: (today - due).num_days(),
                status_symbol: symbol,
                file_rel,
            });
        }
    }

    // Sortowanie 1:1 z zapytaniem Tasks: po symbolu statusu (spacja jak '9'),
    // dogrywka po terminie żeby kolejność w obrębie tego samego symbolu była
    // deterministyczna.
    items.sort_by(|a, b| sort_key(a.status_symbol).cmp(&sort_key(b.status_symbol)).then(a.due.cmp(&b.due)));
    items.truncate(max_tasks);
    Ok(items)
}

