//! Rozmowa skrótu klawiszowego z działającym demonem.
//!
//! Ten sam wzorzec, co w `cosmic-widget-drawer`: skrót COSMIC-a umie tylko
//! URUCHOMIĆ program, więc `cosmic-quicklook --toggle` nie startuje drugiej
//! instancji, tylko łączy się z gniazdem uniksowym demona i wysyła słowo.
//! Gdy demona nie ma — uruchamia go i ponawia.
//!
//! Demon musi chodzić w tle z dwóch powodów: (1) start libcosmica trwa
//! ~300 ms, a podgląd ma wyskakiwać natychmiast; (2) tylko proces, który
//! już żyje, wie, czy podgląd jest otwarty — druga spacja ma go zamykać.
//!
//! Gniazdo: `$XDG_RUNTIME_DIR/cosmic-quicklook.sock`.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cosmic::iced::Subscription;
use cosmic::iced::futures::SinkExt;
use cosmic::iced::futures::channel::mpsc::Sender;
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Spacja: pokaż podgląd zaznaczonego pliku albo schowaj otwarty.
    Toggle,
    Show,
    Hide,
    /// Konkretny plik — z wiersza poleceń albo z „Otwórz za pomocą”.
    /// Omija całą zabawę ze schowkiem.
    Path(PathBuf),
}

impl Command {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        match s {
            "toggle" | "--toggle" => Some(Self::Toggle),
            "show" | "--show" => Some(Self::Show),
            "hide" | "--hide" => Some(Self::Hide),
            _ => s.strip_prefix("path:").map(|p| Self::Path(PathBuf::from(p))),
        }
    }

    fn word(&self) -> String {
        match self {
            Self::Toggle => "toggle".into(),
            Self::Show => "show".into(),
            Self::Hide => "hide".into(),
            Self::Path(p) => format!("path:{}", p.display()),
        }
    }

    /// Ścieżkę podaną w wierszu poleceń zamieniamy na bezwzględną, zanim
    /// pójdzie do demona — on ma zupełnie inny katalog roboczy.
    #[must_use]
    pub fn from_arg(arg: &str) -> Option<Self> {
        if let Some(cmd) = Self::parse(arg) {
            return Some(cmd);
        }
        let p = Path::new(arg);
        if !p.exists() {
            return None;
        }
        let abs = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        Some(Self::Path(abs))
    }
}

#[must_use]
pub fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join("cosmic-quicklook.sock")
}

/// Strona demona: nasłuch gniazda jako subskrypcja iced.
#[must_use]
pub fn subscription() -> Subscription<Command> {
    Subscription::run(|| {
        cosmic::iced::stream::channel(4, |mut output: Sender<Command>| async move {
            let path = socket_path();
            // Po `kill -9` zostaje plik gniazda i `bind` by się nie udał.
            // Jesteśmy jedynym demonem — klient najpierw próbuje się połączyć.
            let _ = std::fs::remove_file(&path);
            let listener = match tokio::net::UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("cosmic-quicklook: nie mogę otworzyć {}: {e}", path.display());
                    std::future::pending::<()>().await;
                    return;
                }
            };
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    continue;
                };
                let mut buf = vec![0u8; 4096];
                let Ok(n) = stream.read(&mut buf).await else {
                    continue;
                };
                if let Some(cmd) = Command::parse(&String::from_utf8_lossy(&buf[..n])) {
                    let _ = output.send(cmd).await;
                }
            }
        })
    })
}

/// Strona klienta (`--toggle` i spółka).
pub fn send(cmd: &Command) -> Result<(), String> {
    if try_send(cmd).is_ok() {
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe)
        .spawn()
        .map_err(|e| format!("nie mogę uruchomić demona: {e}"))?;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(100));
        if try_send(cmd).is_ok() {
            return Ok(());
        }
    }
    Err("demon nie odpowiada".into())
}

fn try_send(cmd: &Command) -> std::io::Result<()> {
    let mut stream = std::os::unix::net::UnixStream::connect(socket_path())?;
    stream.write_all(cmd.word().as_bytes())?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::Command;
    use std::path::PathBuf;

    #[test]
    fn parsuje_obie_formy() {
        assert_eq!(Command::parse("--toggle"), Some(Command::Toggle));
        assert_eq!(Command::parse("hide\n"), Some(Command::Hide));
        assert_eq!(Command::parse("nonsens"), None);
    }

    #[test]
    fn sciezka_przechodzi_tam_i_z_powrotem() {
        let cmd = Command::Path(PathBuf::from("/home/user/a b.pdf"));
        assert_eq!(Command::parse(&cmd.word()), Some(cmd));
    }
}
