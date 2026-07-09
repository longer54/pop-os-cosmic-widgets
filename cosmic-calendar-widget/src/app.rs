//! Desktop calendar widget — layer-shell surface anchored to a screen
//! corner (same mechanism as `cosmic-widget-panel`), positioned below the
//! services-status widget by default. Behaves like Obsidian's own
//! calendar: clicking a day opens (creating it from the daily-note
//! template if needed) that day's note in Obsidian, and each day shows the
//! same word-count dots as Obsidian's Calendar plugin.

use std::collections::HashMap;
use std::time::Duration;

use cosmic::app::{Core, Task};
use cosmic::iced::border::Border;
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{
    IcedMargin, SctkLayerSurfaceSettings,
};
use cosmic::iced::platform_specific::shell::commands::layer_surface::{
    get_layer_surface, Anchor, KeyboardInteractivity, Layer,
};
use cosmic::iced::window;
use cosmic::iced::{Background, Color, Length, Shadow, Subscription, Vector};
use cosmic::widget::{button, container, text, tooltip};
use cosmic::Element;
use jiff::civil::{Date, Weekday};

use crate::config::Config;
use crate::holidays::HolidayData;
use crate::types::{CalendarState, Message};
use crate::vault::Vault;

const WIDGET_HEIGHT: u32 = 430;
const CELL_SIZE: f32 = 44.0;
const DOT_ROW_HEIGHT: f32 = 8.0;
const ROWS: usize = 6;
const COLS: usize = 7;
const DOT_REFRESH_SECS: u64 = 30;

static MONTHS: [&str; 12] = [
    "Styczeń", "Luty", "Marzec", "Kwiecień", "Maj", "Czerwiec",
    "Lipiec", "Sierpień", "Wrzesień", "Październik", "Listopad", "Grudzień",
];

static WEEKDAYS: [&str; 7] = ["Pon", "Wt", "Śr", "Czw", "Pt", "Sob", "Ndz"];

// =========================================================================
// Widget Struct
// =========================================================================

pub struct Widget {
    core: Core,
    surface_id: window::Id,
    config: Config,
    vault: Vault,
    calendar: CalendarState,
    holidays: HolidayData,
    /// Note-dot counts for the currently visible grid, keyed by date.
    dots: HashMap<Date, u32>,
}

fn anchor_from_str(s: &str) -> Anchor {
    match s {
        "top-left" => Anchor::TOP | Anchor::LEFT,
        "bottom-left" => Anchor::BOTTOM | Anchor::LEFT,
        "bottom-right" => Anchor::BOTTOM | Anchor::RIGHT,
        _ => Anchor::TOP | Anchor::RIGHT,
    }
}

// =========================================================================
// Application Trait
// =========================================================================

impl cosmic::Application for Widget {
    type Executor = cosmic::executor::Default;
    type Flags = Config;
    type Message = Message;
    const APP_ID: &'static str = "com.example.CosmicCalendarWidget";

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
            size: Some((Some(config.width), Some(WIDGET_HEIGHT))),
            exclusive_zone: 0,
            namespace: "cosmic-calendar-widget".into(),
            ..Default::default()
        });

        let vault = Vault::from_config(&config);
        let calendar = CalendarState::today();
        let mut widget = Widget {
            core,
            surface_id,
            config,
            vault,
            calendar,
            holidays: HolidayData::load(),
            dots: HashMap::new(),
        };
        widget.refresh_dots();

        (widget, surface_task)
    }

    /// Main window is suppressed — empty view.
    fn view(&self) -> Element<'_, Message> {
        text::body("").into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if id != self.surface_id {
            return text::body("").into();
        }
        self.build_calendar_widget()
    }

    fn subscription(&self) -> Subscription<Message> {
        cosmic::iced::time::every(Duration::from_secs(DOT_REFRESH_SECS)).map(|_| Message::Tick)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectDate(date) => {
                self.calendar.selected = date;
                if let Err(e) = self.vault.ensure_note(date) {
                    eprintln!("cosmic-calendar-widget: nie mogę utworzyć notatki {date}: {e}");
                }
                self.dots.insert(date, self.vault.dots_for(date));
                let uri = self.vault.open_uri(date);
                if let Err(e) = std::process::Command::new("xdg-open").arg(uri).spawn() {
                    eprintln!("cosmic-calendar-widget: nie mogę uruchomić xdg-open: {e}");
                }
                Task::none()
            }
            Message::PrevMonth => {
                self.calendar.show_prev_month();
                self.refresh_dots();
                Task::none()
            }
            Message::NextMonth => {
                self.calendar.show_next_month();
                self.refresh_dots();
                Task::none()
            }
            Message::GoToToday => {
                self.calendar.go_to_today();
                self.refresh_dots();
                Task::none()
            }
            Message::Tick => {
                self.refresh_dots();
                Task::none()
            }
        }
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::iced::theme::Style {
            background_color: cosmic::iced::Color::TRANSPARENT,
            text_color: Default::default(),
            icon_color: Default::default(),
        })
    }
}

impl Widget {
    /// Rescans the currently visible 6x7 day grid and updates the dot
    /// cache. Cheap: each visible day is a small markdown file read at most.
    fn refresh_dots(&mut self) {
        let first_visible =
            get_calendar_first(self.calendar.visible_year, self.calendar.visible_month, Weekday::Monday);
        let mut current = first_visible;
        let mut dots = HashMap::with_capacity(ROWS * COLS);
        for _ in 0..(ROWS * COLS) {
            dots.insert(current, self.vault.dots_for(current));
            current = current.tomorrow().expect("date overflow");
        }
        self.dots = dots;
    }
}

// =========================================================================
// Calendar UI
// =========================================================================

impl Widget {
    fn build_calendar_widget(&self) -> Element<'_, Message> {
        let today = jiff::Zoned::now().date();
        let state = &self.calendar;
        let holidays = &self.holidays;
        let month_name = MONTHS[(state.visible_month - 1) as usize];

        // ---- Header: ← Month Year → ----
        let prev_btn = button::text("←").on_press(Message::PrevMonth);
        let next_btn = button::text("→").on_press(Message::NextMonth);
        let month_year = format!("{} {}", month_name, state.visible_year);
        let title = container(text::heading(month_year))
            .width(Length::Fill)
            .center_x(Length::Fill);

        let header = cosmic::widget::row::with_children(vec![
            prev_btn.into(),
            title.into(),
            next_btn.into(),
        ])
        .spacing(4);

        // ---- Weekday labels ----
        let weekday_row = cosmic::widget::row::with_children(
            WEEKDAYS
                .iter()
                .map(|d| -> Element<'_, Message> {
                    container(text::caption_heading(*d))
                        .width(Length::Fixed(CELL_SIZE))
                        .center_x(Length::Fixed(CELL_SIZE))
                        .into()
                })
                .collect::<Vec<_>>(),
        );

        // ---- Day grid ----
        let first_visible =
            get_calendar_first(state.visible_year, state.visible_month, Weekday::Monday);

        let mut grid_rows: Vec<Element<'_, Message>> = Vec::with_capacity(ROWS);
        let mut current = first_visible;

        for _row in 0..ROWS {
            let mut cells: Vec<Element<'_, Message>> = Vec::with_capacity(COLS);
            for _col in 0..COLS {
                cells.push(self.build_day_cell(current, state, today, holidays));
                current = current.tomorrow().expect("date overflow");
            }
            grid_rows.push(cosmic::widget::row::with_children(cells).spacing(0).into());
        }

        let grid = cosmic::widget::column::with_children(grid_rows).spacing(2);

        // ---- Today button at bottom ----
        let today_btn = button::standard("Dziś")
            .on_press(Message::GoToToday)
            .width(Length::Fill);

        // ---- Upcoming holidays ----
        let upcoming = self.build_upcoming_holidays(state, holidays);

        // ---- Compose ----
        let mut content = cosmic::widget::column::with_children(vec![
            container(header).padding([4, 0, 6, 0]).into(),
            weekday_row.into(),
            grid.into(),
        ]);

        if let Some(upcoming_section) = upcoming {
            content = content.push(container(upcoming_section).padding([4, 0]));
        }

        content = content.push(container(today_btn).padding([6, 16]));

        // Tinted glass card background, same visual language as the
        // services-status widget it sits below.
        container(content)
            .padding([12, 10, 10, 10])
            .class(cosmic::theme::Container::custom(glass_style))
            .width(Length::Fixed(self.config.width as f32))
            .into()
    }

    /// Build one day cell: day number + a row of note-dots below it.
    fn build_day_cell(
        &self,
        date: Date,
        state: &CalendarState,
        today: Date,
        holidays: &HolidayData,
    ) -> Element<'_, Message> {
        let is_today = date == today;
        let is_selected = date == state.selected;
        let holiday = holidays.get(date);
        let dot_count = self.dots.get(&date).copied().unwrap_or(0);

        let day_label = text::body(format!("{}", date.day()));
        let number_row = container(day_label)
            .width(Length::Fixed(CELL_SIZE))
            .center_x(Length::Fixed(CELL_SIZE));

        let dot_row = container(build_dot_row(dot_count))
            .width(Length::Fixed(CELL_SIZE))
            .height(Length::Fixed(DOT_ROW_HEIGHT))
            .center_x(Length::Fixed(CELL_SIZE));

        let cell_content = cosmic::widget::column::with_children(vec![
            number_row.into(),
            dot_row.into(),
        ])
        .align_x(cosmic::iced::Alignment::Center);

        let cell_content = container(cell_content)
            .width(Length::Fixed(CELL_SIZE))
            .height(Length::Fixed(CELL_SIZE))
            .center_y(Length::Fixed(CELL_SIZE));

        // Every visible day is clickable — including leading/trailing days
        // from adjacent months — same as Obsidian's own calendar.
        let btn = button::custom(cell_content)
            .width(Length::Fixed(CELL_SIZE))
            .height(Length::Fixed(CELL_SIZE))
            .class(if is_selected {
                cosmic::theme::Button::Suggested
            } else if is_today {
                cosmic::theme::Button::Standard
            } else {
                cosmic::theme::Button::Text
            })
            .on_press(Message::SelectDate(date));

        if let Some(h) = holiday {
            tooltip::tooltip(btn, text::body(h.name), tooltip::Position::Top).into()
        } else {
            btn.into()
        }
    }

    /// Build upcoming holidays list for ~2 months ahead.
    fn build_upcoming_holidays(
        &self,
        state: &CalendarState,
        holidays: &HolidayData,
    ) -> Option<Element<'_, Message>> {
        let today = jiff::Zoned::now().date();
        let start = state.first_of_month();
        let end = start
            .checked_add(jiff::Span::new().months(2))
            .expect("valid date");

        let upcoming: Vec<&crate::holidays::Holiday> = holidays
            .holidays
            .iter()
            .filter(|h| h.date >= today && h.date < end)
            .take(3)
            .collect();

        if upcoming.is_empty() {
            return None;
        }

        let items: Vec<Element<'_, Message>> = upcoming
            .iter()
            .map(|h| {
                let date_str = format!("{:02}.{:02}", h.date.day(), h.date.month());
                cosmic::widget::row::with_children(vec![
                    text::body(date_str).width(Length::Fixed(36.0)).into(),
                    text::body(h.name).into(),
                ])
                .spacing(8)
                .into()
            })
            .collect();

        Some(
            cosmic::widget::column::with_children(items)
                .spacing(2)
                .into(),
        )
    }
}

/// Row of small filled dots — like Obsidian's Calendar plugin, one dot per
/// `words_per_dot` words in the day's note (clamped 1..=max_dots), nothing
/// when there's no note.
fn build_dot_row<'a>(count: u32) -> Element<'a, Message> {
    if count == 0 {
        return container(text::body("")).into();
    }
    let dots: Vec<Element<'a, Message>> = (0..count).map(|_| note_dot()).collect();
    cosmic::widget::row::with_children(dots)
        .spacing(3)
        .into()
}

fn note_dot<'a>() -> Element<'a, Message> {
    container(text::body(""))
        .width(Length::Fixed(4.0))
        .height(Length::Fixed(4.0))
        .class(cosmic::theme::Container::custom(|_| {
            cosmic::iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.65))),
                border: Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }))
        .into()
}

// =========================================================================
// Style
// =========================================================================

/// Tinted-glass card look, matching `cosmic-widget-panel`.
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

// =========================================================================
// Helpers
// =========================================================================

fn get_calendar_first(year: i16, month: i8, from_weekday: Weekday) -> Date {
    let first = Date::new(year, month, 1).expect("valid date");
    let wd = first.weekday();

    let offset: i64 = match (wd, from_weekday) {
        (Weekday::Monday, Weekday::Monday) => 0,
        (Weekday::Tuesday, Weekday::Monday) => -1,
        (Weekday::Wednesday, Weekday::Monday) => -2,
        (Weekday::Thursday, Weekday::Monday) => -3,
        (Weekday::Friday, Weekday::Monday) => -4,
        (Weekday::Saturday, Weekday::Monday) => -5,
        (Weekday::Sunday, Weekday::Monday) => -6,
        _ => 0,
    };

    if offset == 0 {
        first
    } else {
        first
            .checked_add(jiff::Span::new().days(offset))
            .expect("valid offset")
    }
}
