//! Jedna szuflada — stan powierzchni layer-shell i jej animacji.
//!
//! Wydzielone z `app.rs`, gdy doszła druga szuflada (drzewo vaulta z lewej).
//! Obie działają identycznie i różnią się wyłącznie stroną: powierzchnia
//! obejmuje cały obszar roboczy, karta rysuje się przy swojej krawędzi,
//! a reszta powierzchni to przyciemnione tło łapiące kliknięcie „obok”.

use std::time::{Duration, Instant};

use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{
    IcedMargin, SctkLayerSurfaceSettings,
};
use cosmic::iced::platform_specific::shell::commands::layer_surface::{
    Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface,
};
use cosmic::iced::window;
use cosmic_widget_common::{Glass, glass};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Treść widgetów pulpitowych (pogoda, harmonogram, zadania, kursy).
    Right,
    /// Drzewo katalogów vaulta Obsidiana.
    Left,
}

impl Side {
    pub fn namespace(self) -> &'static str {
        match self {
            Side::Right => "cosmic-widget-drawer",
            Side::Left => "cosmic-widget-drawer-files",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Closed,
    Opening,
    Open,
    Closing,
}

pub struct Panel {
    pub side: Side,
    /// Szerokość karty w px logicznych.
    pub width: u32,
    /// Odstęp karty od krawędzi ekranu.
    pub margin: u32,
    /// Czas wysuwania/chowania w ms; 0 = bez animacji.
    pub animation_ms: u64,

    pub id: Option<window::Id>,
    /// Realny rozmiar powierzchni — podaje go kompozytor (`Resized`), bo to
    /// obszar roboczy MINUS strefy wyłączne panelu i doka, a nie rozmiar
    /// ekranu. Bez niego nie wiadomo, gdzie wyciąć region rozmycia.
    pub surface_size: Option<(f32, f32)>,
    pub phase: Phase,
    anim_start: Instant,
    /// Kiedy szuflada ostatnio się zamknęła — karencja dla gorącego paska
    /// przy krawędzi (patrz `reopen_blocked`).
    closed_at: Instant,
}

impl Panel {
    pub fn new(side: Side, width: u32, margin: u32, animation_ms: u64) -> Self {
        let now = Instant::now();
        Self {
            side,
            width,
            margin,
            animation_ms,
            id: None,
            surface_size: None,
            phase: Phase::Closed,
            anim_start: now,
            // Bezpiecznie „dawno temu”: `Instant` jest monotoniczny od startu
            // systemu, więc odejmowanie może nie mieć dokąd pójść tuż po
            // uruchomieniu maszyny.
            closed_at: now.checked_sub(Duration::from_secs(60)).unwrap_or(now),
        }
    }

    /// Czy szuflada jest na ekranie (także w trakcie wysuwania/chowania).
    pub fn is_open(&self) -> bool {
        !matches!(self.phase, Phase::Closed)
    }

    /// Czy szuflada ma ZOSTAĆ na ekranie. Chowająca się liczy się już jako
    /// zamknięta — inaczej gest zrobiony tuż po poprzednim „zamykałby
    /// zamykane” zamiast otworzyć drugą szufladę.
    pub fn is_showing(&self) -> bool {
        matches!(self.phase, Phase::Open | Phase::Opening)
    }

    /// Czy gorący pasek ma teraz milczeć. Bez tego zamknięcie szuflady
    /// (Esc/klik) z kursorem stojącym przy krawędzi natychmiast dostaje
    /// `on_enter` i szuflada wraca w kółko.
    pub fn reopen_blocked(&self) -> bool {
        self.closed_at.elapsed() < Duration::from_millis(700)
    }

    pub fn animating(&self) -> bool {
        matches!(self.phase, Phase::Opening | Phase::Closing)
    }

    pub fn anim_done(&self) -> bool {
        self.animation_ms == 0
            || self.anim_start.elapsed() >= Duration::from_millis(self.animation_ms)
    }

    /// Ułamek schowania: 0.0 = karta na swoim miejscu, 1.0 = całkiem za
    /// swoją krawędzią. Wygładzone `ease-out`, żeby wjazd wyhamowywał.
    pub fn slide(&self) -> f32 {
        if self.animation_ms == 0 {
            return match self.phase {
                Phase::Closed | Phase::Closing => 1.0,
                _ => 0.0,
            };
        }
        let t =
            (self.anim_start.elapsed().as_millis() as f32 / self.animation_ms as f32).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - t).powi(3);
        match self.phase {
            Phase::Opening => 1.0 - eased,
            Phase::Closing => eased,
            Phase::Open => 0.0,
            Phase::Closed => 1.0,
        }
    }

    /// O ile pikseli karta jest wypchnięta poza swoją krawędź.
    pub fn offset(&self) -> f32 {
        self.slide() * (self.width as f32 + self.margin as f32)
    }

    /// Otwórz. Zwraca komendę tworzącą powierzchnię (albo pustą, gdy
    /// powierzchnia jeszcze żyje po przerwanym chowaniu).
    pub fn open<T: Send + 'static>(&mut self) -> cosmic::app::Task<T> {
        if matches!(self.phase, Phase::Open | Phase::Opening) {
            return cosmic::app::Task::none();
        }
        self.anim_start = Instant::now();
        self.phase = if self.animation_ms > 0 {
            Phase::Opening
        } else {
            Phase::Open
        };
        // Chowanie zostawia powierzchnię do końca animacji — przy szybkim
        // „zamknij i otwórz” jest co pokazać i nie trzeba jej tworzyć od nowa.
        if self.id.is_some() {
            return cosmic::app::Task::none();
        }
        let id = window::Id::unique();
        self.id = Some(id);
        self.surface_size = None;
        get_layer_surface(self.settings(id))
    }

    pub fn close<T: Send + 'static>(&mut self) -> cosmic::app::Task<T> {
        if matches!(self.phase, Phase::Closed | Phase::Closing) {
            return cosmic::app::Task::none();
        }
        self.anim_start = Instant::now();
        self.closed_at = Instant::now();
        if self.animation_ms > 0 {
            self.phase = Phase::Closing;
            cosmic::app::Task::none()
        } else {
            self.phase = Phase::Closed;
            self.destroy()
        }
    }

    /// Koniec animacji — dla chowania znaczy „zabierz powierzchnię”.
    pub fn finish_anim<T: Send + 'static>(&mut self) -> cosmic::app::Task<T> {
        match self.phase {
            Phase::Opening => {
                self.phase = Phase::Open;
                cosmic::app::Task::none()
            }
            Phase::Closing => {
                self.phase = Phase::Closed;
                self.destroy()
            }
            _ => cosmic::app::Task::none(),
        }
    }

    pub fn destroy<T: Send + 'static>(&mut self) -> cosmic::app::Task<T> {
        self.surface_size = None;
        match self.id.take() {
            Some(id) => destroy_layer_surface(id),
            None => cosmic::app::Task::none(),
        }
    }

    /// Region rozmycia dokładnie pod kartą (karta stoi przy swojej krawędzi,
    /// więc `x` nie jest zerem dla prawej szuflady). Kompozytor gubi region
    /// razem z powierzchnią — wołać po jej utworzeniu i po każdym `Resized`.
    pub fn blur<T: Send + 'static>(&self, glass_level: Glass) -> cosmic::app::Task<T> {
        let (Some(id), Some((w, h))) = (self.id, self.surface_size) else {
            return cosmic::app::Task::none();
        };
        let m = self.margin as f32;
        let card_w = self.width as f32;
        let card_h = (h - 2.0 * m).max(0.0);
        let x = match self.side {
            Side::Right => w - m - card_w,
            Side::Left => m,
        };
        glass::blur_task_rects(id, glass_level, &[(x, m, card_w, card_h)])
    }

    /// Powierzchnia na cały obszar roboczy — tylko wtedy widać kliknięcie
    /// POZA kartą (Wayland nie mówi aplikacji o klikach w cudze
    /// powierzchnie). Klawiatura na wyłączność, żeby działał Esc.
    fn settings(&self, id: window::Id) -> SctkLayerSurfaceSettings {
        SctkLayerSurfaceSettings {
            id,
            layer: Layer::Top,
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            margin: IcedMargin::default(),
            // Obie osie „niech zdecyduje kompozytor” + kotwice na
            // przeciwległych krawędziach = powierzchnia na cały obszar roboczy.
            size: Some((None, None)),
            exclusive_zone: 0,
            namespace: self.side.namespace().into(),
            ..Default::default()
        }
    }
}

/// Gorący pasek przy prawej krawędzi — kilka pikseli szerokości, wysokość
/// z configu, wyśrodkowany w pionie. Warstwa `Top`, żeby łapał kursor także
/// nad zmaksymalizowanym oknem.
///
/// Tylko prawa strona: lewą krawędź ekranu zajmuje u autora dok, więc pasek
/// i tak nie miałby gdzie stanąć.
pub fn edge_settings(id: window::Id, width: u32, height: u32) -> SctkLayerSurfaceSettings {
    SctkLayerSurfaceSettings {
        id,
        layer: Layer::Top,
        keyboard_interactivity: KeyboardInteractivity::None,
        anchor: Anchor::RIGHT,
        margin: IcedMargin::default(),
        size: Some((Some(width), Some(height))),
        exclusive_zone: 0,
        namespace: "cosmic-widget-drawer-edge".into(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{Panel, Phase, Side};

    #[test]
    fn zamknieta_szuflada_jest_za_krawedzia() {
        let p = Panel::new(Side::Right, 420, 10, 140);
        assert_eq!(p.slide(), 1.0);
        assert!(!p.is_open());
        assert_eq!(p.offset(), 430.0);
    }

    #[test]
    fn bez_animacji_otwiera_sie_od_razu() {
        let mut p = Panel::new(Side::Left, 300, 8, 0);
        let _: cosmic::app::Task<()> = p.open();
        assert_eq!(p.phase, Phase::Open);
        assert_eq!(p.slide(), 0.0);
        assert!(p.is_open());
    }

    #[test]
    fn chowajaca_sie_szuflada_nie_liczy_sie_jako_pokazana() {
        let mut p = Panel::new(Side::Left, 340, 10, 500);
        let _: cosmic::app::Task<()> = p.open();
        let _: cosmic::app::Task<()> = p.close();
        assert_eq!(p.phase, Phase::Closing);
        assert!(p.is_open(), "powierzchnia jeszcze żyje");
        assert!(!p.is_showing(), "ale gest ma ją traktować jak zamkniętą");
    }

    #[test]
    fn karencja_po_zamknieciu_blokuje_krawedz() {
        let mut p = Panel::new(Side::Right, 420, 10, 0);
        let _: cosmic::app::Task<()> = p.open();
        let _: cosmic::app::Task<()> = p.close();
        assert!(p.reopen_blocked());
    }
}
