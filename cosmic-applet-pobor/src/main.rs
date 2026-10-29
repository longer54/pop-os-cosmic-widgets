//! Aplet panelu COSMIC: ile pradu bierze CALY laptop.
//!
//! W panelu: pobor teraz + srednia z ostatniej godziny. Klik otwiera popup ze
//! srednimi z 15 min / 1 / 3 / 6 / 12 h, wykresem i czasem pracy na baterii.
//!
//! Pomiar = prad x napiecie baterii (BAT0 nie ma power_now). Dziala tylko na
//! baterii — na zasilaczu bateria podaje prad ladowania, nie pobor laptopa,
//! wiec takie chwile nie wchodza do srednich.
//!
//! Probka co 30 s, co minute srednia minutowa idzie do
//! ~/.local/state/pobor-pradu/historia.log ("epoch mW"), wiec srednie
//! przezywaja restart panelu. Do historii dolicza sie tez log trybu zombi
//! (~/.cache/zombi/pomiar.log), bo wtedy panel nie chodzi.

use cosmic::app::{Core, Task};
use cosmic::iced::core::window;
use cosmic::iced::window::Id;
use cosmic::iced::{Length, Rectangle, Subscription};
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{list_column, settings, text};
use cosmic::{Application, Element};
use std::collections::VecDeque;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ID: &str = "io.github.longer54.CosmicAppletPobor";
const BAT: &str = "/sys/class/power_supply/BAT0";
/// Czesciej nie ma sensu: firmware laptopa odswieza prad baterii co kilka minut,
/// a kazde odswiezenie panelu budzi kompozytor.
const CO_ILE_S: u64 = 30;
const HISTORIA_H: u64 = 12;
/// "Teraz" = srednia z ostatniej minuty.
const TERAZ_PROBEK: usize = 2;

/// Srednia minutowa poboru (tylko minuty na baterii).
#[derive(Clone, Copy)]
struct Minuta {
    t: u64,
    mw: u32,
}

#[derive(Default)]
struct Bateria {
    na_baterii: bool,
    status: String,
    procent: u32,
    mw: Option<u32>,
    wh_zostalo: f64,
}

pub struct Aplet {
    core: Core,
    popup: Option<Id>,
    bateria: Bateria,
    historia: VecDeque<Minuta>,
    biezaca_minuta: u64,
    probki_minuty: Vec<u32>,
    ostatnie: VecDeque<u32>,
}

#[derive(Clone, Debug)]
pub enum Message {
    PopupClosed(Id),
    Surface(cosmic::surface::Action),
    Tick,
}

fn teraz_s() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn czytaj(plik: &str) -> Option<String> {
    fs::read_to_string(format!("{BAT}/{plik}")).ok().map(|s| s.trim().to_string())
}

fn czytaj_liczbe(plik: &str) -> Option<u64> {
    czytaj(plik)?.parse().ok()
}

fn odczyt_baterii() -> Bateria {
    let status = czytaj("status").unwrap_or_default();
    let na_baterii = status == "Discharging";
    let prad = czytaj_liczbe("current_now"); // uA
    let napiecie = czytaj_liczbe("voltage_now"); // uV
    let mw = match (prad, napiecie) {
        (Some(i), Some(u)) if i > 0 => Some((i as f64 * u as f64 / 1e9) as u32),
        _ => None,
    };
    let wh_zostalo = match (czytaj_liczbe("charge_now"), napiecie) {
        (Some(q), Some(u)) => q as f64 * u as f64 / 1e12,
        _ => 0.0,
    };
    Bateria {
        na_baterii,
        status,
        procent: czytaj_liczbe("capacity").unwrap_or(0) as u32,
        mw,
        wh_zostalo,
    }
}

fn plik_historii() -> PathBuf {
    let baza = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/state"));
    baza.join("pobor-pradu/historia.log")
}

fn wczytaj_historie() -> VecDeque<Minuta> {
    let od = teraz_s().saturating_sub(HISTORIA_H * 3600);
    let mut minuty: Vec<Minuta> = Vec::new();

    if let Ok(tekst) = fs::read_to_string(plik_historii()) {
        for linia in tekst.lines() {
            let mut pola = linia.split_whitespace();
            if let (Some(Ok(t)), Some(Ok(mw))) =
                (pola.next().map(str::parse::<u64>), pola.next().map(str::parse::<u32>))
            {
                if t >= od {
                    minuty.push(Minuta { t, mw });
                }
            }
        }
    }

    // Log trybu zombi: "2026-10-11_18:19:29 6292 Discharging 73"
    let zombi = PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache/zombi/pomiar.log");
    if let Ok(tekst) = fs::read_to_string(zombi) {
        for linia in tekst.lines() {
            let pola: Vec<&str> = linia.split_whitespace().collect();
            if pola.len() < 3 || pola[2] != "Discharging" {
                continue;
            }
            let Ok(data) = chrono::NaiveDateTime::parse_from_str(pola[0], "%Y-%m-%d_%H:%M:%S") else {
                continue;
            };
            let Some(t) = data.and_local_timezone(chrono::Local).earliest().map(|d| d.timestamp() as u64) else {
                continue;
            };
            if let Ok(mw) = pola[1].parse() {
                if t >= od {
                    minuty.push(Minuta { t, mw });
                }
            }
        }
    }

    minuty.sort_by_key(|m| m.t);
    minuty.into()
}

fn dopisz_do_historii(m: Minuta) {
    let plik = plik_historii();
    if let Some(kat) = plik.parent() {
        let _ = fs::create_dir_all(kat);
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&plik) {
        let _ = writeln!(f, "{} {}", m.t, m.mw);
    }
    // Raz na dobe (o polnocy UTC) przycinamy plik do ostatnich 12 h.
    if m.t % 86400 < 60 {
        let od = m.t.saturating_sub(HISTORIA_H * 3600);
        if let Ok(tekst) = fs::read_to_string(&plik) {
            let zostaw: String = tekst
                .lines()
                .filter(|l| l.split_whitespace().next().and_then(|t| t.parse::<u64>().ok()).is_some_and(|t| t >= od))
                .map(|l| format!("{l}\n"))
                .collect();
            let _ = fs::write(&plik, zostaw);
        }
    }
}

fn waty(mw: f64) -> String {
    format!("{:.1} W", mw / 1000.0).replace('.', ",")
}

fn godziny(h: f64) -> String {
    let min = (h * 60.0).round() as u64;
    format!("{} h {:02} min", min / 60, min % 60)
}

impl Aplet {
    fn teraz_mw(&self) -> Option<f64> {
        if !self.bateria.na_baterii || self.ostatnie.is_empty() {
            return None;
        }
        Some(self.ostatnie.iter().map(|&x| x as f64).sum::<f64>() / self.ostatnie.len() as f64)
    }

    /// Srednia z ostatnich `okno_s` sekund + ile minut danych faktycznie bylo.
    fn srednia(&self, okno_s: u64) -> Option<(f64, usize)> {
        let od = teraz_s().saturating_sub(okno_s);
        let mut suma = 0.0;
        let mut n = 0usize;
        for m in self.historia.iter().rev().take_while(|m| m.t >= od) {
            suma += m.mw as f64;
            n += 1;
        }
        // Biezaca, niepelna minuta tez sie liczy.
        if !self.probki_minuty.is_empty() {
            suma += self.probki_minuty.iter().map(|&x| x as f64).sum::<f64>() / self.probki_minuty.len() as f64;
            n += 1;
        }
        (n > 0).then(|| (suma / n as f64, n))
    }

    fn opis_sredniej(&self, okno_s: u64) -> String {
        match self.srednia(okno_s) {
            None => "brak danych".into(),
            Some((mw, n)) if (n as u64) * 60 < okno_s * 9 / 10 => {
                format!("{}  (z {})", waty(mw), godziny(n as f64 / 60.0).replace("0 h ", ""))
            }
            Some((mw, _)) => waty(mw),
        }
    }

    /// Wykres z ostatnich 6 h, slupek = 15 min.
    fn wykres(&self) -> String {
        const SLUPKI: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        let teraz = teraz_s();
        let mut kubelki = [(0.0f64, 0u32); 24];
        for m in &self.historia {
            let wstecz = teraz.saturating_sub(m.t) / 900;
            if wstecz < 24 {
                let k = &mut kubelki[23 - wstecz as usize];
                k.0 += m.mw as f64;
                k.1 += 1;
            }
        }
        let wartosci: Vec<Option<f64>> =
            kubelki.iter().map(|&(s, n)| (n > 0).then(|| s / n as f64)).collect();
        let max = wartosci.iter().flatten().cloned().fold(0.0, f64::max);
        let min = wartosci.iter().flatten().cloned().fold(f64::MAX, f64::min);
        if max <= 0.0 {
            return String::new();
        }
        wartosci
            .iter()
            .map(|w| match w {
                None => ' ',
                Some(v) if max - min < 1.0 => SLUPKI[3],
                Some(v) => SLUPKI[(((v - min) / (max - min)) * 7.0).round() as usize],
            })
            .collect()
    }

    fn etykieta_panelu(&self) -> String {
        let godzina = self.srednia(3600).map(|(mw, _)| waty(mw));
        match (self.teraz_mw(), godzina) {
            (Some(t), Some(g)) => format!("{}  ⌀ {}", waty(t), g),
            (Some(t), None) => waty(t),
            (None, Some(g)) => format!("⚡ {}%  ⌀ {}", self.bateria.procent, g),
            (None, None) => format!("⚡ {}%", self.bateria.procent),
        }
    }

    fn zawartosc_popupu(&self) -> Element<'_, Message> {
        let teraz = match self.teraz_mw() {
            Some(mw) => waty(mw),
            None => format!("na zasilaczu ({})", self.bateria.status.to_lowercase()),
        };

        let mut kolumna = list_column()
            .add(settings::item("Cały laptop teraz", text::body(teraz)))
            .add(settings::item("Średnio 15 min", text::body(self.opis_sredniej(900))))
            .add(settings::item("Średnio 1 h", text::body(self.opis_sredniej(3600))))
            .add(settings::item("Średnio 3 h", text::body(self.opis_sredniej(3 * 3600))))
            .add(settings::item("Średnio 6 h", text::body(self.opis_sredniej(6 * 3600))))
            .add(settings::item("Średnio 12 h", text::body(self.opis_sredniej(12 * 3600))));

        let mut bateria = format!("{}% · {:.1} Wh", self.bateria.procent, self.bateria.wh_zostalo).replace('.', ",");
        if self.bateria.na_baterii {
            if let Some((mw, _)) = self.srednia(3600) {
                if mw > 0.0 {
                    bateria += &format!(" · ~{}", godziny(self.bateria.wh_zostalo / (mw / 1000.0)));
                }
            }
        }
        kolumna = kolumna.add(settings::item("Bateria", text::body(bateria)));

        let wykres = self.wykres();
        if !wykres.is_empty() {
            kolumna = kolumna
                .add(cosmic::applet::padded_control(text::caption("Ostatnie 6 h (słupek = 15 min)")))
                .add(cosmic::applet::padded_control(text::title3(wykres).width(Length::Fill)));
        }

        kolumna = kolumna.add(cosmic::applet::padded_control(text::caption(
            "Bateria laptopa często odświeża odczyt co kilka minut, więc „teraz” bywa nieświeże — miarodajne są średnie. Tylko na baterii (zasilacz nie podaje poboru). Czas pracy wg średniej z 1 h.",
        )));

        self.core.applet.popup_container(kolumna).into()
    }

    fn probkuj(&mut self) {
        self.bateria = odczyt_baterii();
        let t = teraz_s();
        let minuta = t / 60;

        if minuta != self.biezaca_minuta {
            if !self.probki_minuty.is_empty() {
                let mw = self.probki_minuty.iter().map(|&x| x as u64).sum::<u64>() / self.probki_minuty.len() as u64;
                let m = Minuta { t: self.biezaca_minuta * 60 + 59, mw: mw as u32 };
                self.historia.push_back(m);
                dopisz_do_historii(m);
            }
            self.probki_minuty.clear();
            self.biezaca_minuta = minuta;
            let od = t.saturating_sub(HISTORIA_H * 3600);
            while self.historia.front().is_some_and(|m| m.t < od) {
                self.historia.pop_front();
            }
        }

        match (self.bateria.na_baterii, self.bateria.mw) {
            (true, Some(mw)) => {
                self.probki_minuty.push(mw);
                self.ostatnie.push_back(mw);
                if self.ostatnie.len() > TERAZ_PROBEK {
                    self.ostatnie.pop_front();
                }
            }
            _ => self.ostatnie.clear(),
        }
    }
}

impl Application for Aplet {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Message>) {
        let mut aplet = Aplet {
            core,
            popup: None,
            bateria: Bateria::default(),
            historia: wczytaj_historie(),
            biezaca_minuta: teraz_s() / 60,
            probki_minuty: Vec::new(),
            ostatnie: VecDeque::new(),
        };
        aplet.probkuj();
        (aplet, Task::none())
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
            Message::Surface(action) => {
                return cosmic::task::message(cosmic::Action::Cosmic(cosmic::app::Action::Surface(action)));
            }
            Message::Tick => self.probkuj(),
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let otwarty = self.popup;
        let (poziomo, pionowo) = self.core.applet.suggested_padding(true);
        let przycisk = cosmic::widget::button::custom(self.core.applet.text(self.etykieta_panelu()))
            .padding([pionowo, poziomo])
            .class(cosmic::theme::Button::AppletIcon)
            .on_press_with_rectangle(move |offset, bounds| {
                if let Some(id) = otwarty {
                    Message::Surface(destroy_popup(id))
                } else {
                    Message::Surface(app_popup::<Aplet>(
                        |_| Default::default(),
                        move |state: &mut Aplet| {
                            let nowe_id = Id::unique();
                            state.popup = Some(nowe_id);
                            let mut ustawienia = state.core.applet.get_popup_settings(
                                state.core.main_window_id().unwrap(),
                                nowe_id,
                                None,
                                None,
                                None,
                            );
                            ustawienia.positioner.anchor_rect = Rectangle {
                                x: (bounds.x - offset.x) as i32,
                                y: (bounds.y - offset.y) as i32,
                                width: bounds.width as i32,
                                height: bounds.height as i32,
                            };
                            ustawienia
                        },
                        Some(Box::new(move |state: &Aplet| state.zawartosc_popupu().map(cosmic::Action::App))),
                    ))
                }
            });

        self.core
            .applet
            .autosize_window(self.core.applet.applet_tooltip::<Message>(
                przycisk,
                "Pobór prądu całego laptopa · teraz i średnia z 1 h",
                self.popup.is_some(),
                Message::Surface,
                None,
            ))
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        text::body("").into()
    }

    fn subscription(&self) -> Subscription<Message> {
        cosmic::iced::time::every(Duration::from_secs(CO_ILE_S)).map(|_| Message::Tick)
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<Aplet>(())
}
