//! Rozmowa skrótu klawiszowego z działającym demonem.
//!
//! Szuflada to jeden proces w tle (autostart), który trzyma pobrane dane i
//! tworzy powierzchnię layer-shell dopiero na żądanie. Skrót klawiszowy
//! COSMIC-a umie tylko uruchomić program, więc `cosmic-widget-drawer
//! --toggle` nie startuje drugiej instancji, tylko łączy się z gniazdem
//! uniksowym demona i wysyła jedno słowo. Gdy demona nie ma — uruchamia go
//! i ponawia próbę.
//!
//! Gniazdo: `$XDG_RUNTIME_DIR/cosmic-widget-drawer.sock` (per użytkownik,
//! znika po wylogowaniu).

use std::io::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use cosmic::iced::Subscription;
use cosmic::iced::futures::SinkExt;
use cosmic::iced::futures::channel::mpsc::Sender;
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Show,
    Hide,
    Toggle,
    /// To samo dla lewej szuflady (drzewo vaulta).
    ShowFiles,
    HideFiles,
    ToggleFiles,
    /// Obie szuflady naraz (skrót użytkownika: Super+N). Wciśnięcie przy
    /// którejkolwiek otwartej chowa obie — inaczej po jednym wciśnięciu
    /// zostawałaby otwarta ta, która akurat nie pasowała do stanu.
    ToggleBoth,
    /// Gest gładzikiem. Sama strona ruchu, bez decyzji „co zrobić” — tę
    /// podejmuje demon, bo tylko on wie, która szuflada jest otwarta:
    /// ruch w lewo wysuwa prawą albo chowa lewą, w prawo odwrotnie.
    SwipeLeft,
    SwipeRight,
}

impl Command {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "show" | "--show" => Some(Self::Show),
            "hide" | "--hide" => Some(Self::Hide),
            "toggle" | "--toggle" => Some(Self::Toggle),
            "show-files" | "--show-files" => Some(Self::ShowFiles),
            "hide-files" | "--hide-files" => Some(Self::HideFiles),
            "toggle-files" | "--toggle-files" => Some(Self::ToggleFiles),
            "toggle-both" | "--toggle-both" => Some(Self::ToggleBoth),
            "swipe-left" | "--swipe-left" => Some(Self::SwipeLeft),
            "swipe-right" | "--swipe-right" => Some(Self::SwipeRight),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::Hide => "hide",
            Self::Toggle => "toggle",
            Self::ShowFiles => "show-files",
            Self::HideFiles => "hide-files",
            Self::ToggleFiles => "toggle-files",
            Self::ToggleBoth => "toggle-both",
            Self::SwipeLeft => "swipe-left",
            Self::SwipeRight => "swipe-right",
        }
    }
}

#[must_use]
pub fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join("cosmic-widget-drawer.sock")
}

/// Strona demona: nasłuch gniazda jako subskrypcja iced.
#[must_use]
pub fn subscription() -> Subscription<Command> {
    Subscription::run(|| {
        cosmic::iced::stream::channel(4, |mut output: Sender<Command>| async move {
            let path = socket_path();
            // Po nieczystym wyjściu (kill -9) zostaje plik gniazda i bind
            // by się nie udał. Kasujemy — jesteśmy jedynym demonem, bo
            // druga instancja i tak najpierw próbuje się połączyć.
            let _ = std::fs::remove_file(&path);
            let listener = match tokio::net::UnixListener::bind(&path) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("cosmic-widget-drawer: nie mogę otworzyć {}: {e}", path.display());
                    // Bez gniazda zostaje otwieranie krawędzią — nie ubijamy
                    // procesu, tylko kończymy tę subskrypcję.
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

/// Strona klienta (`--toggle` i spółka). Zwraca `Err`, gdy nie udało się
/// dogadać z demonem mimo jego uruchomienia.
pub fn send(cmd: Command) -> Result<(), String> {
    if try_send(cmd).is_ok() {
        return Ok(());
    }
    // Demona nie ma — uruchom go i poczekaj, aż otworzy gniazdo.
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe)
        .spawn()
        .map_err(|e| format!("nie mogę uruchomić demona: {e}"))?;
    for _ in 0..30 {
        std::thread::sleep(Duration::from_millis(100));
        if try_send(cmd).is_ok() {
            return Ok(());
        }
    }
    Err("demon nie odpowiada".into())
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
    fn parsuje_obie_formy() {
        assert_eq!(Command::parse("--toggle"), Some(Command::Toggle));
        assert_eq!(Command::parse("show\n"), Some(Command::Show));
        assert_eq!(Command::parse("nonsens"), None);
    }

    #[test]
    fn kazda_komenda_przechodzi_tam_i_z_powrotem() {
        for cmd in [
            Command::Show,
            Command::Hide,
            Command::Toggle,
            Command::ShowFiles,
            Command::HideFiles,
            Command::ToggleFiles,
            Command::SwipeLeft,
            Command::SwipeRight,
        ] {
            assert_eq!(Command::parse(cmd.word()), Some(cmd));
        }
    }
}
