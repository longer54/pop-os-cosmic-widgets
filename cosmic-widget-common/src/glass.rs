//! Szkło („glass”) w stylu COSMIC — wspólny wygląd kart widgetów.
//!
//! Dwie rzeczy naraz:
//!
//! - **wygląd karty** (`Glass::container`) — tło brane z motywu COSMIC
//!   (`bg_color()` / `on_bg_color()`, więc działa i w jasnym, i w ciemnym),
//!   a **przezroczystość z suwaka przezroczystości doka** (COSMIC ustawia
//!   go w `com.system76.CosmicPanel.Dock` → klucz `opacity`). Przesunięcie
//!   suwaka w ustawieniach doka zmienia szkło widgetów bez restartu —
//!   pilnuje tego `subscription()`;
//! - **realne rozmycie tła** (`blur_task`) — `cosmic-comp` od 1.x wystawia
//!   protokół `ext-background-effect-v1` i libcosmic ma do niego komendę.
//!   Kompozytor rozmywa tapetę tylko w podanym **regionie**, dlatego nie
//!   używamy automatycznego blura z libcosmic (`core.set_auto_blur`):
//!   region domyślny to cały prostokąt powierzchni layer-shell, więc szkło
//!   wypełniłoby też przezroczyste szpary między kartami i kwadratowe rogi
//!   poza zaokrągleniem. Tutaj region składamy sami — z prostokątów
//!   dokładnie pod kartami, ze schodkowym przybliżeniem zaokrąglonych rogów.
//!
//! Na kompozytorze bez tego protokołu komenda jest ignorowana (libcosmic
//! loguje i tyle) — zostaje sama półprzezroczystość, czyli zachowanie jak
//! przed zmianą.

use std::path::PathBuf;
use std::time::Duration;

use cosmic::iced::futures::SinkExt;
use cosmic::iced::futures::channel::mpsc::Sender;
use cosmic::iced::widget::container;
use cosmic::iced::{Background, Border, Color, Rectangle, Shadow, Subscription, Vector, window};

/// Promień zaokrąglenia karty (px logiczne). Wspólna stała, bo region
/// rozmycia musi się zgadzać z tym, co rysuje iced — gdyby każdy widget
/// miał swój promień, szkło wystawałoby zza rogów.
pub const RADIUS: f32 = 18.0;

/// Gdy nie da się odczytać configu doka — tyle, co domyślny `opacity`
/// cosmic-panel.
const FALLBACK_OPACITY: f32 = 0.8;

/// Co ile sprawdzać, czy użytkownik ruszył suwakiem doka.
const POLL: Duration = Duration::from_secs(2);

/// Skąd brać poziom oszklenia. Sterowane kluczem `glass` w configu widgetu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Domyślnie: tyle, ile ma dok (suwak przezroczystości w ustawieniach).
    Dock,
    /// Na sztywno — wartość w promilach, żeby typ został `Eq` (dla `Hash`
    /// subskrypcji); `Fixed(550)` = 0.55.
    Fixed(u16),
    /// Bez szkła: karta nieprzezroczysta, bez proszenia o rozmycie.
    Off,
}

impl Level {
    /// `"dock"` (domyślnie) · `"off"` · liczba `0.0`–`1.0`.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "" | "dock" | "panel" => Self::Dock,
            "off" | "none" | "solid" => Self::Off,
            other => match other.parse::<f32>() {
                Ok(v) => Self::Fixed((v.clamp(0.0, 1.0) * 1000.0).round() as u16),
                Err(_) => Self::Dock,
            },
        }
    }
}

/// Rozwiązany poziom oszklenia — to trzyma widget w swoim stanie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glass {
    level: Level,
    opacity: f32,
}

impl Default for Glass {
    fn default() -> Self {
        Self::new(Level::Dock)
    }
}

impl Glass {
    /// Odczytaj aktualny poziom dla danego trybu.
    #[must_use]
    pub fn new(level: Level) -> Self {
        let opacity = match level {
            Level::Dock => dock_opacity().unwrap_or(FALLBACK_OPACITY),
            Level::Fixed(promile) => f32::from(promile) / 1000.0,
            Level::Off => 1.0,
        };
        Self {
            level,
            opacity: opacity.clamp(0.0, 1.0),
        }
    }

    /// Poziom z configu widgetu (klucz `glass`).
    #[must_use]
    pub fn from_config(setting: &str) -> Self {
        Self::new(Level::parse(setting))
    }

    #[must_use]
    pub fn level(self) -> Level {
        self.level
    }

    #[must_use]
    pub fn opacity(self) -> f32 {
        self.opacity
    }

    /// Czy w ogóle prosić kompozytor o rozmycie. Przy nieprzezroczystej
    /// karcie rozmycie nic nie daje, a kosztuje kompozytor pracę.
    #[must_use]
    pub fn blurred(self) -> bool {
        self.level != Level::Off && self.opacity < 0.995
    }

    /// Styl karty do `cosmic::theme::Container::custom(...)`.
    #[must_use]
    pub fn container(self) -> impl Fn(&cosmic::Theme) -> container::Style + 'static {
        move |theme| {
            let c = theme.cosmic();
            let bg = c.bg_color();
            let on = c.on_bg_color();
            container::Style {
                icon_color: Some(Color::from_rgb(on.red, on.green, on.blue)),
                text_color: Some(Color::from_rgb(on.red, on.green, on.blue)),
                background: Some(Background::Color(Color::from_rgba(
                    bg.red,
                    bg.green,
                    bg.blue,
                    self.opacity,
                ))),
                border: Border {
                    color: Color::from_rgba(on.red, on.green, on.blue, 0.14),
                    width: 1.0,
                    radius: RADIUS.into(),
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                    offset: Vector::new(0.0, 6.0),
                    blur_radius: 24.0,
                },
                ..Default::default()
            }
        }
    }
}

/// Nasłuch suwaka doka. Emituje tylko przy realnej zmianie, więc widget
/// nie przerysowuje się co dwie sekundy bez powodu. Dla `Level::Fixed`
/// i `Level::Off` nie ma czego pilnować — pusta subskrypcja.
#[must_use]
pub fn subscription(level: Level) -> Subscription<Glass> {
    if level != Level::Dock {
        return Subscription::none();
    }
    Subscription::run(|| {
        cosmic::iced::stream::channel(2, |mut output: Sender<Glass>| async move {
            let mut last = Glass::new(Level::Dock);
            loop {
                tokio::time::sleep(POLL).await;
                let now = Glass::new(Level::Dock);
                if now != last {
                    last = now;
                    let _ = output.send(now).await;
                }
            }
        })
    })
}

/// Komenda „rozmyj tło pod kartami”. `cards` to pary
/// `(offset od góry powierzchni, wysokość karty)` w px logicznych —
/// widget zna je, bo sam wylicza wysokość swojej powierzchni.
///
/// Wołać po utworzeniu powierzchni layer-shell i po każdej zmianie jej
/// rozmiaru (kompozytor kasuje region razem z powierzchnią).
#[must_use]
pub fn blur_task<T: Send + 'static>(
    id: window::Id,
    glass: Glass,
    width: u32,
    cards: &[(u32, u32)],
) -> cosmic::iced::Task<T> {
    let region = if glass.blurred() {
        let w = width as f32;
        let mut rects = Vec::new();
        for &(top, height) in cards {
            rounded_rect(0.0, top as f32, w, height as f32, RADIUS, &mut rects);
        }
        Some(rects)
    } else {
        None
    };
    cosmic::iced::platform_specific::shell::commands::blur::blur(id, region).discard()
}

/// Wariant dla powierzchni, na której karta NIE zaczyna się w lewym górnym
/// rogu — np. szuflada (`cosmic-widget-drawer`) rozpina powierzchnię na cały
/// ekran, a rysuje kartę przy prawej krawędzi. Prostokąty podajemy jawnie:
/// `(x, y, szerokość, wysokość)` w px logicznych powierzchni.
#[must_use]
pub fn blur_task_rects<T: Send + 'static>(
    id: window::Id,
    glass: Glass,
    cards: &[(f32, f32, f32, f32)],
) -> cosmic::iced::Task<T> {
    let region = if glass.blurred() {
        let mut rects = Vec::new();
        for &(x, y, w, h) in cards {
            rounded_rect(x, y, w, h, RADIUS, &mut rects);
        }
        Some(rects)
    } else {
        None
    };
    cosmic::iced::platform_specific::shell::commands::blur::blur(id, region).discard()
}

/// Skrót dla widgetów z jedną kartą na całą powierzchnię.
#[must_use]
pub fn blur_task_single<T: Send + 'static>(
    id: window::Id,
    glass: Glass,
    width: u32,
    height: u32,
) -> cosmic::iced::Task<T> {
    blur_task(id, glass, width, &[(0, height)])
}

/// Zaokrąglony prostokąt jako zestaw prostokątów: schodki na łukach
/// (górnym i dolnym) plus jeden duży prostokąt w środku. Wiersze o tym
/// samym wcięciu (po zaokrągleniu do piksela) łączymy w pasma, bo i tak
/// `wl_region` dostaje współrzędne całkowite.
fn rounded_rect(x: f32, y: f32, w: f32, h: f32, radius: f32, out: &mut Vec<Rectangle>) {
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    if r < 1.0 {
        out.push(Rectangle {
            x,
            y,
            width: w,
            height: h,
        });
        return;
    }

    let steps = r.ceil() as u32;
    let mut row = 0u32;
    while row < steps {
        let inset = arc_inset(row, r).round();
        let mut next = row + 1;
        while next < steps && arc_inset(next, r).round() == inset {
            next += 1;
        }
        let band = (next - row) as f32;
        let band_w = (w - 2.0 * inset).max(0.0);
        if band_w > 0.0 {
            // łuk górny
            out.push(Rectangle {
                x: x + inset,
                y: y + row as f32,
                width: band_w,
                height: band,
            });
            // łuk dolny (lustrzane odbicie tego samego pasma)
            out.push(Rectangle {
                x: x + inset,
                y: y + h - row as f32 - band,
                width: band_w,
                height: band,
            });
        }
        row = next;
    }

    let mid = h - 2.0 * steps as f32;
    if mid > 0.0 {
        out.push(Rectangle {
            x,
            y: y + steps as f32,
            width: w,
            height: mid,
        });
    }
}

/// Poziome wcięcie wiersza `row` (licząc od krawędzi) na łuku o promieniu `r`.
fn arc_inset(row: u32, r: f32) -> f32 {
    let dy = r - (row as f32 + 0.5);
    r - (r * r - dy * dy).max(0.0).sqrt()
}

/// Przezroczystość doka z configu COSMIC-a (najpierw ustawienie użytkownika,
/// potem wartość systemowa).
fn dock_opacity() -> Option<f32> {
    let mut roots: Vec<PathBuf> = Vec::with_capacity(2);
    if let Some(config) = dirs::config_dir() {
        roots.push(config);
    }
    roots.push(PathBuf::from("/usr/share"));

    roots.into_iter().find_map(|root| {
        let path = root
            .join("cosmic")
            .join("com.system76.CosmicPanel.Dock")
            .join("v1")
            .join("opacity");
        std::fs::read_to_string(path)
            .ok()?
            .trim()
            .parse::<f32>()
            .ok()
    })
}
