//! Widget layer-shell z listą zaległych/dzisiejszych zadań ze skanu vaulta
//! Obsidian (składnia wtyczki Tasks). Ten sam szkielet co pozostałe widgety
//! (Layer::Bottom, jawny rozmiar + set_size po każdym odświeżeniu).

use std::time::Duration as StdDuration;

use chrono::Local;
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
use crate::tasks::{scan_tasks, TaskItem};

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Scanned(Result<Vec<TaskItem>, String>),
    OpenTask(usize),
}

pub struct Widget {
    core: Core,
    surface_id: window::Id,
    config: Config,
    tasks: Option<Vec<TaskItem>>,
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

/// Minimalny percent-encoding — jak w cosmic-calendar-widget/src/vault.rs.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn due_label(t: &TaskItem) -> String {
    if t.overdue_days == 0 {
        "dziś".into()
    } else if t.overdue_days == 1 {
        "wczoraj".into()
    } else {
        format!("{} dni temu", t.overdue_days)
    }
}

impl Widget {
    fn fetch_task(&self) -> Task<Message> {
        let vault_path = self.config.vault_path.clone();
        let max_tasks = self.config.max_tasks;
        cosmic::task::future(async move { Message::Scanned(scan_tasks(&vault_path, max_tasks)) })
    }

    fn estimated_height(&self) -> u32 {
        let mut h: u32 = 24 + 34;
        if self.last_error.is_some() {
            h += 30;
        }
        match &self.tasks {
            None => h += 32,
            Some(tasks) if tasks.is_empty() => h += 32,
            // Każdy wiersz jest owinięty w tooltip (dodatkowy padding) — stąd
            // 36px/wiersz zamiast "gołych" ~26px tekstu, plus bufor 16px na
            // koniec. Lepiej trochę pustego miejsca u dołu karty niż
            // przycięte wiersze poza realnym rozmiarem powierzchni
            // layer-shell (przycinanie było realnym bugiem: 13 zadań w
            // danych, tylko 10 widocznych na ekranie przy starym 26px).
            Some(tasks) => h += 36 * tasks.len() as u32 + 16,
        }
        h.clamp(70, 1600)
    }

    fn resize_task(&self) -> Task<Message> {
        set_size(self.surface_id, Some(self.config.width), Some(self.estimated_height()))
    }
}

impl cosmic::Application for Widget {
    type Executor = cosmic::executor::Default;
    type Flags = Config;
    type Message = Message;
    const APP_ID: &'static str = "com.example.CosmicWidgetTasks";

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
            namespace: "cosmic-widget-tasks".into(),
            ..Default::default()
        });

        let widget = Widget {
            core,
            surface_id,
            config,
            tasks: None,
            last_error: None,
            last_update: None,
        };

        let fetch = widget.fetch_task();
        (widget, Task::batch([surface_task, fetch]))
    }

    fn subscription(&self) -> Subscription<Message> {
        let secs = self.config.refresh_secs.max(30);
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
            Message::Scanned(Ok(tasks)) => {
                self.tasks = Some(tasks);
                self.last_error = None;
                self.last_update = Some(now_hhmm());
                self.resize_task()
            }
            Message::Scanned(Err(e)) => {
                self.last_error = Some(e);
                self.resize_task()
            }
            Message::OpenTask(idx) => {
                if let Some(tasks) = &self.tasks {
                    if let Some(t) = tasks.get(idx) {
                        let uri = format!(
                            "obsidian://open?vault={}&file={}",
                            percent_encode(&self.config.vault_name),
                            percent_encode(&t.file_rel)
                        );
                        let _ = std::process::Command::new("xdg-open").arg(uri).spawn();
                    }
                }
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
const RED: Color = Color::from_rgb(0.95, 0.30, 0.30);
const ORANGE: Color = Color::from_rgb(0.95, 0.65, 0.20);

fn dim_text() -> cosmic::theme::Text {
    cosmic::theme::Text::Color(DIM)
}

fn overdue_color(t: &TaskItem) -> Color {
    if t.overdue_days > 0 {
        RED
    } else {
        ORANGE
    }
}

fn dot<'a>(color: Color) -> Element<'a, Message> {
    container(text::body(""))
        .width(Length::Fixed(10.0))
        .height(Length::Fixed(10.0))
        .class(cosmic::theme::Container::custom(move |_| {
            cosmic::iced::widget::container::Style {
                background: Some(Background::Color(color)),
                border: Border {
                    radius: 5.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }))
        .into()
}

impl Widget {
    fn build_widget(&self) -> Element<'_, Message> {
        let width = self.config.width as f32;
        let mut rows: Vec<Element<'_, Message>> = Vec::new();

        // ---- Nagłówek ----
        let mut header_items: Vec<Element<'_, Message>> =
            vec![text::heading("Zaległe zadania").width(Length::Fill).into()];
        if let Some(ts) = &self.last_update {
            header_items.push(text::caption(ts.clone()).class(dim_text()).into());
        }
        rows.push(
            cosmic::widget::row::with_children(header_items)
                .spacing(8)
                .align_y(cosmic::iced::Alignment::Center)
                .into(),
        );

        // ---- Błąd ----
        if let Some(err) = &self.last_error {
            rows.push(text::caption(format!("błąd: {err}")).class(dim_text()).into());
        }

        // ---- Treść ----
        match &self.tasks {
            None => {
                if self.last_error.is_none() {
                    rows.push(text::caption("Ładowanie…").class(dim_text()).into());
                }
            }
            Some(tasks) if tasks.is_empty() => {
                rows.push(text::caption("Brak zaległych zadań 🎉").class(dim_text()).into());
            }
            Some(tasks) => {
                for (idx, t) in tasks.iter().enumerate() {
                    let row = cosmic::widget::row::with_children(vec![
                        dot(overdue_color(t)),
                        text::caption(due_label(t)).width(Length::Fixed(80.0)).class(dim_text()).into(),
                        text::body(t.text.clone()).width(Length::Fill).into(),
                    ])
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center);

                    let clickable = mouse_area(row).on_press(Message::OpenTask(idx));
                    let el: Element<'_, Message> =
                        tooltip::tooltip(clickable, text::body(t.file_rel.clone()), tooltip::Position::Top).into();
                    rows.push(el);
                }
            }
        }

        let content = cosmic::widget::column::with_children(rows).spacing(6);

        let card = container(content)
            .padding([12, 14])
            .class(cosmic::theme::Container::custom(glass_style))
            .width(Length::Fixed(width));

        card.into()
    }
}

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
