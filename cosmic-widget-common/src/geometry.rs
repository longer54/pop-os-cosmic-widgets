//! Publikacja geometrii widgetu dla innych widgetów (mechanizm stack).
//! Każdy widget zapisuje swoją pozycję i aktualną wysokość do
//! `~/.cache/cosmic-widgets/geometry/<nazwa>.toml`; dzieci czytają plik
//! rodzica w ticku i dopasowują własny margines pionowy.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use cosmic::iced::Subscription;
use cosmic::iced::futures::SinkExt;
use cosmic::iced::futures::channel::mpsc::Sender;

/// Co ile zaglądać do pliku rodzica. Samo zaglądanie dzieje się w tle i
/// NIE budzi widgetu — budzi go dopiero realna zmiana (patrz `watch`).
const WATCH_POLL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Geometry {
    pub anchor: String,
    pub margin_x: i32,
    pub margin_y: i32,
    pub width: u32,
    pub height: u32,
}

fn dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("cosmic-widgets")
        .join("geometry")
}

fn path(name: &str) -> PathBuf {
    dir().join(format!("{name}.toml"))
}

pub fn publish(name: &str, geo: &Geometry) {
    let _ = std::fs::create_dir_all(dir());
    if let Ok(raw) = toml::to_string(geo) {
        let _ = std::fs::write(path(name), raw);
    }
}

pub fn read(name: &str) -> Option<Geometry> {
    let raw = std::fs::read_to_string(path(name)).ok()?;
    toml::from_str(&raw).ok()
}

/// Emituje `()` przy starcie i potem tylko wtedy, gdy rodzic `parent`
/// zmienił pozycję albo wysokość. Zastępuje dawny `every(2 s) → StackTick`,
/// który co 2 s budził każdy widget w kolumnie (update + przerysowanie +
/// praca kompozytora) nawet gdy nic się nie ruszało (2026-10-06, prąd).
pub fn watch(parent: String) -> Subscription<()> {
    Subscription::run_with(parent, |parent| {
        let parent = parent.clone();
        cosmic::iced::stream::channel(2, move |mut output: Sender<()>| async move {
            let mut last: Option<(i32, u32)> = None;
            let mut first = true;
            loop {
                let now = read(&parent).map(|g| (g.margin_y, g.height));
                if first || (now.is_some() && now != last) {
                    first = false;
                    last = now;
                    let _ = output.send(()).await;
                }
                tokio::time::sleep(WATCH_POLL).await;
            }
        })
    })
}
