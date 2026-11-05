//! Lista widżetów z repo + operacje: status, start/stop, autostart.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct WidgetDef {
    pub label: &'static str,
    pub desc: &'static str,
    /// Nazwa binarki (= nazwa katalogu w ~/.config/<bin>/).
    pub bin: &'static str,
    /// Czy widżet czyta vault Obsidiana (ma `vault_path` w config.toml).
    pub vault: bool,
}

pub const WIDGETS: &[WidgetDef] = &[
    WidgetDef { label: "Kalendarz Obsidiana", desc: "Kalendarz notatek dziennych, klik otwiera lub tworzy notatkę dnia", bin: "cosmic-calendar-widget", vault: true },
    WidgetDef { label: "Zadania", desc: "Zaległe i dzisiejsze zadania (wtyczka Tasks)", bin: "cosmic-widget-tasks", vault: true },
    WidgetDef { label: "Notatka", desc: "Karteczka na pulpicie zapisana jako plik .md w vaulcie", bin: "cosmic-widget-note", vault: true },
    WidgetDef { label: "Szuflady", desc: "Wysuwane szuflady: widżety i drzewo vaulta", bin: "cosmic-widget-drawer", vault: true },
    WidgetDef { label: "Agenda", desc: "Nadchodzące wydarzenia z Kalendarza Google (prywatny ICS)", bin: "cosmic-widget-agenda", vault: false },
    WidgetDef { label: "Pogoda", desc: "Aktualna pogoda i krótka prognoza", bin: "cosmic-widget-weather", vault: false },
    WidgetDef { label: "Rynki", desc: "Ceny aktywów i wykresy", bin: "cosmic-widget-markets", vault: false },
    WidgetDef { label: "Panel usług", desc: "Status własnych usług z Twojego endpointu JSON", bin: "cosmic-widget-panel", vault: false },
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
    let path = binary_path(bin).ok_or_else(|| format!("nie znalazłem {bin} w ~/.local/bin ani w PATH"))?;
    Command::new(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("start {bin}: {e}"))
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
            def.label, def.bin
        );
        fs::write(&path, body).map_err(|e| e.to_string())
    } else if path.exists() {
        fs::remove_file(&path).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}
