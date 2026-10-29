//! Odczyt lokalny, zapis przez kanoniczną kopię vaulta na serwerze (SSH).
//!
//! Widgety **czytają** lokalną kopię vaulta — szybko i bez sieci. Ale każdy
//! **zapis** (odhaczenie zadania, dopisanie pozycji `#pamięć`) idzie po SSH
//! na jedno urządzenie: serwer z vaultem. Dzięki temu nigdy nie powstają dwie
//! rozbieżne wersje tej samej notatki, więc Syncthing nie ma czego
//! rozstrzygać i plik `*.sync-conflict-*` nie ma jak powstać. Zmiana wraca
//! na wszystkie komputery normalną synchronizacją.
//!
//! Powód (2026-08-25): widget na uśpionym komputerze pokazuje starą wersję
//! pliku, więc to samo zadanie dawało się odhaczyć na dwóch maszynach — i
//! dokładnie tak powstał konflikt w `Kalendarz/2026/07/2026-07-09.md`.
//!
//! Pusty `host` w configu = zapis lokalny, czyli zachowanie sprzed zmiany.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Kanoniczna kopia vaulta na innej maszynie.
#[derive(Debug, Clone)]
pub struct Remote {
    /// Host SSH — alias z `~/.ssh/config` (np. `myserver`).
    pub host: String,
    /// Katalog vaulta na tamtej maszynie; odpowiednik lokalnego `vault_path`.
    pub root: PathBuf,
}

impl Remote {
    /// `None`, gdy host lub ścieżka są puste — wtedy zapis leci lokalnie.
    pub fn from_config(host: &str, root: &Path) -> Option<Self> {
        let host = host.trim();
        if host.is_empty() || root.as_os_str().is_empty() {
            return None;
        }
        Some(Self { host: host.to_string(), root: root.to_path_buf() })
    }

    /// Przekłada ścieżkę w lokalnym vaultcie na ścieżkę na serwerze.
    fn map(&self, local_root: &Path, path: &Path) -> io::Result<PathBuf> {
        let rel = path.strip_prefix(local_root).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("plik {} jest poza vaultem {}", path.display(), local_root.display()),
            )
        })?;
        Ok(self.root.join(rel))
    }

    fn ssh(&self) -> Command {
        let mut cmd = Command::new("ssh");
        // BatchMode: nigdy nie pytaj o hasło — widget nie ma gdzie go wpisać,
        // lepiej czytelny błąd niż wiszący proces.
        cmd.arg("-o")
            .arg("BatchMode=yes")
            .arg("-o")
            .arg("ConnectTimeout=5");
        // Współdzielone połączenie: pierwszy zapis płaci za zestawienie sesji
        // SSH (~0,3 s), każdy następny leci gotowym tunelem (~0,03 s). Bez
        // tego każdy podwójny klik zacinałby widget na czas handshake'u.
        let run_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
        cmd.arg("-o")
            .arg("ControlMaster=auto")
            .arg("-o")
            .arg(format!("ControlPath={run_dir}/cosmic-widget-ssh-%r@%h:%p"))
            .arg("-o")
            .arg("ControlPersist=10m")
            .arg(&self.host);
        cmd
    }
}

/// Cudzysłowy dla zdalnego shella — ścieżki użytkownika mają spacje i polskie znaki.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Czyta plik. `None` = pliku nie ma (a nie: błąd).
///
/// Używane tam, gdzie widget musi zobaczyć **aktualną** treść przed zapisem —
/// czyli tę z serwera, nie ze swojej (może przestarzałej) kopii.
pub fn read_before_write(
    remote: Option<&Remote>,
    local_root: &Path,
    path: &Path,
) -> io::Result<Option<String>> {
    let Some(remote) = remote else {
        return match std::fs::read_to_string(path) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        };
    };

    let target = remote.map(local_root, path)?;
    let out = remote
        .ssh()
        .arg(format!("cat -- {}", sh_quote(&target.to_string_lossy())))
        .output()
        .map_err(|e| io::Error::other(format!("nie mogę uruchomić ssh: {e}")))?;

    if out.status.success() {
        return Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()));
    }
    let err = String::from_utf8_lossy(&out.stderr);
    if err.contains("No such file") {
        return Ok(None);
    }
    Err(io::Error::other(format!(
        "{}: nie mogę odczytać {} ({})",
        remote.host,
        target.display(),
        err.trim()
    )))
}

/// Zapisuje plik — zdalnie, jeśli skonfigurowano hosta.
///
/// Na serwerze zapis idzie przez plik tymczasowy i `mv`, żeby Syncthing nigdy
/// nie zobaczył notatki uciętej w połowie.
pub fn write(
    remote: Option<&Remote>,
    local_root: &Path,
    path: &Path,
    content: &str,
) -> io::Result<()> {
    let Some(remote) = remote else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        return std::fs::write(path, content);
    };

    let target = remote.map(local_root, path)?;
    let q = sh_quote(&target.to_string_lossy());
    let tmp = sh_quote(&format!("{}.widget-tmp", target.to_string_lossy()));
    let script = format!("mkdir -p \"$(dirname {q})\" && cat > {tmp} && mv -f {tmp} {q}");

    let mut child = remote
        .ssh()
        .arg(format!("sh -c {}", sh_quote(&script)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| io::Error::other(format!("nie mogę uruchomić ssh: {e}")))?;

    child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("ssh nie przyjmuje danych na wejściu"))?
        .write_all(content.as_bytes())?;

    let out = child.wait_with_output()?;
    if out.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "{}: nie mogę zapisać {} ({})",
        remote.host,
        target.display(),
        String::from_utf8_lossy(&out.stderr).trim()
    )))
}
