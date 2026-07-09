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
    /// Day cell clicked: select it, ensure its Obsidian daily note exists,
    /// and open it in Obsidian.
    SelectDate(Date),
    PrevMonth,
    NextMonth,
    GoToToday,
    /// Periodic refresh of the note-dot cache, to pick up edits made
    /// directly in Obsidian while the widget stays open.
    Tick,
}
