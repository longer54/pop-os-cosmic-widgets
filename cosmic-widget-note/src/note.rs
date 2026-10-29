//! Notatka w vaultcie: odczyt lokalny, zapis przez kanoniczną kopię na
//! serwerze (SSH) — dokładnie ta sama zasada co w widgetcie zadań, patrz
//! `cosmic_widget_common::vault_io`.
//!
//! Gdy zapis zdalny nie wychodzi (brak sieci, padnięty SSH), treść ląduje
//! w kopii awaryjnej w `~/.cache/cosmic-widget-note/` — nic z tego, co użytkownik
//! napisał, nie może przepaść tylko dlatego, że nie ma internetu.

use std::io;
use std::process::Command;
use std::path::{Path, PathBuf};

use cosmic_widget_common::vault_io::{self, Remote};

use crate::config::Config;

pub struct Note {
    vault_path: PathBuf,
    vault_name: String,
    /// Ścieżka pliku względem vaulta, np. `Notatnik.md`.
    rel: String,
    path: PathBuf,
    /// Katalog archiwum (`zzzNotatnik`) — pełna ścieżka lokalna.
    archive_dir: PathBuf,
    remote: Option<Remote>,
}

impl Note {
    pub fn from_config(config: &Config) -> Self {
        let rel = config.note_file.trim_start_matches('/').to_string();
        let rel = if rel.is_empty() { "Notatnik.md".to_string() } else { rel };
        Note {
            path: config.vault_path.join(&rel),
            vault_path: config.vault_path.clone(),
            vault_name: config.vault_name.clone(),
            archive_dir: config
                .vault_path
                .join(config.archive_folder.trim_matches('/')),
            rel,
            remote: Remote::from_config(&config.remote_host, &config.remote_vault_path),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Nazwa pliku do stopki karty.
    pub fn label(&self) -> &str {
        &self.rel
    }

    /// Treść z lokalnej kopii vaulta. Brak pliku = pusta notatka, a nie błąd —
    /// plik powstanie przy pierwszym zapisie.
    pub fn load(&self) -> io::Result<String> {
        match std::fs::read_to_string(&self.path) {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(e),
        }
    }

    /// Zapis treści. Idzie na serwer (jeśli skonfigurowany); przy błędzie
    /// zostaje kopia awaryjna w cache i błąd do pokazania na karcie.
    pub fn save(&self, text: &str) -> io::Result<()> {
        let content = normalize(text);
        match vault_io::write(self.remote.as_ref(), &self.vault_path, &self.path, &content) {
            Ok(()) => Ok(()),
            Err(e) => {
                let backup = self.backup(&content);
                Err(io::Error::other(match backup {
                    Some(p) => format!("{e} (kopia: {})", p.display()),
                    None => e.to_string(),
                }))
            }
        }
    }

    /// Odłóż treść do archiwum jako osobną notatkę
    /// `<archiwum>/RRRR-MM-DD GG-MM — <pierwsza linijka>.md`. Zapis tą samą
    /// drogą co notatka (serwer po SSH). Nazwy nie nadpisujemy — przy
    /// kolizji dochodzi „ (2)", „ (3)"…
    pub fn archive(&self, text: &str) -> io::Result<PathBuf> {
        let content = normalize(text);
        let stamp = jiff::Zoned::now().strftime("%Y-%m-%d %H-%M").to_string();
        let title = title_of(&content);
        let base = if title.is_empty() {
            stamp
        } else {
            format!("{stamp} — {title}")
        };
        let mut path = self.archive_dir.join(format!("{base}.md"));
        let mut n = 2;
        while path.exists() {
            path = self.archive_dir.join(format!("{base} ({n}).md"));
            n += 1;
        }
        vault_io::write(self.remote.as_ref(), &self.vault_path, &path, &content)?;
        Ok(path)
    }

    /// Kopia awaryjna nieudanego zapisu; zwraca ścieżkę, jeśli się udała.
    fn backup(&self, content: &str) -> Option<PathBuf> {
        let dir = dirs::cache_dir()?.join("cosmic-widget-note");
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join("niezapisane.md");
        std::fs::write(&path, content).ok()?;
        Some(path)
    }

    /// `obsidian://open?vault=...&file=...` dla tej notatki.
    pub fn open_uri(&self) -> String {
        let file = self.rel.strip_suffix(".md").unwrap_or(&self.rel);
        format!(
            "obsidian://open?vault={}&file={}",
            percent_encode(&self.vault_name),
            percent_encode(file)
        )
    }
}

/// Otwiera notatkę w programie od plików `.md`.
///
/// Samo `xdg-open <plik>` nie wystarcza: typ pliku rozpoznaje po ZAWARTOŚCI,
/// więc pusta notatka to dla niego `inode/x-empty` — typ, który nie ma
/// przypisanego programu, i system spada do przeglądarki (2026-08-26: pusta
/// notatka otwierała się w Firefoksie). Dlatego pytamy wprost o program od
/// `text/markdown` i uruchamiamy go przez `gtk-launch`; `xdg-open` zostaje
/// jako zapas, gdyby czegoś z tej ścieżki brakowało.
pub fn open_file(path: &Path) {
    if let Some(app) = default_app(&["text/markdown", "text/plain"]) {
        // `gtk-launch` kończy się od razu po uruchomieniu programu, więc
        // czekanie na jego wynik nie blokuje widgetu, a daje nam sygnał,
        // czy w ogóle się udało.
        match Command::new("gtk-launch").arg(&app).arg(path).status() {
            Ok(st) if st.success() => return,
            Ok(st) => eprintln!("cosmic-widget-note: gtk-launch {app} zwrócił {st}"),
            Err(e) => eprintln!("cosmic-widget-note: nie mogę uruchomić gtk-launch: {e}"),
        }
    }
    open_uri(&path.to_string_lossy());
}

/// Otwiera adres (`obsidian://…`) — tu sniffowanie typu nie wchodzi w grę,
/// bo o programie decyduje schemat URI.
pub fn open_uri(target: &str) {
    if let Err(e) = Command::new("xdg-open").arg(target).spawn() {
        eprintln!("cosmic-widget-note: nie mogę uruchomić xdg-open: {e}");
    }
}

/// Pierwszy z podanych typów MIME, dla którego system zna program;
/// zwraca identyfikator pliku `.desktop`.
fn default_app(mimes: &[&str]) -> Option<String> {
    for mime in mimes {
        let out = Command::new("xdg-mime")
            .args(["query", "default", mime])
            .output()
            .ok()?;
        let app = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !app.is_empty() {
            return Some(app);
        }
    }
    None
}

/// Tytuł do nazwy pliku: pierwsza niepusta linijka bez znaczników
/// Markdowna (`#`, `-`, `[ ]`) i bez znaków zakazanych w nazwach plików,
/// przycięta do 50 znaków.
fn title_of(content: &str) -> String {
    let line = content.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let line = line
        .trim_start_matches(['#', '-', '*', '>', ' '])
        .trim_start_matches("[ ]")
        .trim_start_matches("[x]")
        .trim();
    let clean: String = line
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '[' | ']' | '#' | '^'))
        .collect();
    let clean = clean.trim().trim_start_matches('.');
    let cut: String = clean.chars().take(50).collect();
    cut.trim().to_string()
}

/// Markdown lubi kończyć się jednym znakiem nowej linii; edytor tego nie
/// dokłada, a bez tego każdy zapis kłóciłby się z tym, co pisze Obsidian.
pub fn normalize(text: &str) -> String {
    let trimmed = text.trim_end_matches('\n');
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
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

#[cfg(test)]
mod tests {
    use super::{normalize, title_of};

    #[test]
    fn tytul_z_pierwszej_linijki() {
        assert_eq!(title_of("\n\n# Domki nad jeziorem\ncoś"), "Domki nad jeziorem");
        assert_eq!(title_of("- [ ] zadzwonić: ZUS/US"), "zadzwonić ZUSUS");
        assert_eq!(title_of(""), "");
        assert_eq!(title_of(&"a".repeat(80)).len(), 50);
    }

    #[test]
    fn normalize_daje_dokladnie_jeden_koncowy_enter() {
        assert_eq!(normalize("abc"), "abc\n");
        assert_eq!(normalize("abc\n\n\n"), "abc\n");
        assert_eq!(normalize(""), "");
        assert_eq!(normalize("\n\n"), "");
    }
}
