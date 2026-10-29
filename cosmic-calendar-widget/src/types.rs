//! Shared types for the calendar widget.

use jiff::civil::Date;

/// State for the calendar grid navigation and selection.
#[derive(Debug, Clone)]
pub struct CalendarState {
    pub selected: Date,
    pub visible_year: i16,
    pub visible_month: i8,
}

impl CalendarState {
    pub fn today() -> Self {
        let today = jiff::Zoned::now().date();
        Self {
            selected: today,
            visible_year: today.year(),
            visible_month: today.month(),
        }
    }

    pub fn show_prev_month(&mut self) {
        if self.visible_month == 1 {
            self.visible_month = 12;
            self.visible_year -= 1;
        } else {
            self.visible_month -= 1;
        }
    }

    pub fn show_next_month(&mut self) {
        if self.visible_month == 12 {
            self.visible_month = 1;
            self.visible_year += 1;
        } else {
            self.visible_month += 1;
        }
    }

    pub fn go_to_today(&mut self) {
        let today = jiff::Zoned::now().date();
        self.selected = today;
        self.visible_year = today.year();
        self.visible_month = today.month();
    }

    pub fn first_of_month(&self) -> Date {
        Date::new(self.visible_year, self.visible_month, 1).expect("valid date")
    }
}

/// Messages the widget can handle.
#[derive(Debug, Clone)]
pub enum Message {
    /// Day cell clicked: select it, ensure its daily note exists, and open
    /// it — in Obsidian or directly as a `.md` file, depending on the
    /// `open_in_obsidian` toggle.
    SelectDate(Date),
    PrevMonth,
    NextMonth,
    GoToToday,
    /// Periodic refresh of the note-dot cache, to pick up edits made
    /// directly in Obsidian while the widget stays open.
    Tick,
    /// Persistent toggle: `true` = day clicks open Obsidian, `false` = day
    /// clicks open the `.md` file directly. Saved back to config.toml.
    ToggleOpenMode(bool),
    /// Uchwyt ⠿ wciśnięty — start przeciągania widgetu.
    DragStart,
    /// Event myszy podczas przeciągania (cosmic-widget-common).
    Drag(cosmic_widget_common::DragEvent),
    /// Tick stacku — dopasuj pozycję do widgetu-rodzica.
    StackTick,
    /// Zmiana pulpitu wirtualnego w KWin: `true` = jesteśmy na „swoim"
    /// pulpicie (pokaż powierzchnię), `false` = na innym (schowaj).
    DesktopVisible(bool),
    /// Nowy poziom oszklenia — użytkownik ruszył suwakiem przezroczystości doka.
    Glass(cosmic_widget_common::Glass),
}
