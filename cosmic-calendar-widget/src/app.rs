//! Desktop calendar widget — layer-shell surface anchored to a screen
//! corner (same mechanism as `cosmic-widget-panel`), positioned below the
//! services-status widget by default. Behaves like Obsidian's own
//! calendar: clicking a day opens (creating it from the daily-note
//! template if needed) that day's note — in Obsidian or directly as a
//! `.md` file, depending on the persistent "Obsidian" toggle — and each
//! day shows the same word-count dots as Obsidian's Calendar plugin.

use std::collections::HashMap;
use std::time::Duration;

use cosmic::app::{Core, Task};
use cosmic::iced::border::Border;
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{
    IcedMargin, SctkLayerSurfaceSettings,
};
use cosmic::iced::platform_specific::shell::commands::layer_surface::{
    destroy_layer_surface, get_layer_surface, set_margin, Anchor, KeyboardInteractivity, Layer,
};
use cosmic::iced::window;
use cosmic::iced::{Background, Color, Length, Subscription};
use cosmic::widget::{button, container, text};
use cosmic::Element;
use cosmic_widget_common::{desktop, drag, glass, Glass, Outcome, WidgetPos};
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
    pos: WidgetPos,
    /// Szkło karty: przezroczystość + region rozmycia tła.
    glass: Glass,
    vault: Vault,
    calendar: CalendarState,
    holidays: HolidayData,
    /// Note-dot counts for the currently visible grid, keyed by date.
    dots: HashMap<Date, u32>,
    /// Czy powierzchnia layer-shell istnieje w tej chwili. `false` = jesteśmy
    /// na innym pulpicie wirtualnym i widget jest schowany (patrz `desktop`).
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
) -> SctkLayerSurfaceSettings {
    SctkLayerSurfaceSettings {
        id,
        // NIE zmieniać na Layer::Background (sprawdzone 2026-08-11): reszta
        // widgetów zniosła to bez szkody, ale ten jeden ląduje wtedy pod
        // tapetą plasmashella i znika z pulpitu na dobre — proces żyje i
        // publikuje geometrię, a na ekranie go nie ma.
        layer: Layer::Bottom,
        keyboard_interactivity: KeyboardInteractivity::None,
        anchor: anchor_from_str(anchor),
        margin: IcedMargin {
            top: margin_y,
            right: margin_x,
            bottom: margin_y,
            left: margin_x,
        },
        size: Some((Some(width), Some(WIDGET_HEIGHT))),
        exclusive_zone: 0,
        namespace: "cosmic-calendar-widget".into(),
        ..Default::default()
    }
}

// =========================================================================
// Application Trait
// =========================================================================

impl cosmic::Application for Widget {
    type Executor = cosmic::executor::Default;
    type Flags = Config;
    type Message = Message;
    const APP_ID: &'static str = "io.github.longer54.CosmicCalendarWidget";

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
        let vault = Vault::from_config(&config);
        let calendar = CalendarState::today();
        // Powierzchnia POWSTAJE PO `WidgetPos` i z jego marginesów, nie
        // z surowego configu — patrz `WidgetPos::new` (config bywa
        // nieświeży, prawdziwą pozycję zna opublikowana geometria).
        let mut pos = WidgetPos::new(
            "calendar",
            &config.anchor,
            config.margin_x,
            config.margin_y,
            config.width,
            Some(config.stack_below.clone()),
            config.stack_gap,
            config.stack_offset_y,
        );
        pos.set_height(WIDGET_HEIGHT);

        let surface_task = get_layer_surface(surface_settings(
            surface_id,
            &config.anchor,
            pos.margin_x(),
            pos.margin_y(),
            config.width,
        ));
        let glass = Glass::from_config(&config.glass);
        let mut widget = Widget {
            core,
            surface_id,
            config,
            pos,
            glass,
            vault,
            calendar,
            holidays: HolidayData::load(),
            dots: HashMap::new(),
            visible: true,
        };
        widget.refresh_dots();

        let blur = widget.blur_task();
        (widget, Task::batch([surface_task, blur]))
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
        let mut subs = vec![
            cosmic::iced::time::every(Duration::from_secs(DOT_REFRESH_SECS)).map(|_| Message::Tick),
        ];
        if self.pos.dragging() {
            subs.push(drag::subscription().map(Message::Drag));
        }
        // Schowany widget nie ma powierzchni, więc nie ma czego przesuwać.
        if self.pos.is_stacked() && self.visible {
            subs.push(
                cosmic::iced::time::every(Duration::from_secs(2)).map(|_| Message::StackTick),
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

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectDate(date) => {
                self.calendar.selected = date;
                if let Err(e) = self.vault.ensure_note(date) {
                    eprintln!("cosmic-calendar-widget: nie mogę utworzyć notatki {date}: {e}");
                }
                self.dots.insert(date, self.vault.dots_for(date));
                let target = if self.config.open_in_obsidian {
                    self.vault.open_uri(date)
                } else {
                    self.vault.note_path(date).to_string_lossy().into_owned()
                };
                if let Err(e) = std::process::Command::new("xdg-open").arg(target).spawn() {
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
            Message::ToggleOpenMode(enabled) => {
                self.config.open_in_obsidian = enabled;
                self.config.save();
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
            // przypiąć layer-shella do pulpitu); powrót = odtwórz ją, z
            // pozycją z `pos`, żeby przeciągnięcia i stack nie przepadły.
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
            background_color: cosmic::iced::Color::TRANSPARENT,
            text_color: Default::default(),
            icon_color: Default::default(),
        })
    }
}

impl Widget {
    /// Region rozmycia tła pod kartą. Kompozytor gubi go razem z
    /// powierzchnią, więc wołamy to po każdym jej utworzeniu. Wysokość jest
    /// tu stała (`WIDGET_HEIGHT`), więc nie ma czego odświeżać przy resize.
    fn blur_task(&self) -> Task<Message> {
        glass::blur_task_single(self.surface_id, self.glass, self.config.width, WIDGET_HEIGHT)
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
            drag::handle(Message::DragStart),
            prev_btn.into(),
            title.into(),
            next_btn.into(),
        ])
        .spacing(4)
        .align_y(cosmic::iced::Alignment::Center);

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
                cells.push(self.build_day_cell(current, state, today));
                current = current.tomorrow().expect("date overflow");
            }
            grid_rows.push(cosmic::widget::row::with_children(cells).spacing(0).into());
        }

        let grid = cosmic::widget::column::with_children(grid_rows).spacing(2);

        // ---- Bottom row: "Dziś" + persistent open-mode toggle ----
        let today_btn = button::standard("Dziś")
            .on_press(Message::GoToToday)
            .width(Length::Fill);
        // Trwały przełącznik: włączony = klik w dzień otwiera Obsidiana,
        // wyłączony = klik otwiera plik .md bezpośrednio.
        let mode_toggle = cosmic::widget::toggler(self.config.open_in_obsidian)
            .label("Obsidian".to_string())
            .spacing(6.0)
            .on_toggle(Message::ToggleOpenMode);
        let bottom_row = cosmic::widget::row::with_children(vec![
            today_btn.into(),
            container(mode_toggle)
                .center_y(Length::Shrink)
                .padding([6, 0, 0, 4])
                .into(),
        ])
        .spacing(6);

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

        content = content.push(container(bottom_row).padding([6, 16]));

        // Tinted glass card background, same visual language as the
        // services-status widget it sits below.
        container(content)
            .padding([12, 10, 10, 10])
            .class(cosmic::theme::Container::custom(self.glass.container()))
            .width(Length::Fixed(self.config.width as f32))
            .into()
    }

    /// Build one day cell: day number + a row of note-dots below it.
    fn build_day_cell(&self, date: Date, state: &CalendarState, today: Date) -> Element<'_, Message> {
        let is_today = date == today;
        let is_selected = date == state.selected;
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

        // tooltip::tooltip renderował się źle w tym layer-shell surface'ie
        // (pusta ciemna belka zamiast dymka nad kursorem) — nazwa święta
        // i tak jest widoczna w liście "najbliższe święta" pod kalendarzem.
        btn.into()
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
