//! Integracja z vaultem Obsidian: ścieżki dziennych notatek, liczenie
//! "kropek" pod dniem (jak wtyczka Calendar) i otwieranie/tworzenie notatek
//! przez `obsidian://` URI.

use std::path::PathBuf;

use jiff::civil::Date;

use crate::config::Config;

pub struct Vault {
    path: PathBuf,
    name: String,
    daily_folder: String,
    template_path: Option<PathBuf>,
    words_per_dot: u32,
    max_dots: u32,
}

impl Vault {
    pub fn from_config(config: &Config) -> Self {
        let template_path = if config.daily_note_template.trim().is_empty() {
            None
        } else {
            Some(config.vault_path.join(format!("{}.md", config.daily_note_template)))
        };

        Vault {
            path: config.vault_path.clone(),
            name: config.vault_name.clone(),
            daily_folder: config.daily_notes_folder.clone(),
            template_path,
            words_per_dot: config.words_per_dot,
            max_dots: config.max_dots,
        }
    }

    pub fn note_path(&self, date: Date) -> PathBuf {
        self.path.join(&self.daily_folder).join(format!("{date}.md"))
    }

    /// Liczba kropek do pokazania pod danym dniem — 0 gdy notatka nie
    /// istnieje, w przeciwnym razie `clamp(words / words_per_dot, 1, max)`,
    /// dokładnie jak `getWordLengthAsDots` we wtyczce Calendar (nawet pusta
    /// istniejąca notatka dostaje 1 kropkę).
    pub fn dots_for(&self, date: Date) -> u32 {
        let Ok(text) = std::fs::read_to_string(self.note_path(date)) else {
            return 0;
        };
        if self.words_per_dot == 0 {
            return 1;
        }
        let words = text.split_whitespace().count() as u32;
        (words / self.words_per_dot).clamp(1, self.max_dots)
    }

    /// Tworzy notatkę dnia z szablonu, jeśli jeszcze nie istnieje.
    pub fn ensure_note(&self, date: Date) -> std::io::Result<()> {
        let path = self.note_path(date);
        if path.exists() {
            return Ok(());
        }

        let date_str = date.to_string();
        let content = match &self.template_path {
            Some(tp) => std::fs::read_to_string(tp)
                .unwrap_or_default()
                .replace("{{date}}", &date_str)
                .replace("{{title}}", &date_str),
            None => String::new(),
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)
    }

    /// `obsidian://open?vault=...&file=...` dla danego dnia.
    pub fn open_uri(&self, date: Date) -> String {
        let rel = format!("{}/{date}", self.daily_folder);
        format!(
            "obsidian://open?vault={}&file={}",
            percent_encode(&self.name),
            percent_encode(&rel)
        )
    }
}

/// Minimalny percent-encoding — wystarczający dla ścieżek vaulta (litery,
/// cyfry, `/`, spacje, polskie znaki).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
