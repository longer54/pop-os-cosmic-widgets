//! Podpięcie vaulta Obsidiana: walidacja folderu i zapis do configów widżetów.

use crate::catalog::{WIDGETS, config_path};
use std::fs;
use std::path::Path;
use toml_edit::{DocumentMut, value};

/// Vault Obsidiana to folder z podfolderem `.obsidian/`.
pub fn is_vault(p: &Path) -> bool {
    p.join(".obsidian").is_dir()
}

/// Wpisuje `vault_path` i `vault_name` do config.toml każdego widżetu z vaultem.
/// Reszta pliku (komentarze, inne klucze) zostaje nietknięta. Zwraca liczbę zmienionych configów.
pub fn apply_vault(p: &Path) -> Result<usize, String> {
    if !is_vault(p) {
        return Err(format!("{} nie wygląda na vault Obsidiana (brak folderu .obsidian)", p.display()));
    }
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut n = 0;
    for w in WIDGETS.iter().filter(|w| w.vault) {
        let cfg = config_path(w.bin);
        let text = fs::read_to_string(&cfg).unwrap_or_default();
        let mut doc: DocumentMut = text.parse().map_err(|e| format!("{}: {e}", cfg.display()))?;
        doc["vault_path"] = value(p.to_string_lossy().as_ref());
        doc["vault_name"] = value(name.as_str());
        if let Some(dir) = cfg.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        fs::write(&cfg, doc.to_string()).map_err(|e| format!("{}: {e}", cfg.display()))?;
        n += 1;
    }
    Ok(n)
}

/// Aktualnie ustawiony vault (z configu kalendarza, jeśli jest).
pub fn current_vault() -> Option<String> {
    let text = fs::read_to_string(config_path("cosmic-calendar-widget")).ok()?;
    let doc: DocumentMut = text.parse().ok()?;
    doc.get("vault_path")?.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zapisuje_vault_i_zachowuje_reszte() {
        let base = std::env::temp_dir().join(format!("cwm-test-{}", std::process::id()));
        let vault = base.join("MojVault");
        fs::create_dir_all(vault.join(".obsidian")).unwrap();
        let cfg_home = base.join("cfg");
        // SAFETY: test jednowątkowy względem tej zmiennej
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &cfg_home) };
        let tasks = config_path("cosmic-widget-tasks");
        fs::create_dir_all(tasks.parent().unwrap()).unwrap();
        fs::write(&tasks, "# mój komentarz\nrefresh_secs = 30\nvault_path = \"/stare\"\n").unwrap();

        assert_eq!(apply_vault(&vault).unwrap(), 4);
        let t = fs::read_to_string(&tasks).unwrap();
        assert!(t.contains("# mój komentarz") && t.contains("refresh_secs = 30"));
        assert!(t.contains(&format!("vault_path = \"{}\"", vault.display())));
        assert!(t.contains("vault_name = \"MojVault\""));
        assert!(config_path("cosmic-widget-note").is_file());
        assert!(apply_vault(&base).is_err()); // brak .obsidian
        let _ = fs::remove_dir_all(&base);
    }
}
