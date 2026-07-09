//! Widget statusu usług — powierzchnia layer-shell przyklejona do
//! pulpitu (warstwa Bottom: nad tapetą, pod oknami), zakotwiczona w rogu
//! ekranu wg configu. Pozycja jest deterministyczna — po każdym starcie
//! widget ląduje w tym samym miejscu.

use std::time::Duration;

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

use crate::api::{fetch_status, StatusResponse};
use crate::config::Config;

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Fetched(Result<StatusResponse, String>),
    OpenPanel,
}

pub struct Widget {
    core: Core,
    surface_id: window::Id,
    config: Config,
    data: Option<StatusResponse>,
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
    // Prosty zegar lokalny bez dodatkowych zależności.
    let out = std::process::Command::new("date").arg("+%H:%M").output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => String::new(),
    }
}

impl Widget {
    fn fetch_task(&self) -> Task<Message> {
        let url = self.config.api_url.clone();
        let key = self.config.api_key.clone();
        cosmic::task::future(async move { Message::Fetched(fetch_status(url, key).await) })
    }

    /// Szacunkowa wysokość zawartości w px logicznych — powierzchnia
    /// layer-shell musi dostać jawny rozmiar.
    fn estimated_height(&self) -> u32 {
        let mut h: u32 = 24 + 30; // padding karty + nagłówek
        if self.last_error.is_some() {
            h += 28;
        }
        match &self.data {
            None => h += 44,
            Some(data) => {
                for hs in data.hosts.values() {
                    h += 28; // linia hosta z metrykami
                    let visible = hs
                        .services
                        .iter()
                        .filter(|s| self.config.show_all || s.status != "up")
                        .count() as u32;
                    h += 30 * visible;
                }
            }
        }
        h.clamp(70, 1000)
    }

    fn resize_task(&self) -> Task<Message> {
        set_size(
            self.surface_id,
            Some(self.config.width),
            Some(self.estimated_height()),
        )
    }
}

impl cosmic::Application for Widget {
    type Executor = cosmic::executor::Default;
    type Flags = Config;
    type Message = Message;
    const APP_ID: &'static str = "com.example.CosmicWidgetPanel";

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
            // Jawny rozmiar — layer-shell nie mierzy zawartości sam
            // (autosize działa tylko w appletach); wysokość aktualizujemy
            // po każdym odświeżeniu danych w update().
            size: Some((Some(config.width), Some(90))),
            exclusive_zone: 0,
            namespace: "cosmic-widget-panel".into(),
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
        let secs = self.config.refresh_secs.max(30);
        cosmic::iced::time::every(Duration::from_secs(secs)).map(|_| Message::Tick)
    }

    /// Główne (stłumione) okno — pusty widok.
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
            Message::OpenPanel => {
                let _ = std::process::Command::new("xdg-open")
                    .arg(&self.config.panel_url)
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

const GREEN: Color = Color::from_rgb(0.30, 0.85, 0.45);
const RED: Color = Color::from_rgb(0.95, 0.30, 0.30);
const ORANGE: Color = Color::from_rgb(0.95, 0.65, 0.20);
const DIM: Color = Color::from_rgba(0.95, 0.95, 0.97, 0.55);

fn status_color(status: &str) -> Color {
    match status {
        "up" => GREEN,
        "stale" => ORANGE,
        _ => RED,
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
        let mut header_items: Vec<Element<'_, Message>> = vec![
            text::heading("Status usług").width(Length::Fill).into(),
        ];
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
            rows.push(
                cosmic::widget::row::with_children(vec![
                    dot(ORANGE),
                    text::caption(format!("brak połączenia: {err}")).into(),
                ])
                .spacing(8)
                .align_y(cosmic::iced::Alignment::Center)
                .into(),
            );
        }

        // ---- Treść ----
        match &self.data {
            None if self.config.api_key.is_empty() => {
                rows.push(
                    text::caption(format!(
                        "Uzupełnij api_key w:\n{}",
                        Config::path().display()
                    ))
                    .into(),
                );
            }
            None => {
                if self.last_error.is_none() {
                    rows.push(text::caption("Ładowanie…").class(dim_text()).into());
                }
            }
            Some(data) => {
                for (host, hs) in &data.hosts {
                    let total = hs.services.len();
                    let up = hs.services.iter().filter(|s| s.status == "up").count();

                    // Nagłówek hosta + metryki
                    let mut host_line = format!("{host}  ·  {up}/{total} OK");
                    if let Some(m) = &hs.metrics {
                        let mut parts: Vec<String> = Vec::new();
                        if let Some(r) = m.ram_pct {
                            parts.push(format!("RAM {r:.0}%"));
                        }
                        if let Some(h) = m.hdd_pct {
                            parts.push(format!("HDD {h:.0}%"));
                        }
                        if let Some(t) = m.temp_c {
                            parts.push(format!("{t:.0}°C"));
                        }
                        if !parts.is_empty() {
                            host_line = format!("{host_line}  ·  {}", parts.join(" "));
                        }
                    }
                    let all_ok = up == total;
                    rows.push(
                        cosmic::widget::row::with_children(vec![
                            dot(if all_ok { GREEN } else { RED }),
                            text::caption_heading(host_line).into(),
                        ])
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center)
                        .into(),
                    );

                    // Lista usług: wszystkie albo tylko problematyczne
                    for svc in &hs.services {
                        if !self.config.show_all && svc.status == "up" {
                            continue;
                        }
                        let label = text::body(svc.service.clone());
                        let row = cosmic::widget::row::with_children(vec![
                            container(dot(status_color(&svc.status)))
                                .padding([0, 0, 0, 14])
                                .into(),
                            label.into(),
                            text::caption(svc.status.clone()).class(dim_text()).into(),
                        ])
                        .spacing(8)
                        .align_y(cosmic::iced::Alignment::Center);

                        let el: Element<'_, Message> = if let Some(d) = &svc.detail {
                            tooltip::tooltip(row, text::body(d.clone()), tooltip::Position::Top)
                                .into()
                        } else {
                            row.into()
                        };
                        rows.push(el);
                    }
                }
            }
        }

        let content = cosmic::widget::column::with_children(rows).spacing(6);

        let card = container(content)
            .padding([12, 14])
            .class(cosmic::theme::Container::custom(glass_style))
            .width(Length::Fixed(width));

        mouse_area(card).on_press(Message::OpenPanel).into()
    }
}

fn dim_text() -> cosmic::theme::Text {
    cosmic::theme::Text::Color(DIM)
}

/// Ciemna półprzezroczysta karta w stylu szkła — jak w widgetcie kalendarza.
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
