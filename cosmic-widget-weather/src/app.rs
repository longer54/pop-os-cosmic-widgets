//! Widget pogody — powierzchnia layer-shell z aktualnym odczytem z
//! prawdziwej stacji IMGW w Lublinie i krótką prognozą z Open-Meteo.
//! Ten sam szkielet layer-shell co pozostałe widgety (Layer::Bottom,
//! jawny rozmiar + set_size po każdym odświeżeniu, karta w stylu szkła).

use std::time::Duration as StdDuration;

use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate};
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
use crate::weather::{fetch_weather, wind_compass, wmo_desc, WeatherData};

static WEEKDAYS: [&str; 7] = ["Pon", "Wt", "Śr", "Czw", "Pt", "Sob", "Ndz"];

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Fetched(Result<WeatherData, String>),
    OpenWeather,
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
    data: Option<WeatherData>,
    last_error: Option<String>,
    /// Czy powierzchnia layer-shell istnieje w tej chwili. `false` = jesteśmy
    /// na innym pulpicie wirtualnym i widget jest schowany.
    visible: bool,
}

fn anchor_from_str(s: &str) -> Anchor {
    match s {
        "top-right" => Anchor::TOP | Anchor::RIGHT,
        "bottom-left" => Anchor::BOTTOM | Anchor::LEFT,
        "bottom-right" => Anchor::BOTTOM | Anchor::RIGHT,
        _ => Anchor::TOP | Anchor::LEFT,
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
        namespace: "cosmic-widget-weather".into(),
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

impl Widget {
    fn fetch_task(&self) -> Task<Message> {
        let lat = self.config.lat;
        let lon = self.config.lon;
        let station = self.config.imgw_station.clone();
        let days = self.config.forecast_days;
        cosmic::task::future(async move { Message::Fetched(fetch_weather(lat, lon, station, days).await) })
    }

    /// Szacunkowa wysokość zawartości w px logicznych — powierzchnia
    /// layer-shell musi dostać jawny rozmiar (patrz cosmic-widget-panel).
    /// Wiersze prognozy liczone hojnie (36px), żeby nie przycinać treści
    /// (nauczka z cosmic-widget-tasks: realna wysokość wiersza bywa większa
    /// niż się wydaje).
    fn estimated_height(&self) -> u32 {
        let mut h: u32 = 24 + 30; // padding karty + nagłówek
        if self.last_error.is_some() {
            h += 24;
        }
        match &self.data {
            None => h += 30,
            Some(data) => {
                h += match &data.warning {
                    Some(_) => 24,
                    None => 0,
                };
                h += if data.current.is_some() { 76 } else { 26 };
                h += if data.forecast.is_empty() { 26 } else { 8 + 36 * data.forecast.len() as u32 };
            }
        }
        h.clamp(90, 900)
    }

    fn resize_task(&mut self) -> Task<Message> {
        let h = self.estimated_height();
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
            self.estimated_height(),
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
    const APP_ID: &'static str = "io.github.longer54.CosmicWidgetWeather";

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
            "weather",
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
            160,
        ));

        let glass = Glass::from_config(&config.glass);

        let widget = Widget {
            core,
            surface_id,
            config,
            pos,
            glass,
            data: None,
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
            Message::Fetched(Ok(data)) => {
                self.data = Some(data);
                self.last_error = None;
                self.resize_task()
            }
            Message::Fetched(Err(e)) => {
                self.last_error = Some(e);
                self.resize_task()
            }
            Message::OpenWeather => {
                let _ = std::process::Command::new("xdg-open")
                    .arg(&self.config.weather_url)
                    .spawn();
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
                            self.estimated_height(),
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
        let header_items: Vec<Element<'_, Message>> = vec![
            drag::handle(Message::DragStart),
            text::heading("Pogoda — Lublin").width(Length::Fill).into(),
        ];
        rows.push(
            cosmic::widget::row::with_children(header_items)
                .spacing(8)
                .align_y(cosmic::iced::Alignment::Center)
                .into(),
        );

        // ---- Błąd połączenia (całkowity) ----
        if let Some(err) = &self.last_error {
            rows.push(text::caption(format!("brak połączenia: {err}")).class(dim_text()).into());
        }

        match &self.data {
            None => {
                if self.last_error.is_none() {
                    rows.push(text::caption("Ładowanie…").class(dim_text()).into());
                }
            }
            Some(data) => {
                // ---- Ostrzeżenie częściowe (jedno z dwóch API padło) ----
                if let Some(w) = &data.warning {
                    rows.push(text::caption(w.clone()).class(dim_text()).into());
                }

                // ---- Aktualne odczyty ze stacji IMGW ----
                match &data.current {
                    Some(cur) => {
                        rows.push(
                            cosmic::widget::row::with_children(vec![
                                text::title2(format!("{:.0}°C", cur.temperature)).into(),
                                text::caption(format!("stacja Lublin, godz. {}", cur.measured_hour))
                                    .class(dim_text())
                                    .into(),
                            ])
                            .spacing(10)
                            .align_y(cosmic::iced::Alignment::Center)
                            .into(),
                        );
                        let mut stats = format!(
                            "Wiatr {:.0} m/s {} · Wilgotność {:.0}% · Ciśnienie {:.0} hPa",
                            cur.wind_speed,
                            wind_compass(cur.wind_dir_deg),
                            cur.humidity,
                            cur.pressure
                        );
                        if cur.precip_sum > 0.0 {
                            stats.push_str(&format!(" · Opad {:.1} mm", cur.precip_sum));
                        }
                        rows.push(text::caption(stats).class(dim_text()).into());
                    }
                    None => {
                        rows.push(text::caption("Brak odczytu ze stacji").class(dim_text()).into());
                    }
                }

                // ---- Prognoza ----
                if data.forecast.is_empty() {
                    rows.push(text::caption("Brak prognozy").class(dim_text()).into());
                } else {
                    for day in &data.forecast {
                        let row = cosmic::widget::row::with_children(vec![
                            text::caption(day_label(day.date)).width(Length::Fixed(56.0)).class(accent_text()).into(),
                            text::body(wmo_desc(day.code)).width(Length::Fill).into(),
                            text::caption(format!("{:.0}°/{:.0}°", day.tmax, day.tmin))
                                .width(Length::Fixed(60.0))
                                .into(),
                            text::caption(format!("{:.0}%", day.precip_prob))
                                .width(Length::Fixed(38.0))
                                .class(dim_text())
                                .into(),
                        ])
                        .spacing(6)
                        .align_y(cosmic::iced::Alignment::Center);
                        rows.push(row.into());
                    }
                }
            }
        }

        let content = cosmic::widget::column::with_children(rows).spacing(6);

        let card = container(content)
            .padding([12, 14])
            .class(cosmic::theme::Container::custom(self.glass.container()))
            .width(Length::Fixed(width))
            // Wysokość JAWNA, taka sama jak wysokość powierzchni i regionu
            // rozmycia (`estimated_height`). Bez tego karta kurczy się do
            // treści, a szkło — liczone z wysokości powierzchni — wystaje
            // spod niej rozmytym paskiem.
            .height(Length::Fixed(self.estimated_height() as f32));

        mouse_area(card).on_press(Message::OpenWeather).into()
    }
}

