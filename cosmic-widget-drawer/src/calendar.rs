//! Kalendarzyk pod drzewem vaulta — klik w dzień otwiera notatkę dnia.
//!
//! Układ notatek jest ten sam, co w widgecie kalendarza
//! (`cosmic-applet-calendar`): `<vault>/<folder dzienny>/RRRR-MM-DD.md`,
//! a brakująca notatka powstaje z szablonu z podmianą `{{date}}`/`{{title}}`.
//! Tamtejszego modułu `vault.rs` nie da się tu wciągnąć przez `#[path]` tak
//! jak reszty źródeł — sięga po `crate::config::Config`, czyli w tym crate
//! trafiłby na konfigurację szuflady. Stąd te kilkadziesiąt linii osobno,
//! za to bez zależności na `jiff` (kalendarz liczymy `chrono`, które już
//! mamy).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Days, Months, NaiveDate};

pub static WEEKDAY_HEADS: [&str; 7] = ["Pn", "Wt", "Śr", "Cz", "Pt", "So", "Nd"];

static MONTHS: [&str; 12] = [
    "styczeń",
    "luty",
    "marzec",
    "kwiecień",
    "maj",
    "czerwiec",
    "lipiec",
    "sierpień",
    "wrzesień",
    "październik",
    "listopad",
    "grudzień",
];

pub struct Calendar {
    vault: PathBuf,
    daily_folder: String,
    template: String,
    /// Pierwszy dzień pokazywanego miesiąca.
    month: NaiveDate,
    /// Dni tego miesiąca, dla których notatka już istnieje. Liczone raz —
    /// przy otwarciu szuflady i przy zmianie miesiąca — żeby nie robić
    /// `stat` na trzydziestu plikach w każdej klatce animacji.
    with_note: HashSet<NaiveDate>,
}

impl Calendar {
    pub fn new(vault: PathBuf, daily_folder: String, template: String, today: NaiveDate) -> Self {
        let mut cal = Self {
            vault,
            daily_folder,
            template,
            month: first_of_month(today),
            with_note: HashSet::new(),
        };
        cal.rescan();
        cal
    }

    pub fn month(&self) -> NaiveDate {
        self.month
    }

    pub fn month_label(&self) -> String {
        format!(
            "{} {}",
            MONTHS[(self.month.month() - 1) as usize],
            self.month.year()
        )
    }

    pub fn shift(&mut self, months_forward: i32) {
        self.month = if months_forward >= 0 {
            self.month
                .checked_add_months(Months::new(months_forward as u32))
                .unwrap_or(self.month)
        } else {
            self.month
                .checked_sub_months(Months::new((-months_forward) as u32))
                .unwrap_or(self.month)
        };
        self.rescan();
    }

    pub fn go_to(&mut self, day: NaiveDate) {
        self.month = first_of_month(day);
        self.rescan();
    }

    pub fn has_note(&self, day: NaiveDate) -> bool {
        self.with_note.contains(&day)
    }

    pub fn note_path(&self, day: NaiveDate) -> PathBuf {
        self.vault
            .join(&self.daily_folder)
            .join(format!("{day}.md"))
    }

    /// Siatka tygodni od poniedziałku. `None` = pole puste (dni sąsiednich
    /// miesięcy nie pokazujemy, tak jak w widgecie kalendarza).
    pub fn weeks(&self) -> Vec<[Option<NaiveDate>; 7]> {
        let days_in_month = days_in_month(self.month);
        let first_col = self.month.weekday().num_days_from_monday() as usize;

        let mut weeks = Vec::new();
        let mut week: [Option<NaiveDate>; 7] = [None; 7];
        let mut col = first_col;
        for day_num in 1..=days_in_month {
            let day = self.month.with_day(day_num).expect("dzień w tym miesiącu");
            week[col] = Some(day);
            col += 1;
            if col == 7 {
                weeks.push(week);
                week = [None; 7];
                col = 0;
            }
        }
        if col > 0 {
            weeks.push(week);
        }
        weeks
    }

    pub fn rescan(&mut self) {
        self.with_note.clear();
        for day_num in 1..=days_in_month(self.month) {
            if let Some(day) = self.month.with_day(day_num) {
                if self.note_path(day).is_file() {
                    self.with_note.insert(day);
                }
            }
        }
    }

    /// Utwórz notatkę dnia, jeśli jej nie ma, i zwróć ścieżkę do otwarcia.
    ///
    /// Zapis idzie lokalnie — tak samo, jak robi to widget kalendarza.
    /// Notatka dnia jest NOWYM plikiem, więc nie grozi tu konflikt dwóch
    /// wersji tej samej treści, przed którym broni `vault_io` w widgecie
    /// zadań (tam chodzi o edycję istniejących linii).
    pub fn ensure_note(&mut self, day: NaiveDate) -> std::io::Result<PathBuf> {
        let path = self.note_path(day);
        if path.is_file() {
            return Ok(path);
        }
        let date_str = day.to_string();
        let content = match self.template_path() {
            Some(tp) => std::fs::read_to_string(tp)
                .unwrap_or_default()
                .replace("{{date}}", &date_str)
                .replace("{{title}}", &date_str),
            None => String::new(),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        self.with_note.insert(day);
        Ok(path)
    }

    fn template_path(&self) -> Option<PathBuf> {
        let t = self.template.trim();
        if t.is_empty() {
            return None;
        }
        let path = self.vault.join(format!("{t}.md"));
        path.is_file().then_some(path)
    }
}

fn first_of_month(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap_or(d)
}

fn days_in_month(first: NaiveDate) -> u32 {
    let next = first
        .checked_add_months(Months::new(1))
        .unwrap_or(first)
        .with_day(1)
        .unwrap_or(first);
    next.checked_sub_days(Days::new(1))
        .map(|d| d.day())
        .unwrap_or(28)
}

/// Krótka etykieta pliku dziennego — do podpowiedzi „co się otworzy”.
pub fn note_label(day: NaiveDate, folder: &Path) -> String {
    format!("{}/{day}.md", folder.display())
}

#[cfg(test)]
mod tests {
    use super::{Calendar, days_in_month};
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn cal(today: NaiveDate) -> Calendar {
        let dir = std::env::temp_dir().join(format!("drawer-cal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Kalendarz")).unwrap();
        std::fs::write(dir.join("Kalendarz/2026-09-18.md"), "cokolwiek").unwrap();
        Calendar::new(dir, "Kalendarz".into(), String::new(), today)
    }

    #[test]
    fn wrzesien_2026_zaczyna_sie_we_wtorek_i_ma_30_dni() {
        let c = cal(d(2026, 9, 18));
        let weeks = c.weeks();
        // 1 września 2026 to wtorek → poniedziałek pierwszego tygodnia pusty.
        assert!(weeks[0][0].is_none());
        assert_eq!(weeks[0][1], Some(d(2026, 9, 1)));
        assert_eq!(days_in_month(d(2026, 9, 1)), 30);
        let ostatni = weeks.last().unwrap().iter().flatten().max().copied();
        assert_eq!(ostatni, Some(d(2026, 9, 30)));
    }

    #[test]
    fn luty_roku_przestepnego_ma_29_dni() {
        assert_eq!(days_in_month(d(2028, 2, 1)), 29);
        assert_eq!(days_in_month(d(2026, 2, 1)), 28);
    }

    #[test]
    fn widzi_istniejaca_notatke_dnia() {
        let c = cal(d(2026, 9, 18));
        assert!(c.has_note(d(2026, 9, 18)));
        assert!(!c.has_note(d(2026, 9, 17)));
    }

    #[test]
    fn brakujaca_notatka_powstaje_przy_kliknieciu() {
        let mut c = cal(d(2026, 9, 18));
        let path = c.ensure_note(d(2026, 9, 17)).unwrap();
        assert!(path.is_file());
        assert!(c.has_note(d(2026, 9, 17)));
    }

    #[test]
    fn przewijanie_miesiecy_przechodzi_przez_granice_roku() {
        let mut c = cal(d(2026, 12, 10));
        c.shift(1);
        assert_eq!(c.month(), d(2027, 1, 1));
        c.shift(-2);
        assert_eq!(c.month(), d(2026, 11, 1));
    }
}
