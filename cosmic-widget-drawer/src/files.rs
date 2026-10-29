//! Drzewo katalogów vaulta Obsidiana dla lewej szuflady.
//!
//! Czysta logika: co pokazać i w jakiej kolejności. Rysowanie jest w
//! `app.rs`, otwieranie notatki — w `open_entry`.
//!
//! Skanujemy **leniwie**: przy starcie tylko korzeń vaulta, potem katalog
//! dopiero wtedy, gdy użytkownik go rozwinie. Vault ma tysiące plików, a
//! `read_dir` jednego katalogu to mikrosekundy — więc nie ma tu ani cache'a
//! z unieważnianiem, ani skanu w tle.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Katalogi pomijane zawsze: kropkowe (`.obsidian`, `.git`, `.makemd`,
/// `.trash`, `.stversions`) plus śmieci narzędziowe. Te same nazwy, co w
/// skanerze zadań — vault jest ten sam.
const SKIP_DIRS: &[&str] = &["node_modules", "venv", "target", "__pycache__", "dist", "build"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    /// Nazwa do pokazania — notatki bez rozszerzenia `.md`.
    pub label: String,
    pub is_dir: bool,
}

/// Jeden wiersz gotowy do narysowania: wpis plus jego poziom zagnieżdżenia.
#[derive(Debug, Clone)]
pub struct Row {
    pub entry: Entry,
    pub depth: usize,
    pub expanded: bool,
}

pub struct FileTree {
    root: PathBuf,
    /// Czy pokazywać pliki inne niż `.md` (PDF-y, obrazki, arkusze).
    show_all: bool,
    expanded: HashSet<PathBuf>,
    listing: HashMap<PathBuf, Vec<Entry>>,
}

impl FileTree {
    pub fn new(root: PathBuf, show_all: bool) -> Self {
        let mut tree = Self {
            root,
            show_all,
            expanded: HashSet::new(),
            listing: HashMap::new(),
        };
        tree.load(&tree.root.clone());
        tree
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Klik w katalog: rozwiń albo zwiń. Rozwinięcie dociąga zawartość.
    pub fn toggle(&mut self, dir: &Path) {
        if self.expanded.remove(dir) {
            return;
        }
        self.expanded.insert(dir.to_path_buf());
        if !self.listing.contains_key(dir) {
            self.load(dir);
        }
    }

    /// Przeładuj to, co widać (użytkownik dopisał notatkę w Obsidianie).
    pub fn reload(&mut self) {
        let known: Vec<PathBuf> = self.listing.keys().cloned().collect();
        self.listing.clear();
        for dir in known {
            if dir == self.root || self.expanded.contains(&dir) {
                self.load(&dir);
            }
        }
        if !self.listing.contains_key(&self.root) {
            self.load(&self.root.clone());
        }
    }

    /// Spłaszczone drzewo: korzeń plus zawartość rozwiniętych katalogów.
    pub fn rows(&self) -> Vec<Row> {
        let mut out = Vec::new();
        self.collect(&self.root, 0, &mut out);
        out
    }

    fn collect(&self, dir: &Path, depth: usize, out: &mut Vec<Row>) {
        let Some(entries) = self.listing.get(dir) else {
            return;
        };
        for e in entries {
            let expanded = e.is_dir && self.expanded.contains(&e.path);
            out.push(Row {
                entry: e.clone(),
                depth,
                expanded,
            });
            if expanded {
                self.collect(&e.path, depth + 1, out);
            }
        }
    }

    fn load(&mut self, dir: &Path) {
        let mut entries = Vec::new();
        if let Ok(read) = std::fs::read_dir(dir) {
            for item in read.flatten() {
                let path = item.path();
                let name = item.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let is_dir = path.is_dir();
                if is_dir {
                    if SKIP_DIRS.contains(&name.as_str()) {
                        continue;
                    }
                    entries.push(Entry {
                        path,
                        label: name,
                        is_dir: true,
                    });
                    continue;
                }
                // Kopie konfliktowe Syncthinga tylko zaśmiecają listę —
                // vault ich się dorabia sam (patrz nota o sync-conflict).
                if name.contains(".sync-conflict-") {
                    continue;
                }
                let is_md = name.to_lowercase().ends_with(".md");
                if !is_md && !self.show_all {
                    continue;
                }
                let label = if is_md {
                    name.trim_end_matches(".md").trim_end_matches(".MD").to_string()
                } else {
                    name
                };
                entries.push(Entry {
                    path,
                    label,
                    is_dir: false,
                });
            }
        }
        // Katalogi przed plikami, każde alfabetycznie bez względu na wielkość
        // liter — tak samo, jak układa to panel plików Obsidiana.
        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
        });
        self.listing.insert(dir.to_path_buf(), entries);
    }
}

// =========================================================================
// Operacje na plikach — zarządzanie vaultem prosto z szuflady
// =========================================================================
//
// Wszystko zwykłym `std::fs`: Obsidian sam zauważy zmiany na dysku (to
// ten sam mechanizm, co przy Syncthingu). Nic nie jest kasowane na stałe —
// „usuń" przenosi do `<vault>/.trash`, jak kosz Obsidiana.
//
// Ograniczenie: zmiana nazwy/przeniesienie POZA Obsidianem nie poprawia
// wikilinków `[[…]]` w innych notatkach (Obsidian widzi to jako „plik
// zniknął, pojawił się inny").

impl FileTree {
    /// Rozwiń katalog (bez zwijania, gdy już rozwinięty) — po utworzeniu
    /// lub przeniesieniu czegoś do środka, żeby wynik było widać.
    pub fn expand(&mut self, dir: &Path) {
        if dir == self.root {
            return;
        }
        self.expanded.insert(dir.to_path_buf());
        self.load(dir);
    }

    /// Katalog zmienił ścieżkę — przenieś stan „rozwinięty" jego i dzieci.
    pub fn relocated(&mut self, from: &Path, to: &Path) {
        let moved: Vec<PathBuf> = self
            .expanded
            .iter()
            .filter(|p| p.starts_with(from))
            .cloned()
            .collect();
        for p in moved {
            self.expanded.remove(&p);
            if let Ok(rest) = p.strip_prefix(from) {
                self.expanded.insert(to.join(rest));
            }
        }
    }
}

/// Czy wpis jest notatką (pokazujemy go bez `.md` i tak go nazywamy).
pub fn is_note(path: &Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().eq_ignore_ascii_case("md"))
        .unwrap_or(false)
}

/// Nazwa do pola edycji: notatka bez `.md`, reszta w całości.
pub fn edit_label(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if is_note(path) && !path.is_dir() {
        name[..name.len() - 3].to_string()
    } else {
        name
    }
}

/// Sprawdź nazwę wpisaną przez użytkownika. Ukośnik zrobiłby z nazwy ścieżkę,
/// a kropka na początku schowałaby wpis (sami ukrywamy kropkowe).
fn clean_name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    if name.is_empty() {
        return Err("pusta nazwa".into());
    }
    if name.contains('/') || name.contains('\\') {
        return Err("nazwa nie może zawierać „/\"".into());
    }
    if name.starts_with('.') {
        return Err("nazwa nie może zaczynać się od kropki".into());
    }
    Ok(name.to_string())
}

fn with_md(name: String) -> String {
    if name.to_lowercase().ends_with(".md") {
        name
    } else {
        format!("{name}.md")
    }
}

fn ensure_free(path: &Path) -> Result<(), String> {
    if path.exists() {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        Err(format!("„{name}\" już istnieje"))
    } else {
        Ok(())
    }
}

/// Nowa, pusta notatka `<dir>/<nazwa>.md`.
pub fn create_note(dir: &Path, raw: &str) -> Result<PathBuf, String> {
    let path = dir.join(with_md(clean_name(raw)?));
    ensure_free(&path)?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn create_folder(dir: &Path, raw: &str) -> Result<PathBuf, String> {
    let path = dir.join(clean_name(raw)?);
    ensure_free(&path)?;
    std::fs::create_dir(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Zmiana nazwy w tym samym katalogu. Notatka zostaje notatką — `.md`
/// dokładamy sami, jeśli użytkownik go nie wpisał.
pub fn rename(path: &Path, raw: &str) -> Result<PathBuf, String> {
    let mut name = clean_name(raw)?;
    if !path.is_dir() && is_note(path) {
        name = with_md(name);
    }
    let parent = path.parent().ok_or("brak katalogu nadrzędnego")?;
    let target = parent.join(name);
    if target == path {
        return Ok(target);
    }
    // Sama zmiana wielkości liter („notatka" → „Notatka") na systemie
    // plików bez rozróżniania wyglądałaby jak kolizja — na ext4/btrfs
    // wystarczy zwykłe sprawdzenie.
    ensure_free(&target)?;
    std::fs::rename(path, &target).map_err(|e| e.to_string())?;
    Ok(target)
}

/// Przenieś plik albo katalog do `dir`, zachowując nazwę.
pub fn move_into(path: &Path, dir: &Path) -> Result<PathBuf, String> {
    if !dir.is_dir() {
        return Err("cel nie jest katalogiem".into());
    }
    if dir.starts_with(path) {
        return Err("nie można przenieść katalogu do jego wnętrza".into());
    }
    let name = path.file_name().ok_or("brak nazwy")?;
    let target = dir.join(name);
    if target == path {
        return Ok(target);
    }
    ensure_free(&target)?;
    std::fs::rename(path, &target).map_err(|e| e.to_string())?;
    Ok(target)
}

/// „Usuń" = przenieś do `<vault>/.trash` (kosz Obsidiana). Przy kolizji
/// nazw dokładamy znacznik czasu, żeby niczego w koszu nie nadpisać.
pub fn trash(path: &Path, root: &Path) -> Result<PathBuf, String> {
    let bin = root.join(".trash");
    std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
    let name = path.file_name().ok_or("brak nazwy")?.to_string_lossy().to_string();
    let mut target = bin.join(&name);
    if target.exists() {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let (stem, ext) = match name.rsplit_once('.') {
            Some((s, e)) if !path.is_dir() && !s.is_empty() => (s.to_string(), format!(".{e}")),
            _ => (name.clone(), String::new()),
        };
        target = bin.join(format!("{stem} ({stamp}){ext}"));
    }
    std::fs::rename(path, &target).map_err(|e| e.to_string())?;
    Ok(target)
}

/// Cofnięcie ostatniej operacji: wróć `now` na `was`, o ile miejsce wolne.
pub fn restore(now: &Path, was: &Path) -> Result<(), String> {
    ensure_free(was)?;
    if let Some(parent) = was.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(now, was).map_err(|e| e.to_string())
}

/// Otwórz notatkę. `"obsidian"` woła `obsidian://open?vault=…&file=…`
/// (notatka ląduje w Obsidianie, z wikilinkami i wtyczkami), cokolwiek
/// innego — `xdg-open`, czyli systemowy edytor tekstu.
pub fn open_entry(path: &Path, root: &Path, vault_name: &str, open_with: &str) {
    if open_with == "obsidian" && !vault_name.is_empty() {
        if let Ok(rel) = path.strip_prefix(root) {
            let uri = format!(
                "obsidian://open?vault={}&file={}",
                urlencode(vault_name),
                urlencode(&rel.to_string_lossy())
            );
            if std::process::Command::new("xdg-open").arg(&uri).spawn().is_ok() {
                return;
            }
            eprintln!("cosmic-widget-drawer: nie udało się otworzyć {uri}, próbuję systemowo");
        }
    }
    // Notatka idzie do aplikacji od `text/markdown` wprost. `xdg-open`
    // rozpoznaje typ po TREŚCI, a świeżo utworzona (pusta) notatka to dla
    // niego `inode/x-empty` bez skojarzenia — lądowała w przeglądarce.
    if is_note(path) {
        if let Some(app) = default_app("text/markdown") {
            if std::process::Command::new("gtk-launch")
                .arg(&app)
                .arg(path)
                .spawn()
                .is_ok()
            {
                return;
            }
        }
    }
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

/// Plik `.desktop` skojarzony z typem MIME (`xdg-mime query default`).
fn default_app(mime: &str) -> Option<String> {
    let out = std::process::Command::new("xdg-mime")
        .args(["query", "default", mime])
        .output()
        .ok()?;
    let app = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!app.is_empty()).then_some(app)
}

/// Kodowanie procentowe dla URI — bez dokładania zależności na jeden adres.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{FileTree, create_folder, create_note, move_into, rename, restore, trash, urlencode};

    /// Każdy test dostaje WŁASNY katalog — testy lecą równolegle w jednym
    /// procesie, więc wspólna ścieżka po PID kasowała dane spod nóg sąsiadowi.
    fn vault(nazwa: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("drawer-test-{}-{nazwa}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Kalendarz")).unwrap();
        std::fs::create_dir_all(dir.join(".obsidian")).unwrap();
        std::fs::write(dir.join("Notatnik.md"), "x").unwrap();
        std::fs::write(dir.join("zdjecie.png"), "x").unwrap();
        std::fs::write(dir.join(".obsidian/app.json"), "{}").unwrap();
        std::fs::write(dir.join("Kalendarz/2026-09-18.md"), "x").unwrap();
        std::fs::write(
            dir.join("Notatnik.sync-conflict-20260918-120000-ABC.md"),
            "x",
        )
        .unwrap();
        dir
    }

    #[test]
    fn korzen_ma_katalogi_przed_plikami_bez_kropkowych_i_konfliktow() {
        let dir = vault("korzen");
        let tree = FileTree::new(dir.clone(), false);
        let labels: Vec<String> = tree.rows().into_iter().map(|r| r.entry.label).collect();
        assert_eq!(labels, vec!["Kalendarz", "Notatnik"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rozwiniecie_dociaga_zawartosc_katalogu() {
        let dir = vault("rozwin");
        let mut tree = FileTree::new(dir.clone(), false);
        tree.toggle(&dir.join("Kalendarz"));
        let rows = tree.rows();
        let dzien = rows.iter().find(|r| r.entry.label == "2026-09-18").unwrap();
        assert_eq!(dzien.depth, 1);
        assert!(!dzien.entry.is_dir);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn show_all_pokazuje_zalaczniki() {
        let dir = vault("zalaczniki");
        let tree = FileTree::new(dir.clone(), true);
        let labels: Vec<String> = tree.rows().into_iter().map(|r| r.entry.label).collect();
        assert!(labels.contains(&"zdjecie.png".to_string()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn urlencode_radzi_sobie_z_polskimi_znakami_i_spacja() {
        assert_eq!(urlencode("Moje projekty"), "Moje%20projekty");
        assert_eq!(urlencode("pamięć"), "pami%C4%99%C4%87");
    }

    #[test]
    fn nowa_notatka_i_folder() {
        let dir = vault("nowe");
        let n = create_note(&dir, " Pomysł ").unwrap();
        assert_eq!(n, dir.join("Pomysł.md"));
        assert!(create_note(&dir, "Pomysł.md").is_err(), "kolizja");
        assert!(create_note(&dir, "a/b").is_err());
        assert!(create_note(&dir, ".ukryta").is_err());
        let f = create_folder(&dir, "Nowy").unwrap();
        assert!(f.is_dir());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn zmiana_nazwy_zachowuje_md() {
        let dir = vault("rename");
        let n = rename(&dir.join("Notatnik.md"), "Zeszyt").unwrap();
        assert_eq!(n, dir.join("Zeszyt.md"));
        assert!(rename(&n, "Kalendarz").is_ok(), "plik Kalendarz.md ≠ katalog Kalendarz");
        let k = rename(&dir.join("Kalendarz"), "Dziennik").unwrap();
        assert!(k.join("2026-09-18.md").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn przenoszenie_kosz_i_cofanie() {
        let dir = vault("move");
        let src = dir.join("Notatnik.md");
        let moved = move_into(&src, &dir.join("Kalendarz")).unwrap();
        assert!(moved.exists() && !src.exists());
        assert!(move_into(&dir.join("Kalendarz"), &dir.join("Kalendarz")).is_err());
        restore(&moved, &src).unwrap();
        assert!(src.exists());
        let t1 = trash(&src, &dir).unwrap();
        assert_eq!(t1, dir.join(".trash/Notatnik.md"));
        std::fs::write(&src, "y").unwrap();
        let t2 = trash(&src, &dir).unwrap();
        assert!(t2 != t1 && t2.exists(), "drugi raz nie nadpisuje kosza");
        let _ = std::fs::remove_dir_all(dir);
    }
}
