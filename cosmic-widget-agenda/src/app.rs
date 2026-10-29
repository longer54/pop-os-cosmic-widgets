//! Widget statusu harmonogramu — powierzchnia layer-shell z listą
//! najbliższych wydarzeń z prywatnego feedu iCal Kalendarza Google.
//! Ten sam szkielet layer-shell co cosmic-widget-panel / cosmic-calendar-widget
//! (Layer::Bottom, jawny rozmiar + set_size po każdym odświeżeniu).

use std::time::Duration as StdDuration;

use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate, Timelike};
use cosmic::app::{Core, Task};
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{
    IcedMargin, SctkLayerSurfaceSettings,
};
use cosmic::iced::platform_specific::shell::commands::layer_surface::{
    destroy_layer_surface, get_layer_surface, set_margin, set_size, Anchor, KeyboardInteractivity, Layer,
};
use cosmic::iced::window;
use cosmic::iced::{Color, Length, Subscription};
use cosmic::widget::{container, mouse_area, text};
use cosmic::Element;
use cosmic_widget_common::{desktop, drag, glass, Glass, Outcome, WidgetPos};

use crate::config::Config;
use crate::ics::{fetch_agenda, AgendaEvent};

static WEEKDAYS: [&str; 7] = ["Pon", "Wt", "Śr", "Czw", "Pt", "Sob", "Ndz"];

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Fetched(Result<Vec<AgendaEvent>, String>),
    OpenCalendar,
    DragStart,
    Drag(drag::DragEvent),
    StackTick,
    /// Zmiana pulpitu wirtualnego w KWin: `true` = jesteśmy na „swoim"
    /// pulpicie (pokaż powierzchnię), `false` = na innym (schowaj).
    DesktopVisible(bool),
    /// Nowy poziom oszklenia — użytkownik ruszył suwakiem przezroczystości doka.
    Glass(Glass),
}

pub struct Widget {
    core: Core,
    surface_id: window::Id,
    config: Config,
    pos: WidgetPos,
    /// Szkło karty: przezroczystość + region rozmycia tła.
    glass: Glass,
    events: Option<Vec<AgendaEvent>>,
    last_error: Option<String>,
    /// Czy powierzchnia layer-shell istnieje w tej chwili. `false` = jesteśmy
    /// na innym pulpicie wirtualnym i widget jest schowany.
    visible: bool,
}

fn anchor_from_str(s: &str) -> Anchor {
    match s {
        "top-left" => Anchor::TOP | Anchor::LEFT,
        "bottom-left" => Anchor::BOTTOM | Anchor::LEFT,
        "bottom-right" => Anchor::BOTTOM | Anchor::RIGHT,
        _ => Anchor::TOP | Anchor::RIGHT,
    }
}

/// Ustawienia powierzchni layer-shell. Wydzielone, bo widget odtwarza ją po
/// powrocie na swój pulpit wirtualny (`Message::DesktopVisible`), a nie
/// tylko raz przy starcie.
fn surface_settings(
    id: window::Id,
    anchor: &str,
    margin_x: i32,
    margin_y: i32,
    width: u32,
    height: u32,
) -> SctkLayerSurfaceSettings {
    SctkLayerSurfaceSettings {
        id,
        // NIE zmieniać na Layer::Background (sprawdzone 2026-08-11): klik w
        // tapetę podnosi okno pulpitu plasmashella nad tamtą warstwę i widget
        // znika z ekranu. Pokazywaniem się na innych pulpitach zajmuje się
        // `desktop::subscription`, nie warstwa.
        layer: Layer::Bottom,
        keyboard_interactivity: KeyboardInteractivity::None,
        anchor: anchor_from_str(anchor),
        margin: IcedMargin {
            top: margin_y,
            right: margin_x,
            bottom: margin_y,
            left: margin_x,
        },
        size: Some((Some(width), Some(height))),
        exclusive_zone: 0,
        namespace: "cosmic-widget-agenda".into(),
        ..Default::default()
    }
}

fn day_label(d: NaiveDate) -> String {
    let today = Local::now().date_naive();
    if d == today {
        return "Dziś".into();
    }
    if d == today + ChronoDuration::days(1) {
        return "Jutro".into();
    }
    let wd = WEEKDAYS[d.weekday().num_days_from_monday() as usize];
    format!("{} {:02}.{:02}", wd, d.day(), d.month())
}

fn time_label(ev: &AgendaEvent) -> String {
    if ev.all_day {
        "cały dzień".into()
    } else if ev.end.date_naive() == ev.start.date_naive() {
        format!(
            "{:02}:{:02}–{:02}:{:02}",
            ev.start.hour(),
            ev.start.minute(),
            ev.end.hour(),
            ev.end.minute()
        )
    } else {
        format!("{:02}:{:02}", ev.start.hour(), ev.start.minute())
    }
}

impl Widget {
    fn fetch_task(&self) -> Task<Message> {
        let url = self.config.ics_url.clone();
        let days = self.config.days_ahead;
        cosmic::task::future(async move { Message::Fetched(fetch_agenda(url, days).await) })
    }

    /// Szacunkowa wysokość zawartości w px logicznych — powierzchnia
    /// layer-shell musi dostać jawny rozmiar (patrz cosmic-widget-panel).
    fn estimated_height(&self) -> u32 {
        let mut h: u32 = 24 + 30; // padding karty + nagłówek
        if self.last_error.is_some() {
            h += 28;
        }
        match &self.events {
            None => h += 30,
            Some(events) if events.is_empty() => h += 30,
            Some(events) => {
                let visible: Vec<&AgendaEvent> = events.iter().take(self.config.max_events).collect();
                // Tekst wydarzenia zawija się na kilka wierszy (długie tytuły,
                // wklejone linki) — bez doliczenia tych wierszy karta ma za
                // małą JAWNĄ wysokość i ostatnie wydarzenia są po prostu
                // ucięte poza powierzchnią (2026-09-13: na poniedziałek
                // widać było 1 z 2 wydarzeń).
                let summary_cpl = self.chars_per_line(SUMMARY_COL_PX);
                let loc_cpl = self.chars_per_line(0);
                let mut last_day: Option<NaiveDate> = None;
                for ev in &visible {
                    let d = ev.start.date_naive();
                    if last_day != Some(d) {
                        h += 26;
                        last_day = Some(d);
                    }
                    h += 26 + LINE_PX * (wrapped_lines(&ev.summary, summary_cpl) - 1);
                    if let Some(loc) = &ev.location {
                        h += 20 + LINE_PX * (wrapped_lines(loc, loc_cpl) - 1);
                    }
                }
            }
        }
        h.clamp(70, 1200)
    }

    /// Ile znaków mieści się w jednym wierszu tekstu w karcie.
    /// `reserved` = szerokość kolumn przed tekstem (kolumna godziny + odstęp).
    fn chars_per_line(&self, reserved: u32) -> usize {
        // 28 = poziomy padding karty.
        let text_px = self.config.width.saturating_sub(28 + reserved);
        (text_px / AVG_CHAR_PX).max(8) as usize
    }

    /// Wysokość karty i powierzchni layer-shell: tyle, ile treść, ale nie
    /// więcej niż `max_height` z configu — nadmiar przewija się kółkiem
    /// (jak w widgecie notatki), zamiast rozciągać kartę poza ekran.
    fn card_height(&self) -> u32 {
        let content = self.estimated_height();
        match self.config.max_height {
            0 => content,
            limit => content.min(limit.max(70)),
        }
    }

    fn resize_task(&mut self) -> Task<Message> {
        let h = self.card_height();
        self.pos.set_height(h);
        Task::batch([
            set_size(self.surface_id, Some(self.config.width), Some(h)),
            self.blur_task(),
        ])
    }

    /// Region rozmycia tła pod kartą. Kompozytor gubi go razem z
    /// powierzchnią, więc wołamy to po każdym utworzeniu powierzchni i po
    /// każdej zmianie jej rozmiaru.
    fn blur_task(&self) -> Task<Message> {
        glass::blur_task_single(
            self.surface_id,
            self.glass,
            self.config.width,
            self.card_height(),
        )
    }

    /// Zastosuj aktualną pozycję z `self.pos` na powierzchni.
    fn margin_task(&self) -> Task<Message> {
        set_margin(
            self.surface_id,
            self.pos.margin_y(),
            self.pos.margin_x(),
            self.pos.margin_y(),
            self.pos.margin_x(),
        )
    }

    /// Koniec przeciągania: utrwal nową pozycję w configu.
    fn save_position(&mut self) {
        self.config.margin_x = self.pos.margin_x();
        // `margin_y` zapisujemy ZAWSZE, także w stacku: to jedyna kopia
        // pozycji, która przeżyje wyczyszczenie `~/.cache`. Wcześniej
        // stackowany widget zapisywał wyłącznie `stack_offset_y`, więc
        // `margin_y` w configu zostawał skamieliną sprzed miesięcy.
        self.config.margin_y = self.pos.margin_y();
        if self.pos.is_stacked() {
            self.config.stack_offset_y = self.pos.stack_offset_y();
        }
        self.config.save();
        self.pos.publish();
    }
}

impl cosmic::Application for Widget {
    type Executor = cosmic::executor::Default;
    type Flags = Config;
    type Message = Message;
    const APP_ID: &'static str = "io.github.longer54.CosmicWidgetAgenda";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, config: Config) -> (Self, Task<Message>) {
        core.window.show_headerbar = false;
        // Automatyczny blur libcosmica bierze CAŁY prostokąt powierzchni
        // layer-shell — szkło wypełniłoby też przezroczyste szpary między
        // kartami i kwadratowe rogi poza zaokrągleniem. Pusty zestaw = nie
        // proś kompozytora o blur automatycznie; region podajemy sami,
        // dokładnie pod kartami (`glass::blur_task`).
        core.set_auto_blur(Default::default());

        let surface_id = window::Id::unique();
        // Powierzchnia POWSTAJE PO `WidgetPos` i z jego marginesów, nie
        // z surowego configu — patrz `WidgetPos::new` (config bywa
        // nieświeży, prawdziwą pozycję zna opublikowana geometria).
        let pos = WidgetPos::new(
            "agenda",
            &config.anchor,
            config.margin_x,
            config.margin_y,
            config.width,
            Some(config.stack_below.clone()),
            config.stack_gap,
            config.stack_offset_y,
        );
        pos.publish();

        let surface_task = get_layer_surface(surface_settings(
            surface_id,
            &config.anchor,
            pos.margin_x(),
            pos.margin_y(),
            config.width,
            120,
        ));

        let glass = Glass::from_config(&config.glass);

        let widget = Widget {
            core,
            surface_id,
            config,
            pos,
            glass,
            events: None,
            last_error: None,
            visible: true,
        };

        let fetch = widget.fetch_task();
        let blur = widget.blur_task();
        (widget, Task::batch([surface_task, blur, fetch]))
    }

    fn subscription(&self) -> Subscription<Message> {
        let secs = self.config.refresh_secs.max(60);
        let mut subs =
            vec![cosmic::iced::time::every(StdDuration::from_secs(secs)).map(|_| Message::Tick)];
        if self.pos.dragging() {
            subs.push(drag::subscription().map(Message::Drag));
        }
        // Schowany widget nie ma powierzchni, więc nie ma czego przesuwać.
        if self.pos.is_stacked() && self.visible {
            subs.push(
                self.pos.stack_subscription().map(|()| Message::StackTick),
            );
        }
        subs.push(glass::subscription(self.glass.level()).map(Message::Glass));
        if self.config.only_on_desktop > 0 {
            subs.push(
                desktop::subscription(self.config.only_on_desktop).map(Message::DesktopVisible),
            );
        }
        Subscription::batch(subs)
    }

    fn view(&self) -> Element<'_, Message> {
        text::body("").into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if id != self.surface_id {
            return text::body("").into();
        }
        self.build_widget()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => self.fetch_task(),
            Message::Fetched(Ok(events)) => {
                self.events = Some(events);
                self.last_error = None;
                self.resize_task()
            }
            Message::Fetched(Err(e)) => {
                self.last_error = Some(e);
                self.resize_task()
            }
            Message::OpenCalendar => {
                // 2026-10-05: zamiast osobnej appki (cosmic-calendar-webapp) kalendarz
                // otwiera się jako nowa karta w Zenie (użytkownik: "nie chcę już tej appki").
                // Kolejno: Zen z AUR (zen-browser / zen), Zen z Flatpaka, na końcu xdg-open.
                let url = &self.config.calendar_url;
                let proby: [(&str, &[&str]); 3] = [
                    ("zen-browser", &["--new-tab"]),
                    ("zen", &["--new-tab"]),
                    ("flatpak", &["run", "app.zen_browser.zen", "--new-tab"]),
                ];
                // `flatpak` startuje nawet bez zainstalowanego Zena — wtedy go pomijamy.
                let home = dirs::home_dir().unwrap_or_default();
                let zen_flatpak = std::path::Path::new("/var/lib/flatpak/app/app.zen_browser.zen")
                    .exists()
                    || home.join(".local/share/flatpak/app/app.zen_browser.zen").exists();
                let otwarte = proby.iter().any(|(bin, args)| {
                    if *bin == "flatpak" && !zen_flatpak {
                        return false;
                    }
                    std::process::Command::new(bin)
                        .args(*args)
                        .arg(url)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                        .is_ok()
                });
                if !otwarte {
                    eprintln!("cosmic-widget-agenda: brak Zena — otwieram kalendarz przez xdg-open");
                    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
                }
                Task::none()
            }
            Message::DragStart => {
                self.pos.start_drag();
                Task::none()
            }
            Message::Drag(ev) => match self.pos.on_event(&ev, self.surface_id) {
                Outcome::Moved => self.margin_task(),
                Outcome::Released => {
                    self.save_position();
                    self.margin_task()
                }
                Outcome::None => Task::none(),
            },
            Message::Glass(g) => {
                self.glass = g;
                self.blur_task()
            }
            Message::StackTick => {
                if self.pos.stack_tick() {
                    self.pos.publish();
                    self.margin_task()
                } else {
                    Task::none()
                }
            }
            // Zejście na inny pulpit = zniszcz powierzchnię (KWin nie umie
            // przypiąć layer-shella do pulpitu); powrót = odtwórz ją z
            // aktualną pozycją i wysokością.
            Message::DesktopVisible(show) => {
                if show == self.visible {
                    return Task::none();
                }
                self.visible = show;
                if show {
                    self.surface_id = window::Id::unique();
                    Task::batch([
                        get_layer_surface(surface_settings(
                            self.surface_id,
                            &self.config.anchor,
                            self.pos.margin_x(),
                            self.pos.margin_y(),
                            self.config.width,
                            self.card_height(),
                        )),
                        self.blur_task(),
                    ])
                } else {
                    destroy_layer_surface(self.surface_id)
                }
            }
        }
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::iced::theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: Default::default(),
            icon_color: Default::default(),
        })
    }
}

// =========================================================================
// UI
// =========================================================================

const DIM: Color = Color::from_rgba(0.95, 0.95, 0.97, 0.55);
const ACCENT: Color = Color::from_rgb(0.55, 0.75, 0.98);

/// Przybliżona szerokość znaku w px — zaniżona celowo, lepiej parę pikseli
/// zapasu na dole karty niż ucięty wiersz.
const AVG_CHAR_PX: u32 = 7;
/// Wysokość dodatkowego (zawiniętego) wiersza tekstu w px.
const LINE_PX: u32 = 19;
/// Szerokość kolumny godziny (92) wraz z odstępem wiersza (8).
const SUMMARY_COL_PX: u32 = 100;

/// Na ile wierszy zawinie się tekst przy `chars_per_line` znakach w wierszu.
/// Zawijanie po słowach — tak jak robi to silnik tekstu iced.
fn wrapped_lines(text: &str, chars_per_line: usize) -> u32 {
    let mut lines = 1;
    let mut used = 0;
    for word in text.split_whitespace() {
        let w = word.chars().count();
        if used == 0 {
            used = w;
        } else if used + 1 + w <= chars_per_line {
            used += 1 + w;
        } else {
            lines += 1;
            used = w;
        }
        // Słowo dłuższe niż wiersz (długi link) łamie się w środku.
        while used > chars_per_line {
            lines += 1;
            used -= chars_per_line;
        }
    }
    lines
}

fn dim_text() -> cosmic::theme::Text {
    cosmic::theme::Text::Color(DIM)
}

fn accent_text() -> cosmic::theme::Text {
    cosmic::theme::Text::Color(ACCENT)
}

impl Widget {
    fn build_widget(&self) -> Element<'_, Message> {
        let width = self.config.width as f32;
        let mut rows: Vec<Element<'_, Message>> = Vec::new();

        // ---- Nagłówek ----
        // Zostaje NA ZEWNĄTRZ przewijanej listy — tytuł i uchwyt ⠿ mają
        // być zawsze pod kursorem, nie uciekać razem z treścią.
        let header_items: Vec<Element<'_, Message>> = vec![
            drag::handle(Message::DragStart),
            text::heading("Harmonogram").width(Length::Fill).into(),
        ];
        let header: Element<'_, Message> = cosmic::widget::row::with_children(header_items)
            .spacing(8)
            .align_y(cosmic::iced::Alignment::Center)
            .into();

        // ---- Błąd połączenia ----
        if let Some(err) = &self.last_error {
            rows.push(text::caption(format!("brak połączenia: {err}")).class(dim_text()).into());
        }

        // ---- Treść ----
        match &self.events {
            None if self.config.ics_url.is_empty() => {
                rows.push(
                    text::caption(format!("Uzupełnij ics_url w:\n{}", Config::path().display())).into(),
                );
            }
            None => {
                if self.last_error.is_none() {
                    rows.push(text::caption("Ładowanie…").class(dim_text()).into());
                }
            }
            Some(events) if events.is_empty() => {
                rows.push(text::caption("Brak wydarzeń w najbliższych dniach").class(dim_text()).into());
            }
            Some(events) => {
                let visible: Vec<&AgendaEvent> = events.iter().take(self.config.max_events).collect();
                let mut last_day: Option<NaiveDate> = None;
                for ev in visible {
                    let d = ev.start.date_naive();
                    if last_day != Some(d) {
                        rows.push(text::caption_heading(day_label(d)).class(accent_text()).into());
                        last_day = Some(d);
                    }
                    let row = cosmic::widget::row::with_children(vec![
                        text::caption(time_label(ev)).width(Length::Fixed(92.0)).class(dim_text()).into(),
                        text::body(ev.summary.clone()).width(Length::Fill).into(),
                    ])
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center);
                    rows.push(row.into());
                    // tooltip::tooltip renderował się źle w tym layer-shell
                    // surface'ie (pusta ciemna belka zamiast dymka nad
                    // kursorem) — lokalizacja jako osobna, przygaszona linia
                    // zamiast dymka.
                    if let Some(loc) = &ev.location {
                        rows.push(
                            text::caption(format!("     {loc}"))
                                .class(dim_text())
                                .into(),
                        );
                    }
                }
            }
        }

        // Lista w `scrollable` — gdy wydarzeń jest więcej, niż mieści
        // `max_height`, kółko myszy przewija treść zamiast rozpychać kartę
        // poza ekran. Bez widocznego paska, tak jak w widgecie notatki.
        let list = cosmic::widget::scrollable(
            cosmic::widget::column::with_children(rows).spacing(6),
        )
        .scrollbar_width(0.0)
        .scroller_width(0.0)
        .scrollbar_padding(0.0)
        .width(Length::Fill)
        .height(Length::Fill);

        let content = cosmic::widget::column::with_children(vec![
            header,
            container(list).height(Length::Fill).into(),
        ])
        .spacing(6);

        let card = container(content)
            .padding([12, 14])
            .class(cosmic::theme::Container::custom(self.glass.container()))
            .width(Length::Fixed(width))
            // Wysokość JAWNA, taka sama jak wysokość powierzchni i regionu
            // rozmycia (`estimated_height`). Bez tego karta kurczy się do
            // treści, a szkło — liczone z wysokości powierzchni — wystaje
            // spod niej rozmytym paskiem.
            .height(Length::Fixed(self.card_height() as f32));

        mouse_area(card).on_press(Message::OpenCalendar).into()
    }
}

