//! Widget pogody — powierzchnia layer-shell z aktualnym odczytem z
//! prawdziwej stacji IMGW w Lublinie i krótką prognozą z Open-Meteo.
//! Ten sam szkielet layer-shell co pozostałe widgety (Layer::Bottom,
//! jawny rozmiar + set_size po każdym odświeżeniu, karta w stylu szkła).

use std::time::Duration as StdDuration;

use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate};
use cosmic::app::{Core, Task};
use cosmic::iced::border::Border;
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{
    IcedMargin, SctkLayerSurfaceSettings,
};
use cosmic::iced::platform_specific::shell::commands::layer_surface::{
    get_layer_surface, set_size, Anchor, KeyboardInteractivity, Layer,
};
use cosmic::iced::window;
use cosmic::iced::{Background, Color, Length, Shadow, Subscription, Vector};
use cosmic::widget::{container, mouse_area, text};
use cosmic::Element;

use crate::config::Config;
use crate::weather::{fetch_weather, wind_compass, wmo_desc, WeatherData};

static WEEKDAYS: [&str; 7] = ["Pon", "Wt", "Śr", "Czw", "Pt", "Sob", "Ndz"];

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Fetched(Result<WeatherData, String>),
    OpenWeather,
}

pub struct Widget {
    core: Core,
    surface_id: window::Id,
    config: Config,
    data: Option<WeatherData>,
    last_error: Option<String>,
    last_update: Option<String>,
}

fn anchor_from_str(s: &str) -> Anchor {
    match s {
        "top-right" => Anchor::TOP | Anchor::RIGHT,
        "bottom-left" => Anchor::BOTTOM | Anchor::LEFT,
        "bottom-right" => Anchor::BOTTOM | Anchor::RIGHT,
        _ => Anchor::TOP | Anchor::LEFT,
    }
}

fn now_hhmm() -> String {
    Local::now().format("%H:%M").to_string()
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

    fn resize_task(&self) -> Task<Message> {
        set_size(self.surface_id, Some(self.config.width), Some(self.estimated_height()))
    }
}

impl cosmic::Application for Widget {
    type Executor = cosmic::executor::Default;
    type Flags = Config;
    type Message = Message;
    const APP_ID: &'static str = "com.example.CosmicWidgetWeather";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, config: Config) -> (Self, Task<Message>) {
        core.window.show_headerbar = false;

        let surface_id = window::Id::unique();
        let anchor = anchor_from_str(&config.anchor);
        let margin = IcedMargin {
            top: config.margin_y,
            right: config.margin_x,
            bottom: config.margin_y,
            left: config.margin_x,
        };

        let surface_task = get_layer_surface(SctkLayerSurfaceSettings {
            id: surface_id,
            layer: Layer::Bottom,
            keyboard_interactivity: KeyboardInteractivity::None,
            anchor,
            margin,
            size: Some((Some(config.width), Some(160))),
            exclusive_zone: 0,
            namespace: "cosmic-widget-weather".into(),
            ..Default::default()
        });

        let widget = Widget {
            core,
            surface_id,
            config,
            data: None,
            last_error: None,
            last_update: None,
        };

        let fetch = widget.fetch_task();
        (widget, Task::batch([surface_task, fetch]))
    }

    fn subscription(&self) -> Subscription<Message> {
        let secs = self.config.refresh_secs.max(60);
        cosmic::iced::time::every(StdDuration::from_secs(secs)).map(|_| Message::Tick)
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
                self.last_update = Some(now_hhmm());
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
        let mut header_items: Vec<Element<'_, Message>> =
            vec![text::heading("Pogoda — Lublin").width(Length::Fill).into()];
        if let Some(ts) = &self.last_update {
            header_items.push(text::caption(ts.clone()).class(dim_text()).into());
        }
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
            .class(cosmic::theme::Container::custom(glass_style))
            .width(Length::Fixed(width));

        mouse_area(card).on_press(Message::OpenWeather).into()
    }
}

/// Ciemna półprzezroczysta karta w stylu szkła — jak w pozostałych widgetach.
fn glass_style(_theme: &cosmic::Theme) -> cosmic::iced::widget::container::Style {
    cosmic::iced::widget::container::Style {
        icon_color: Some(Color::from_rgb(0.95, 0.95, 0.97)),
        text_color: Some(Color::from_rgb(0.95, 0.95, 0.97)),
        background: Some(Background::Color(Color::from_rgba(0.08, 0.08, 0.1, 0.55))),
        border: Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.14),
            width: 1.0,
            radius: 18.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
            offset: Vector::new(0.0, 6.0),
            blur_radius: 24.0,
        },
        snap: true,
    }
}
