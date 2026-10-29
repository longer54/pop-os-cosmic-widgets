//! Stan pozycji widgetu: efektywne marginesy layer-shell, przeciąganie
//! i dopasowanie do rodzica w kolumnie (stack).

use std::time::{Duration, Instant};

use cosmic::iced::window;

use crate::drag::DragEvent;
use crate::geometry::{self, Geometry};
use crate::screen;

/// Eventy myszy mierzą pozycję względem POWIERZCHNI, a kompozytor
/// przesuwa powierzchnię dopiero przy klatce (~16 ms). Korekta liczona
/// na KAŻDYM evencie (nawet kilkaset/s) integruje wielokrotnie ten sam,
/// jeszcze niezastosowany błąd — pętla sprzężenia rozbiega się i widget
/// odlatuje poza ekran (2026-07-14, dwa razy: gain 1.0 i gain 0.25).
/// Dlatego korekta idzie najwyżej co DRAG_STEP_INTERVAL: poprzedni krok
/// zdąży dojechać do kompozytora, zanim zmierzymy kolejny, a pomiar
/// zawsze jest świeżą pozycją względem punktu chwytu (nic nie ginie).
/// Dobór pary interval/gain: symulacja siatki (lag 16–60 ms, mysz
/// 125–1000 Hz, kursor 200–2000 px/s) — 20 ms + 0.4 ma przestrzał
/// ≤ ~46 px w najgorszym przypadku i ~50 aktualizacji/s (płynnie);
/// 40 ms było stabilne, ale szarpało (~25 FPS), a 16 ms + 0.5 się
/// rozbujało (przestrzał ~370 px).
const DRAG_STEP_INTERVAL: Duration = Duration::from_millis(20);
/// Ile zmierzonego odchylenia nadrabiamy jednym krokiem.
const DRAG_GAIN: f64 = 0.4;
/// Twardy limit pojedynczego kroku (px) — bezpiecznik przed teleportacją.
const DRAG_MAX_STEP: f64 = 150.0;

pub struct WidgetPos {
    name: String,
    anchor: String,
    anchor_right: bool,
    anchor_bottom: bool,
    /// Marginesy jako f64 — delty myszy są ułamkowe, trzymanie i32
    /// gubiłoby powolne ruchy przez zaokrąglanie co event.
    fx: f64,
    fy: f64,
    width: u32,
    height: u32,
    stack_below: Option<String>,
    stack_gap: i32,
    stack_offset_y: i32,
    dragging: bool,
    grab: Option<(f32, f32)>,
    last_step: Option<Instant>,
    /// Czy `stack_offset_y` został już przeliczony na nową semantykę bazy
    /// („góra rodzica”, 2026-08-28). Robimy to leniwie przy pierwszym
    /// ticku, gdy geometria rodzica jest już dostępna — inaczej wyścig
    /// startowy (dziecko rusza przed publikacją rodzica) teleportowałby
    /// widget. Do tego czasu dziecko stoi tam, gdzie było w zeszłej sesji.
    stack_migrated: bool,
    /// Logiczny rozmiar ekranu — granica dla marginesów. `None` = nie
    /// udało się go ustalić, wtedy nie ograniczamy (jak przed 2026-08-26).
    screen: Option<(u32, u32)>,
    /// Testowa podmiana geometrii rodzica (bez dotykania `~/.cache`).
    #[cfg(test)]
    test_parent: Option<Geometry>,
}

pub enum Outcome {
    /// Nic do zrobienia.
    None,
    /// Pozycja się zmieniła — wywołaj set_margin.
    Moved,
    /// Koniec przeciągania — set_margin + zapis configa + publish.
    Released,
}

impl WidgetPos {
    /// WAŻNE dla wywołujących: powierzchnię layer-shell twórz DOPIERO po
    /// `WidgetPos::new` i z `margin_x()`/`margin_y()`, nigdy wprost z
    /// `config.margin_*`. Config to tylko punkt wyjścia — `new` poprawia go
    /// o realną pozycję z zeszłej sesji (`~/.cache/.../geometry`) i o
    /// przycięcie do ekranu. Gdy powierzchnia startowała z surowego configu
    /// (tak było do 2026-08-29), widget rysował się w jednym miejscu, a cała
    /// arytmetyka stacku liczyła go w drugim: nic nigdy nie wołało
    /// `set_margin`, żeby je pogodzić (`stack_tick` milczy, dopóki pozycja
    /// zgadza się z jego rachunkiem). Efekt: po każdym przelogowaniu widgety
    /// wracały na skamieniałe `margin_y` z configu i nachodziły na siebie.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: &str,
        anchor: &str,
        margin_x: i32,
        margin_y: i32,
        width: u32,
        stack_below: Option<String>,
        stack_gap: i32,
        stack_offset_y: i32,
    ) -> Self {
        let mut pos = Self {
            name: name.into(),
            anchor: anchor.into(),
            anchor_right: anchor.contains("right"),
            anchor_bottom: anchor.contains("bottom"),
            fx: margin_x as f64,
            fy: margin_y as f64,
            width,
            height: 90,
            stack_below: stack_below.filter(|s| !s.is_empty()),
            stack_gap,
            stack_offset_y,
            dragging: false,
            grab: None,
            last_step: None,
            stack_migrated: false,
            screen: screen::size(),
            #[cfg(test)]
            test_parent: None,
        };
        // Stackowane widgety zapisują w configu `stack_offset_y`, a nie
        // aktualny `margin_y` — więc `margin_y` z configu bywa nieświeży.
        // Wystartuj tam, gdzie widget realnie był w zeszłej sesji (jego
        // własna opublikowana geometria trwa w `~/.cache`). Ostateczną
        // relację do rodzica zamraża pierwszy `stack_tick` (`stack_migrated`).
        if pos.stack_below.is_some() {
            if let Some(g) = geometry::read(name) {
                pos.fy = f64::from(g.margin_y);
            }
        }
        // Config mógł zapamiętać margines z czasów bez ograniczenia
        // (runaway przy krawędzi) — przytnij od razu, żeby widget nie
        // wystartował „przyklejony” i nie do odzyskania myszką.
        pos.clamp();
        pos
    }

    /// Nie pozwól marginesom wyjechać poza ekran. Bez tego pętla
    /// przeciągania integruje odchylenie, którego kompozytor już nie
    /// realizuje (patrz `screen`), i margines rośnie bez końca.
    fn clamp(&mut self) {
        self.fx = self.fx.max(0.0);
        self.fy = self.fy.max(0.0);
        if let Some((sw, sh)) = self.screen {
            self.fx = self.fx.min(f64::from(sw.saturating_sub(self.width)));
            self.fy = self.fy.min(f64::from(sh.saturating_sub(self.height)));
        }
    }

    pub fn margin_x(&self) -> i32 {
        self.fx.round() as i32
    }

    pub fn margin_y(&self) -> i32 {
        self.fy.round() as i32
    }

    pub fn dragging(&self) -> bool {
        self.dragging
    }

    pub fn is_stacked(&self) -> bool {
        self.stack_below.is_some()
    }

    /// Subskrypcja ticku stacku: budzi widget tylko przy zmianie rodzica.
    #[must_use]
    pub fn stack_subscription(&self) -> cosmic::iced::Subscription<()> {
        match &self.stack_below {
            Some(parent) => crate::geometry::watch(parent.clone()),
            None => cosmic::iced::Subscription::none(),
        }
    }

    pub fn stack_offset_y(&self) -> i32 {
        self.stack_offset_y
    }

    pub fn start_drag(&mut self) {
        // Rozdzielczość mogła się zmienić od startu widgetu (dokowanie,
        // drugi monitor) — odśwież przed przeciąganiem, nie w kółko.
        self.screen = screen::size();
        self.dragging = true;
        self.grab = None;
        self.last_step = None;
    }

    /// Obsłuż event z `drag::subscription()`. Punkt chwytu to pozycja
    /// kursora przy pierwszym ruchu; kolejne delty liczone są względem
    /// niego — gdy powierzchnia jedzie za kursorem, pozycja względna
    /// wraca do punktu chwytu, więc korekta jest samonaprowadzająca.
    pub fn on_event(&mut self, ev: &DragEvent, surface: window::Id) -> Outcome {
        if !self.dragging {
            return Outcome::None;
        }
        match ev {
            DragEvent::Moved { window, x, y } if *window == surface => {
                self.on_moved(Instant::now(), *x, *y)
            }
            DragEvent::Released => {
                self.dragging = false;
                self.grab = None;
                // Widget w stacku: pion zapamiętujemy jako offset od
                // pozycji wyliczonej z rodzica, żeby auto-rozsuwanie
                // dalej działało po ręcznej korekcie.
                if let Some(base) = self.stack_base() {
                    self.stack_offset_y = self.margin_y() - base;
                }
                Outcome::Released
            }
            _ => Outcome::None,
        }
    }

    /// Jeden krok korekty; `now` osobno, żeby testy mogły symulować czas.
    fn on_moved(&mut self, now: Instant, x: f32, y: f32) -> Outcome {
        let Some((gx, gy)) = self.grab else {
            self.grab = Some((x, y));
            return Outcome::None;
        };
        // Eventy między krokami po prostu przepadają — kolejny krok i tak
        // liczy odchylenie od punktu chwytu ze świeżego pomiaru.
        if self
            .last_step
            .is_some_and(|t| now.duration_since(t) < DRAG_STEP_INTERVAL)
        {
            return Outcome::None;
        }
        let step = |d: f64| (d * DRAG_GAIN).clamp(-DRAG_MAX_STEP, DRAG_MAX_STEP);
        let (dx, dy) = (step((x - gx) as f64), step((y - gy) as f64));
        if dx == 0.0 && dy == 0.0 {
            return Outcome::None;
        }
        self.last_step = Some(now);
        if self.anchor_right {
            self.fx -= dx;
        } else {
            self.fx += dx;
        }
        if self.anchor_bottom {
            self.fy -= dy;
        } else {
            self.fy += dy;
        }
        self.clamp();
        Outcome::Moved
    }

    /// Punkt odniesienia w kolumnie: GÓRA rodzica (`margin_y`) + stały
    /// odstęp. NIE dół rodzica — dzięki temu zmiana jego WYSOKOŚCI
    /// (odświeżenie pogody, przybycie zadania, przeliczenie
    /// `estimated_height`) już nie przesuwa dziecka. Widget rusza się tylko
    /// gdy rodzic realnie zmieni pozycję (przeciągnięcie). To był powód
    /// „rozjeżdżania się” widgetów co jakiś czas (2026-08-28). Pełną
    /// relację pionową trzyma `stack_offset_y` — ustawiany przy starcie
    /// (migracja) i po każdym dragu.
    fn stack_base(&self) -> Option<i32> {
        let parent = self.stack_below.as_deref()?;
        #[cfg(test)]
        if let Some(pg) = &self.test_parent {
            return Some(pg.margin_y + self.stack_gap);
        }
        let pg = geometry::read(parent)?;
        Some(pg.margin_y + self.stack_gap)
    }

    /// Tick stacku (co ~2 s): trzymaj stałą odległość od GÓRY rodzica.
    /// true = pozycja się zmieniła (wywołaj set_margin + publish).
    pub fn stack_tick(&mut self) -> bool {
        if self.dragging {
            return false;
        }
        let Some(base) = self.stack_base() else {
            return false;
        };
        if !self.stack_migrated {
            // Pierwszy tick z widoczną geometrią rodzica: zamroź bieżącą
            // pozycję jako offset od GÓRY rodzica (zmiana semantyki bazy
            // 2026-08-28 z „dół rodzica” na „góra rodzica”). Nic nie ruszamy.
            self.stack_offset_y = self.margin_y() - base;
            self.stack_migrated = true;
            return false;
        }
        let want = (base + self.stack_offset_y).max(0);
        if want != self.margin_y() {
            self.fy = want as f64;
            self.clamp();
            true
        } else {
            false
        }
    }

    /// Zapamiętaj wysokość (po każdym resize) i opublikuj geometrię.
    pub fn set_height(&mut self, h: u32) {
        self.height = h;
        self.clamp();
        self.publish();
    }

    pub fn publish(&self) {
        geometry::publish(
            &self.name,
            &Geometry {
                anchor: self.anchor.clone(),
                margin_x: self.margin_x(),
                margin_y: self.margin_y(),
                width: self.width,
                height: self.height,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Symulacja przeciągania z opóźnieniem kompozytora: skomandowany
    /// margines jest widoczny w pomiarach dopiero po `lag`. Kursor
    /// (globalnie) jedzie w dół 2 px/event przez pół trasy, potem stoi,
    /// ale eventy dalej przychodzą. Zwraca (margines końcowy, maksymalny).
    fn simulate(events_hz: u64, lag: Duration) -> (f64, f64) {
        const START: f64 = 100.0;
        let mut pos = WidgetPos::new("test", "top-left", START as i32, START as i32, 300, None, 0, 0);
        pos.start_drag();
        // Test ma być niezależny od ekranu maszyny, na której leci.
        pos.screen = None;
        let t0 = Instant::now();
        // (kiedy skomandowano, margines) — pomiar widzi ostatni wpis
        // starszy niż lag.
        let mut cmds: Vec<(Instant, f64)> = vec![(t0, START)];
        let applied = |cmds: &[(Instant, f64)], now: Instant| {
            cmds.iter()
                .rev()
                .find(|(t, _)| now.duration_since(*t) >= lag)
                .map(|(_, m)| *m)
                .unwrap_or(cmds[0].1)
        };
        let dt = Duration::from_micros(1_000_000 / events_hz);
        let mut cursor = 500.0_f64;
        let mut max_m = START;
        for i in 0u32..600 {
            let now = t0 + dt * (i + 1);
            if i < 300 {
                cursor += 2.0;
            }
            let rel_y = cursor - applied(&cmds, now);
            if let Outcome::Moved = pos.on_moved(now, 10.0, rel_y as f32) {
                cmds.push((now, pos.margin_y() as f64));
            }
            max_m = max_m.max(pos.margin_y() as f64);
        }
        (pos.margin_y() as f64, max_m)
    }

    /// Szybka mysz (1000 Hz) + lag 25 ms — dokładnie warunki, w których
    /// widget odlatywał poza ekran 2026-07-14. Kursor przesunął się
    /// o 600 px; widget ma wylądować obok i nigdzie nie odlecieć.
    #[test]
    fn drag_stable_fast_mouse_with_lag() {
        let (final_m, max_m) = simulate(1000, Duration::from_millis(25));
        let target = 100.0 + 600.0;
        assert!(
            (final_m - target).abs() < 20.0,
            "margines końcowy {final_m}, oczekiwane ~{target}"
        );
        assert!(max_m < target + 60.0, "przestrzelenie: max {max_m}");
    }

    /// Wolniejszy kompozytor (lag 40 ms) — dalej bez rozbujania.
    #[test]
    fn drag_stable_slow_compositor() {
        let (final_m, max_m) = simulate(1000, Duration::from_millis(40));
        let target = 100.0 + 600.0;
        assert!(
            (final_m - target).abs() < 20.0,
            "margines końcowy {final_m}, oczekiwane ~{target}"
        );
        assert!(max_m < target + 60.0, "przestrzelenie: max {max_m}");
    }

    /// Przy krawędzi ekranu kompozytor przestaje przesuwać powierzchnię,
    /// więc odchylenie kursora nie maleje i pętla dolicza margines bez
    /// końca (tak powstał `margin_x = 12691` przy ekranie 1920 px —
    /// widget przykleił się do lewej i nie dało się go ściągnąć z
    /// powrotem). Z ograniczeniem margines staje na krawędzi.
    #[test]
    fn margines_nie_ucieka_poza_ekran() {
        let mut pos = WidgetPos::new("test", "top-left", 0, 0, 300, None, 0, 0);
        pos.screen = Some((1920, 1080));
        pos.set_height(200);
        pos.start_drag();
        pos.screen = Some((1920, 1080));

        // Kursor ucieka w prawo/w dół, a powierzchnia stoi (zaklinowana
        // na krawędzi) — odchylenie rośnie i nigdy nie wraca do zera.
        let t0 = Instant::now();
        for i in 1u32..=500 {
            let now = t0 + DRAG_STEP_INTERVAL * i;
            let d = i as f32 * 40.0;
            pos.on_moved(now, d, d);
        }

        assert_eq!(pos.margin_x(), 1920 - 300);
        assert_eq!(pos.margin_y(), 1080 - 200);
    }

    /// Sedno fixu z 2026-08-28: dziecko w kolumnie trzyma się GÓRY rodzica,
    /// nie jego dolnej krawędzi. Zmiana wysokości rodzica (odświeżenie
    /// danych) NIE rusza dziecka; dopiero przesunięcie rodzica — tak.
    #[test]
    fn stack_trzyma_gore_rodzica_nie_jego_wysokosci() {
        let mut pos =
            WidgetPos::new("test-child", "top-left", 0, 300, 300, Some("test-parent".into()), 20, 0);
        pos.screen = None;
        pos.fy = 300.0; // pozycja „na ekranie” z zeszłej sesji
        // Rodzic: góra 100, wysokość 200.
        pos.test_parent = Some(Geometry {
            anchor: "top-left".into(),
            margin_x: 0,
            margin_y: 100,
            width: 300,
            height: 200,
        });

        // Pierwszy tick tylko zamraża relację (offset = 300 - (100+20) = 180) i nic nie rusza.
        assert!(!pos.stack_tick());
        assert_eq!(pos.stack_offset_y, 180);
        assert_eq!(pos.margin_y(), 300);

        // Rodzic ROŚNIE o 300 px w dół — dziecko ani drgnie.
        pos.test_parent.as_mut().unwrap().height = 500;
        assert!(!pos.stack_tick(), "wzrost wysokości rodzica nie rusza dziecka");
        assert_eq!(pos.margin_y(), 300);

        // Rodzic PRZESUNIĘTY w dół o 50 px (przeciągnięcie) — dziecko jedzie za nim.
        pos.test_parent.as_mut().unwrap().margin_y = 150;
        assert!(pos.stack_tick(), "przesunięcie rodzica rusza dziecko");
        assert_eq!(pos.margin_y(), 350);
    }

    /// Margines zapamiętany w configu z czasów bez ograniczenia ma zostać
    /// naprawiony już przy starcie widgetu.
    #[test]
    fn config_z_runaway_jest_przycinany_przy_starcie() {
        let mut pos = WidgetPos::new("test", "top-right", 12_691, 580, 460, None, 0, 0);
        pos.screen = Some((1920, 1080));
        pos.clamp();
        assert_eq!(pos.margin_x(), 1920 - 460);
    }

    /// Typowy touchpad (125 Hz) — też ma być stabilnie.
    #[test]
    fn drag_stable_touchpad() {
        let (final_m, max_m) = simulate(125, Duration::from_millis(25));
        let target = 100.0 + 600.0;
        assert!(
            (final_m - target).abs() < 20.0,
            "margines końcowy {final_m}, oczekiwane ~{target}"
        );
        assert!(max_m < target + 60.0, "przestrzelenie: max {max_m}");
    }
}
