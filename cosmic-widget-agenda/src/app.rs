//! Widget statusu harmonogramu — powierzchnia layer-shell z listą
//! najbliższych wydarzeń z prywatnego feedu iCal Kalendarza Google.
//! Ten sam szkielet layer-shell co cosmic-widget-panel / cosmic-calendar-widget
//! (Layer::Bottom, jawny rozmiar + set_size po każdym odświeżeniu).

use std::time::Duration as StdDuration;

use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate, Timelike};
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
use cosmic::widget::{container, mouse_area, text, tooltip};
use cosmic::Element;

use crate::config::Config;
use crate::ics::{fetch_agenda, AgendaEvent};

static WEEKDAYS: [&str; 7] = ["Pon", "Wt", "Śr", "Czw", "Pt", "Sob", "Ndz"];

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Fetched(Result<Vec<AgendaEvent>, String>),
    OpenCalendar,
}

pub struct Widget {
    core: Core,
    surface_id: window::Id,
    config: Config,
    events: Option<Vec<AgendaEvent>>,
    last_error: Option<String>,
    last_update: Option<String>,
}

fn anchor_from_str(s: &str) -> Anchor {
    match s {
        "top-left" => Anchor::TOP | Anchor::LEFT,
        "bottom-left" => Anchor::BOTTOM | Anchor::LEFT,
        "bottom-right" => Anchor::BOTTOM | Anchor::RIGHT,
        _ => Anchor::TOP | Anchor::RIGHT,
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
                let mut last_day: Option<NaiveDate> = None;
                for ev in &visible {
                    let d = ev.start.date_naive();
                    if last_day != Some(d) {
                        h += 26;
                        last_day = Some(d);
                    }
                    h += 26;
                }
            }
        }
        h.clamp(70, 1200)
    }

    fn resize_task(&self) -> Task<Message> {
        set_size(self.surface_id, Some(self.config.width), Some(self.estimated_height()))
    }
}

impl cosmic::Application for Widget {
    type Executor = cosmic::executor::Default;
    type Flags = Config;
    type Message = Message;
    const APP_ID: &'static str = "com.example.CosmicWidgetAgenda";

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
            size: Some((Some(config.width), Some(120))),
            exclusive_zone: 0,
            namespace: "cosmic-widget-agenda".into(),
            ..Default::default()
        });

        let widget = Widget {
            core,
            surface_id,
            config,
            events: None,
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
            Message::Fetched(Ok(events)) => {
                self.events = Some(events);
                self.last_error = None;
                self.last_update = Some(now_hhmm());
                self.resize_task()
            }
            Message::Fetched(Err(e)) => {
                self.last_error = Some(e);
                self.resize_task()
            }
            Message::OpenCalendar => {
                let _ = std::process::Command::new("xdg-open")
                    .arg(&self.config.calendar_url)
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
            vec![text::heading("Harmonogram").width(Length::Fill).into()];
        if let Some(ts) = &self.last_update {
            header_items.push(text::caption(ts.clone()).class(dim_text()).into());
        }
        rows.push(
            cosmic::widget::row::with_children(header_items)
                .spacing(8)
                .align_y(cosmic::iced::Alignment::Center)
                .into(),
        );

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
                    let el: Element<'_, Message> = if let Some(loc) = &ev.location {
                        tooltip::tooltip(row, text::body(loc.clone()), tooltip::Position::Top).into()
                    } else {
                        row.into()
                    };
                    rows.push(el);
                }
            }
        }

        let content = cosmic::widget::column::with_children(rows).spacing(6);

        let card = container(content)
            .padding([12, 14])
            .class(cosmic::theme::Container::custom(glass_style))
            .width(Length::Fixed(width));

        mouse_area(card).on_press(Message::OpenCalendar).into()
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
