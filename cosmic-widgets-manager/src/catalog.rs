//! Lista widżetów z repo + operacje: status, start/stop, autostart.

use crate::i18n::t;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct WidgetDef {
    pub label_pl: &'static str,
    pub label_en: &'static str,
    pub desc_pl: &'static str,
    pub desc_en: &'static str,
    /// Nazwa binarki (= nazwa katalogu w ~/.config/<bin>/).
    pub bin: &'static str,
    /// Czy widżet czyta vault Obsidiana (ma `vault_path` w config.toml).
    pub vault: bool,
}

impl WidgetDef {
    pub fn label(&self) -> &'static str { t(self.label_pl, self.label_en) }
    pub fn desc(&self) -> &'static str { t(self.desc_pl, self.desc_en) }
}

const fn w(label_pl: &'static str, label_en: &'static str, desc_pl: &'static str, desc_en: &'static str, bin: &'static str, vault: bool) -> WidgetDef {
    WidgetDef { label_pl, label_en, desc_pl, desc_en, bin, vault }
}

pub const WIDGETS: &[WidgetDef] = &[
    w("Kalendarz Obsidiana", "Obsidian calendar", "Kalendarz notatek dziennych, klik otwiera lub tworzy notatkę dnia", "Daily-notes calendar; a click opens or creates the day's note", "cosmic-calendar-widget", true),
    w("Zadania", "Tasks", "Zaległe i dzisiejsze zadania (wtyczka Tasks)", "Overdue and today's tasks (Tasks plugin)", "cosmic-widget-tasks", true),
    w("Notatka", "Note", "Karteczka na pulpicie zapisana jako plik .md w vaulcie", "A desktop sticky note saved as an .md file in the vault", "cosmic-widget-note", true),
    w("Szuflady", "Drawers", "Wysuwane szuflady: widżety i drzewo vaulta", "Slide-out drawers: widgets and the vault tree", "cosmic-widget-drawer", true),
    w("Agenda", "Agenda", "Nadchodzące wydarzenia z Kalendarza Google (prywatny ICS)", "Upcoming events from Google Calendar (private ICS)", "cosmic-widget-agenda", false),
    w("Pogoda", "Weather", "Aktualna pogoda i krótka prognoza", "Current weather and a short forecast", "cosmic-widget-weather", false),
    w("Rynki", "Markets", "Ceny aktywów i wykresy", "Asset prices and charts", "cosmic-widget-markets", false),
    w("Panel usług", "Services panel", "Status własnych usług z Twojego endpointu JSON", "Status of your own services from a JSON endpoint", "cosmic-widget-panel", false),
];

pub fn config_path(bin: &str) -> PathBuf {
    dirs::config_dir().unwrap_or_default().join(bin).join("config.toml")
}

fn autostart_path(bin: &str) -> PathBuf {
    dirs::config_dir().unwrap_or_default().join("autostart").join(format!("{bin}.desktop"))
}

/// Gdzie leży binarka: ~/.local/bin, potem PATH.
pub fn binary_path(bin: &str) -> Option<PathBuf> {
    let local = dirs::home_dir()?.join(".local/bin").join(bin);
    if local.is_file() {
        return Some(local);
    }
    std::env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .map(|d| PathBuf::from(d).join(bin))
        .find(|p| p.is_file())
}

/// PID-y działających procesów o danej nazwie (po arg0, bo comm ma limit 15 znaków).
pub fn running_pids(bin: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir("/proc") else { return out };
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else { continue };
        let Ok(cmd) = fs::read(e.path().join("cmdline")) else { continue };
        let arg0 = cmd.split(|b| *b == 0).next().unwrap_or(&[]);
        let arg0 = String::from_utf8_lossy(arg0);
        if arg0.rsplit('/').next() == Some(bin) {
            out.push(pid);
        }
    }
    out
}

pub fn is_running(bin: &str) -> bool {
    !running_pids(bin).is_empty()
}

pub fn start(bin: &str) -> Result<(), String> {
    let path = binary_path(bin).ok_or_else(|| format!("{} {bin} ~/.local/bin / PATH", t("nie znalazłem", "not found:")))?;
    Command::new(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{} {bin}: {e}", t("start", "start")))
}

pub fn stop(bin: &str) {
    for pid in running_pids(bin) {
        let _ = Command::new("kill").arg(pid.to_string()).status();
    }
}

pub fn has_autostart(bin: &str) -> bool {
    autostart_path(bin).is_file()
}

pub fn set_autostart(def: &WidgetDef, on: bool) -> Result<(), String> {
    let path = autostart_path(def.bin);
    if on {
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let body = format!(
            "[Desktop Entry]\nType=Application\nName={}\nExec={}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
            def.label(), def.bin
        );
        fs::write(&path, body).map_err(|e| e.to_string())
    } else if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

/// Blokada dla testów, które ustawiają XDG_CONFIG_HOME (zmienna procesu).
#[cfg(test)]
pub static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
