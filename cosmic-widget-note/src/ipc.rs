//! Rozmowa skrótu klawiszowego z działającym widgetem (wzór z
//! `cosmic-widget-drawer/src/ipc.rs`).
//!
//! Skrót COSMIC-a umie tylko uruchomić program, więc `cosmic-widget-note
//! --new` nie startuje drugiej kartki, tylko łączy się z gniazdem
//! działającego widgetu i wysyła jedno słowo. Gdy widgetu nie ma —
//! uruchamia go i ponawia próbę.
//!
//! Gniazdo: `$XDG_RUNTIME_DIR/cosmic-widget-note.sock`.

use std::io::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use cosmic::iced::Subscription;
use cosmic::iced::futures::SinkExt;
use cosmic::iced::futures::channel::mpsc::Sender;
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Super+Shift+N: odłóż obecną treść do archiwum, wyczyść kartkę,
    /// kursor do pisania.
    New,
    /// Samo wejście w pisanie, bez czyszczenia.
    Edit,
}

impl Command {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "new" | "--new" => Some(Self::New),
            "edit" | "--edit" => Some(Self::Edit),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Edit => "edit",
        }
    }
}

#[must_use]
pub fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join("cosmic-widget-note.sock")
}

/// Strona widgetu: nasłuch gniazda jako subskrypcja iced.
#[must_use]
pub fn subscription() -> Subscription<Command> {
    Subscription::run(|| {
        cosmic::iced::stream::channel(4, |mut output: Sender<Command>| async move {
            let path = socket_path();
            // Po nieczystym wyjściu (kill -9) zostaje plik gniazda i bind
            // by się nie udał.
            let _ = std::fs::remove_file(&path);
            let listener = match tokio::net::UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("cosmic-widget-note: nie mogę otworzyć {}: {e}", path.display());
                    std::future::pending::<()>().await;
                    return;
                }
            };
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    continue;
                };
                let mut buf = [0u8; 32];
                let Ok(n) = stream.read(&mut buf).await else { continue };
                if let Some(cmd) = Command::parse(&String::from_utf8_lossy(&buf[..n])) {
                    let _ = output.send(cmd).await;
                }
            }
        })
    })
}

/// Strona klienta (`--new`, `--edit`).
pub fn send(cmd: Command) -> Result<(), String> {
    if try_send(cmd).is_ok() {
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe)
        .spawn()
        .map_err(|e| format!("nie mogę uruchomić widgetu: {e}"))?;
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(100));
        if try_send(cmd).is_ok() {
            return Ok(());
        }
    }
    Err("widget nie odpowiada".into())
}

fn try_send(cmd: Command) -> std::io::Result<()> {
    let mut stream = std::os::unix::net::UnixStream::connect(socket_path())?;
    stream.write_all(cmd.word().as_bytes())?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::Command;

    #[test]
    fn komendy_tam_i_z_powrotem() {
        for cmd in [Command::New, Command::Edit] {
            assert_eq!(Command::parse(cmd.word()), Some(cmd));
        }
        assert_eq!(Command::parse("--new\n"), Some(Command::New));
        assert_eq!(Command::parse("nonsens"), None);
    }
}
