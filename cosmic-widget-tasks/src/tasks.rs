//! Skanowanie vaulta Obsidian pod kątem zadań wtyczki Tasks (`- [ ] tekst 📅
//! 2026-07-10 ...`), odtwarzające filtr "not done, due on or before today".
//! Statusy ("co znaczy zrobione") wczytywane z rzeczywistej konfiguracji
//! wtyczki (`data.json`), żeby niestandardowe statusy (np. numeryczne
//! priorytety-jako-statusy) działały tak samo jak w Obsidianie — nie z
//! zahardkodowanego domysłu.

use chrono::{Local, NaiveDate};
use cosmic_widget_common::vault_io::{self, Remote};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TaskItem {
    pub text: String,
    pub due: NaiveDate,
    pub overdue_days: i64,
    /// Symbol statusu z `- [x]` (np. ' ', '/', '1'..'5') — do sortowania
    /// dokładnie jak zapytanie użytkownika: `sort by function task.status.symbol
    /// === ' ' ? '9' : task.status.symbol`.
    pub status_symbol: char,
    /// Priorytet z emoji (🔺⏫🔼🔽⏬) — Tasks dokłada to jako kolejne
    /// kryterium sortowania automatycznie, nawet gdy zapytanie ma tylko
    /// jedno jawne "sort by" (potwierdzone na realnym vaultcie: w obrębie
    /// tego samego symbolu statusu zadania z 🔺 zawsze są nad zadaniami bez
    /// priorytetu, które z kolei są nad ⏬, niezależnie od terminu).
    pub priority: Priority,
    /// Pełna ścieżka pliku — do bezpośredniej edycji przy odhaczaniu.
    pub file_path: PathBuf,
    /// Numer linii (0-indeksowany) w pliku, w której znaleziono to zadanie.
    pub line_idx: usize,
    /// Surowa treść tej linii w chwili skanowania. Numer linii sam w sobie
    /// nie wystarcza: zapis idzie do kopii vaulta na serwerze, która mogła się
    /// w międzyczasie zmienić, więc przed odhaczeniem sprawdzamy, że pod tym
    /// numerem stoi dokładnie to zadanie, które użytkownik widział na ekranie.
    pub raw_line: String,
}

impl TaskItem {
    /// „Oczekujące” — czekam na klienta. użytkownik oznacza je statusem `[5]`
    /// (Najniższy); widget pokazuje je osobno, pod kreską (2026-10-01).
    pub fn is_waiting(&self) -> bool {
        self.status_symbol == '5'
    }
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
        } else if name_str.ends_with(".md") && !name_str.contains(".sync-conflict-") {
            // Kopie konfliktów Syncthinga to stare wersje notatek — ich zadania
            // pokazywałyby się jako duchy (np. wczorajszy Dziennik).
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

/// Usuwa tag `#zadanie` z tekstu do wyświetlenia w widgecie (inne tagi, np.
/// `#sprawozdanie`, zostają widoczne — mają swoją wartość informacyjną) —
/// tylko kosmetyczne, plik na dysku (i sam tag w Obsidianie) zostaje
/// nietknięty, bo to działa wyłącznie na kopii tekstu do pokazania, nie na
/// linii zapisywanej z powrotem do pliku.
fn strip_tags(text: &str) -> String {
    text.split_whitespace()
        .filter(|w| *w != "#zadanie")
        .collect::<Vec<_>>()
        .join(" ")
}

fn clean_text(rest: &str) -> String {
    let mut cut = rest.len();
    for m in METADATA_MARKERS {
        if let Some(idx) = rest.find(m) {
            cut = cut.min(idx);
        }
    }
    strip_tags(rest[..cut].trim())
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
    // Kolejność plików musi być deterministyczna i zgodna z tym, jak Obsidian
    // skanuje vault (alfabetycznie po ścieżce) — inaczej remisy w sortowaniu
    // niżej wypadają w innej kolejności niż w prawdziwym zapytaniu Tasks.
    files.sort();
    let today = Local::now().date_naive();

    let mut items = Vec::new();
    for file in &files {
        let Ok(content) = std::fs::read_to_string(file) else { continue };
        for (line_idx, line) in content.lines().enumerate() {
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
            items.push(TaskItem {
                text: clean_text(rest),
                due,
                overdue_days: (today - due).num_days(),
                status_symbol: symbol,
                priority: extract_priority(rest),
                file_path: file.clone(),
                line_idx,
                raw_line: line.to_string(),
            });
        }
    }

    // Sortowanie: jawne kryterium z zapytania użytkownika (symbol statusu, spacja
    // jak '9') jako klucz główny; w obrębie remisu Tasks **zawsze** dokłada
    // własne domyślne kryteria zanim sięgnie po kolejność skanowania —
    // zweryfikowane 1:1 na realnym vaultcie (patrz notatka pamięci): w
    // obrębie tego samego symbolu statusu priorytet-emoji (🔺 nad brakiem
    // priorytetu nad ⏬) bije termin, a dopiero przy remisie priorytetu
    // decyduje termin rosnąco (najbardziej zaległe u góry).
    // Oczekujące zawsze na sam koniec (sekcja pod kreską), także za Todo.
    items.sort_by(|a, b| {
        a.is_waiting()
            .cmp(&b.is_waiting())
            .then(sort_key(a.status_symbol).cmp(&sort_key(b.status_symbol)))
            .then(a.priority.cmp(&b.priority))
            .then(a.due.cmp(&b.due))
    });
    items.truncate(max_tasks);
    Ok(items)
}

/// Zamienia symbol w `- [<symbol>] tekst` na `x`, zachowując wcięcie i resztę
/// linii, i dopisuje `✅ YYYY-MM-DD` (konwencja wtyczki Tasks przy odhaczaniu),
/// o ile taki znacznik jeszcze nie występuje.
fn set_done(line: &str) -> Option<String> {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, trimmed) = line.split_at(indent_len);
    let after_dash = trimmed.strip_prefix("- [")?;
    let tail = after_dash.get(1..)?.strip_prefix(']')?;
    let mut new_line = format!("{indent}- [x]{tail}");
    if !new_line.contains('✅') {
        let today = Local::now().date_naive();
        new_line.push_str(&format!(" ✅ {}", today.format("%Y-%m-%d")));
    }
    Some(new_line)
}

/// Odhacza zadanie w danej linii pliku — używane po szybkim podwójnym
/// kliknięciu w widgecie (celowo bez pojedynczego kliknięcia, żeby uniknąć
/// przypadkowego odhaczenia).
pub fn mark_task_done(
    remote: Option<&Remote>,
    vault_path: &Path,
    file_path: &Path,
    line_idx: usize,
    raw_line: &str,
) -> std::io::Result<()> {
    let Some(content) = vault_io::read_before_write(remote, vault_path, file_path)? else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("nie ma pliku {}", file_path.display()),
        ));
    };
    let had_trailing_newline = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();

    // Trafiaj w zadanie po TREŚCI, nie po samym numerze linii — plik na
    // serwerze mógł się przesunąć, odkąd widget go zeskanował.
    let idx = match lines.get(line_idx) {
        Some(l) if l == raw_line => Some(line_idx),
        _ => lines.iter().position(|l| l == raw_line),
    };
    let Some(idx) = idx else {
        return Err(std::io::Error::other(
            "to zadanie zmieniło się w pliku (ktoś je już odhaczył albo notatka poszła dalej) — odśwież widget",
        ));
    };

    if let Some(new_line) = set_done(&lines[idx]) {
        lines[idx] = new_line;
    }
    let mut new_content = lines.join("\n");
    if had_trailing_newline {
        new_content.push('\n');
    }
    vault_io::write(remote, vault_path, file_path, &new_content)
}

// =========================================================================
// Sekcja "Do zapamiętania" (#pamięć) — bez terminu, sort po priorytecie.
// Współdzieli skan plików/parsowanie checkboxów z resztą tego modułu, ale ma
// inny filtr (tag zamiast terminu) i inny porządek sortowania.
// =========================================================================

const MEMORY_TAG: &str = "#pamięć";

/// Kolejność 1:1 z wtyczką Tasks: Highest -> ... -> Lowest, None pomiędzy
/// Medium i Low. Niższy rank (kolejność wariantów) = wyżej na liście.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Highest,
    High,
    Medium,
    None,
    Low,
    Lowest,
}

impl Priority {
    /// Jednoznakowy glif do pokazania zamiast daty (te elementy nie mają
    /// terminu, więc nie ma tu "X dni temu").
    pub fn glyph(self) -> &'static str {
        match self {
            Priority::Highest => "🔺",
            Priority::High => "⏫",
            Priority::Medium => "🔼",
            Priority::None => "—",
            Priority::Low => "🔽",
            Priority::Lowest => "⏬",
        }
    }
}

fn extract_priority(rest: &str) -> Priority {
    if rest.contains('🔺') {
        Priority::Highest
    } else if rest.contains('⏫') {
        Priority::High
    } else if rest.contains('🔼') {
        Priority::Medium
    } else if rest.contains('🔽') {
        Priority::Low
    } else if rest.contains('⏬') {
        Priority::Lowest
    } else {
        Priority::None
    }
}

/// "tags include #pamięć" wtyczki Tasks = `#pamięć` jest właściwym tagiem
/// linii (całe słowo, ew. `#pamięć/coś` dla tagów zagnieżdżonych) — nie samo
/// wystąpienie podciągu (łapało by np. prozę wspominającą `##pamięć` jako
/// przykład składni).
fn has_memory_tag(rest: &str) -> bool {
    rest.split_whitespace()
        .any(|w| w == MEMORY_TAG || w.starts_with(&format!("{MEMORY_TAG}/")))
}

fn clean_memory_text(rest: &str) -> String {
    let mut cut = rest.len();
    for m in METADATA_MARKERS {
        if let Some(idx) = rest.find(m) {
            cut = cut.min(idx);
        }
    }
    rest[..cut]
        .trim()
        .split_whitespace()
        .filter(|w| *w != MEMORY_TAG)
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone)]
pub struct MemoryItem {
    pub text: String,
    pub priority: Priority,
    pub file_path: PathBuf,
    pub line_idx: usize,
    /// Jak `TaskItem::raw_line` — kotwica treści na potrzeby zapisu.
    pub raw_line: String,
}

pub fn scan_memory_items(vault_path: &Path, max_items: usize) -> Result<Vec<MemoryItem>, String> {
    if !vault_path.is_dir() {
        return Err(format!("brak folderu vaulta: {}", vault_path.display()));
    }
    let status_types = load_status_types(vault_path);
    let mut files = Vec::new();
    collect_md_files(vault_path, &mut files);

    let mut items = Vec::new();
    for file in &files {
        let Ok(content) = std::fs::read_to_string(file) else { continue };
        for (line_idx, line) in content.lines().enumerate() {
            let Some((symbol, rest)) = parse_checkbox(line) else { continue };
            if !has_memory_tag(rest) {
                continue;
            }
            let status_type = status_types.get(&symbol).map(String::as_str).unwrap_or("TODO");
            if status_type == "DONE" || status_type == "CANCELLED" {
                continue;
            }
            items.push(MemoryItem {
                text: clean_memory_text(rest),
                priority: extract_priority(rest),
                file_path: file.clone(),
                line_idx,
                raw_line: line.to_string(),
            });
        }
    }

    items.sort_by_key(|t| t.priority);
    items.truncate(max_items);
    Ok(items)
}

/// Nagłówek sekcji w notatce dziennej, pod którym lądują wpisy `#pamięć` —
/// jest już w szablonie `Kalendarz/Szablony/Szablon Dnia.md`.
const MEMORY_HEADING: &str = "### 🧠 Pamięć";

/// Dopisuje `- [ ] #pamięć <tekst>` do notatki **dzisiejszego** dnia, zaraz
/// pod nagłówkiem [`MEMORY_HEADING`]. Jeśli notatka dnia jeszcze nie
/// istnieje, tworzy ją z szablonu (ten sam wzorzec co `Vault::ensure_note`
/// w [[cosmic_calendar_widget_obsidian]]); jeśli nagłówek nie występuje w
/// notatce (np. ręcznie utworzona, bez szablonu), dopisuje go na końcu.
pub fn append_memory_item(
    remote: Option<&Remote>,
    vault_path: &Path,
    daily_notes_folder: &str,
    daily_note_template: &str,
    text: &str,
) -> std::io::Result<()> {
    let date_str = Local::now().date_naive().format("%Y-%m-%d").to_string();
    let note_path = vault_path.join(daily_notes_folder).join(format!("{date_str}.md"));

    // Notatkę dnia bierzemy z tej kopii, do której będziemy pisać — inaczej
    // dopisalibyśmy wpis do lokalnej (może starszej) wersji i nadpisali nią
    // to, co użytkownik dodał z drugiego komputera.
    let existing = vault_io::read_before_write(remote, vault_path, &note_path)?;
    let mut content = if let Some(existing) = existing {
        existing
    } else if daily_note_template.trim().is_empty() {
        String::new()
    } else {
        let template_path = vault_path.join(format!("{daily_note_template}.md"));
        std::fs::read_to_string(template_path)
            .unwrap_or_default()
            .replace("{{date}}", &date_str)
            .replace("{{title}}", &date_str)
    };

    let new_line = format!("- [ ] {MEMORY_TAG} {text}");
    match content.find(MEMORY_HEADING) {
        Some(idx) => {
            let after_heading = idx + MEMORY_HEADING.len();
            let insert_at = content[after_heading..]
                .find('\n')
                .map(|n| after_heading + n + 1)
                .unwrap_or(content.len());
            content.insert_str(insert_at, &format!("{new_line}\n"));
        }
        None => {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(&format!("{MEMORY_HEADING}\n{new_line}\n"));
        }
    }

    vault_io::write(remote, vault_path, &note_path, &content)
}
