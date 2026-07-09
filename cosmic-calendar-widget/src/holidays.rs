//! Polish holidays — fixed dates + movable (Easter-based) dates.

use jiff::civil::Date;

/// A named holiday on a specific date.
#[derive(Debug, Clone)]
pub struct Holiday {
    pub name: &'static str,
    pub date: Date,
    /// Whether it's a "red-letter" day (ustawowo wolny od pracy).
    pub public: bool,
}

/// Collection of Polish holidays loaded for the current + adjacent years.
pub struct HolidayData {
    /// All holidays, sorted by date for binary search.
    pub(crate) holidays: Vec<Holiday>,
}

impl HolidayData {
    /// Load holidays for the years around today (±2 years).
    pub fn load() -> Self {
        let today = jiff::Zoned::now().date();
        let start_year = today.year() - 2;
        let end_year = today.year() + 2;

        let mut holidays = Vec::new();
        for year in start_year..=end_year {
            holidays.extend(holidays_for_year(year));
        }
        holidays.sort_by_key(|h| h.date);
        HolidayData { holidays }
    }

    /// Find a holiday on the given date, if any.
    pub fn get(&self, date: Date) -> Option<&Holiday> {
        self.holidays
            .binary_search_by_key(&date, |h| h.date)
            .ok()
            .map(|i| &self.holidays[i])
    }

}

/// Generate all Polish holidays for a given year.
fn holidays_for_year(year: i16) -> Vec<Holiday> {
    let easter = easter_sunday(year);

    vec![
        // ---- Fixed-date holidays ----
        h("Nowy Rok", year, 1, 1, true),
        h("Święto Trzech Króli", year, 1, 6, true),
        h("Święto Pracy", year, 5, 1, true),
        h("Święto Konstytucji 3 Maja", year, 5, 3, true),
        h("Wniebowzięcie NMP", year, 8, 15, true),
        h("Wszystkich Świętych", year, 11, 1, true),
        h("Narodowe Święto Niepodległości", year, 11, 11, true),
        h("Boże Narodzenie (1. dzień)", year, 12, 25, true),
        h("Boże Narodzenie (2. dzień)", year, 12, 26, true),

        // ---- Easter-based (movable) holidays ----
        h_rel("Wielkanoc", easter, 0, true),
        h_rel("Poniedziałek Wielkanocny", easter, 1, true),
        h_rel("Zielone Świątki", easter, 49, true),
        h_rel("Boże Ciało", easter, 60, true),
    ]
}

fn h(name: &'static str, year: i16, month: i8, day: i8, public: bool) -> Holiday {
    let date = Date::new(year, month, day).expect("valid date");
    Holiday { name, date, public }
}

fn h_rel(name: &'static str, base: Date, offset_days: i64, public: bool) -> Holiday {
    use jiff::Span;
    let date = base
        .checked_add(Span::new().days(offset_days))
        .expect("valid offset");
    Holiday { name, date, public }
}

/// Computes Easter Sunday date using the Anonymous Gregorian algorithm
/// (Meeus/Jones/Butcher). Valid for all Gregorian years.
fn easter_sunday(year: i16) -> Date {
    let y = year as i32;
    let a = y % 19;
    let b = y / 100;
    let c = y % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = ((h + l - 7 * m + 114) / 31) as i8;
    let day = (((h + l - 7 * m + 114) % 31) + 1) as i8;

    Date::new(year, month, day).expect("Easter calculation produced valid date")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easter_known_dates() {
        // Known Easter dates
        assert_eq!(easter_sunday(2024), Date::new(2024, 3, 31).unwrap());
        assert_eq!(easter_sunday(2025), Date::new(2025, 4, 20).unwrap());
        assert_eq!(easter_sunday(2026), Date::new(2026, 4, 5).unwrap());
        assert_eq!(easter_sunday(2027), Date::new(2027, 3, 28).unwrap());
    }

    #[test]
    fn test_holidays_present() {
        let data = HolidayData::load();
        // Nowy Rok 2026
        assert!(data.get(Date::new(2026, 1, 1).unwrap()).is_some());
        // Wielkanoc 2026
        assert!(data.get(Date::new(2026, 4, 5).unwrap()).is_some());
        // Poniedziałek Wielkanocny 2026
        assert!(data.get(Date::new(2026, 4, 6).unwrap()).is_some());
        // Random non-holiday
        assert!(data.get(Date::new(2026, 3, 15).unwrap()).is_none());
    }
}
