//! Okno podglądu — powierzchnia layer-shell na całym obszarze roboczym,
//! karta na środku, reszta przyciemniona.
//!
//! **Dlaczego layer-shell, a nie zwykłe okno.** Podgląd ma wyskoczyć NAD
//! menedżerem plików, bez ikonki w doku, bez paska tytułu, z klawiaturą na
//! wyłączność (spacja zamyka) i zniknąć bez śladu. Zwykłe okno xdg-shell
//! dostałoby dekoracje, wpis w przełączniku Alt+Tab i własne miejsce w
//! stosie okien. Ten sam wybór, co w `cosmic-widget-drawer`.
//!
//! Proces chodzi w tle (autostart) i przy każdym wywołaniu robi trzy rzeczy:
//! pyta kompozytor o zaznaczony plik ([`crate::selection`]), renderuje
//! treść ([`crate::preview`]) i tworzy powierzchnię. Powierzchnia ginie
//! przy zamknięciu — zamknięty podgląd nie zajmuje ani piksela.

use std::path::PathBuf;

use cosmic::app::{Core, Task};
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{
    IcedMargin, SctkLayerSurfaceSettings,
};
use cosmic::iced::platform_specific::shell::commands::layer_surface::{
    Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface,
};
use cosmic::iced::{Alignment, Background, Color, Length, Subscription, window};
use cosmic::widget::{column, container, divider, image, mouse_area, scrollable, svg, text};
use cosmic::{Element, Theme};
use cosmic_widget_common::{Glass, glass};

use crate::config::Config;
use crate::ipc;
use crate::preview::{self, Body, Preview};
use crate::selection;

/// Zapas wysokości karty na nagłówek, linię i stopkę z podpowiedziami.
const CHROME_H: f32 = 104.0;
/// Wewnętrzny margines karty.
const PAD: f32 = 14.0;
const MIN_W: f32 = 380.0;
/// Jak długo wisi komunikat „nie ma czego pokazać”, zanim sam zniknie.
const HINT_MS: u64 = 2600;

const DIM: Color = Color::from_rgba(0.95, 0.95, 0.97, 0.55);

fn dim() -> cosmic::theme::Text {
    cosmic::theme::Text::Color(DIM)
}

#[derive(Debug, Clone)]
pub enum Message {
    Ipc(ipc::Command),
    /// Wynik pytania o zaznaczenie (pusta lista = powód w `Err`).
    Selected(u64, Result<Vec<PathBuf>, String>),
    /// Gotowy podgląd.
    Loaded(u64, Box<Preview>),
    /// Komunikat sam się chowa — ale tylko jeśli od tego czasu nic się nie
    /// zmieniło (stąd numer pokolenia).
    Expire(u64),
    Close,
    /// Enter — oddaj plik domyślnej aplikacji.
    OpenExternal,
    /// Następny/poprzedni plik z zaznaczenia.
    File(i32),
    /// Następna/poprzednia strona PDF-a.
    Page(i32),
    /// ←/→: strony, gdy podglądamy wielostronicowy PDF; inaczej pliki.
    Arrow(i32),
    Resized(window::Id, f32, f32),
    Glass(Glass),
    /// Klik w kartę — pochłania kliknięcie, żeby nie zamknęło podglądu.
    Noop,
}

/// Co pokazuje karta.
enum Content {
    Loading(PathBuf),
    Ready(Box<Preview>),
    /// Powód, dla którego nie ma podglądu (np. na wierzchu nie ma menedżera
    /// plików). Znika sam po [`HINT_MS`].
    Hint(String),
}

pub struct QuickLook {
    core: Core,
    config: Config,
    glass: Glass,

    id: Option<window::Id>,
    surface_size: Option<(f32, f32)>,

    /// Pliki z zaznaczenia i który z nich oglądamy.
    files: Vec<PathBuf>,
    index: usize,
    page: usize,

    content: Option<Content>,
    /// Numer pokolenia — odpowiedzi ze starych żądań (wolny PDF, zamknięty
    /// w międzyczasie podgląd) mają wylądować w koszu.
    epoch: u64,
}

impl QuickLook {
    fn is_open(&self) -> bool {
        self.id.is_some()
    }

    fn bump(&mut self) -> u64 {
        self.epoch = self.epoch.wrapping_add(1);
        self.epoch
    }

    /// Rozmiar powierzchni: realny (od kompozytora), a zanim przyjdzie —
    /// rozmiar ekranu. Podgląd musi znać go PRZED pierwszym renderowaniem,
    /// bo od niego zależy, na ile pikseli skalować obraz.
    fn surface(&self) -> (f32, f32) {
        self.surface_size.unwrap_or_else(|| {
            let (w, h) = cosmic_widget_common::screen::size().unwrap_or((1920, 1080));
            (w as f32, h as f32)
        })
    }

    /// Największa dozwolona karta.
    fn max_card(&self) -> (f32, f32) {
        let (w, h) = self.surface();
        (
            w * f32::from(self.config.width_pct.clamp(20, 100) as u16) / 100.0,
            h * f32::from(self.config.height_pct.clamp(20, 100) as u16) / 100.0,
        )
    }

    /// Miejsce na samą treść (bez nagłówka i stopki).
    fn content_box(&self) -> (f32, f32) {
        let (w, h) = self.max_card();
        ((w - 2.0 * PAD).max(120.0), (h - CHROME_H).max(120.0))
    }

    /// Realny rozmiar karty — obraz dostaje kartę pod swoje proporcje,
    /// tekst bierze cały dozwolony prostokąt.
    fn card_size(&self) -> (f32, f32) {
        let (max_w, max_h) = self.max_card();
        match self.content.as_ref() {
            Some(Content::Ready(p)) => match p.body.natural_size() {
                Some((w, h)) => ((w + 2.0 * PAD).max(MIN_W).min(max_w), (h + CHROME_H).min(max_h)),
                None => (max_w, max_h),
            },
            Some(Content::Hint(_)) => (MIN_W.min(max_w), 150.0),
            _ => (max_w.min(520.0), 160.0),
        }
    }

    // ---- otwieranie i zamykanie ----

    fn toggle(&mut self) -> Task<Message> {
        if self.is_open() {
            self.close()
        } else {
            self.ask_selection()
        }
    }

    /// Zapytaj kompozytor, co jest zaznaczone w menedżerze plików na
    /// wierzchu. Blokujące (Ctrl+C wirtualną klawiaturą + czekanie na
    /// schowek), więc leci do `spawn_blocking`.
    fn ask_selection(&mut self) -> Task<Message> {
        let epoch = self.bump();
        let managers = self.config.file_managers.clone();
        let settle = self.config.settle_ms;
        cosmic::task::future(async move {
            let res = tokio::task::spawn_blocking(move || {
                selection::grab(&managers, settle).map_err(|e| e.to_string())
            })
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
            Message::Selected(epoch, res)
        })
    }

    fn show_files(&mut self, files: Vec<PathBuf>) -> Task<Message> {
        self.files = files;
        self.index = 0;
        self.page = 0;
        let open = self.ensure_surface();
        Task::batch([open, self.load_current()])
    }

    fn show_hint(&mut self, msg: String) -> Task<Message> {
        let epoch = self.bump();
        self.files.clear();
        self.content = Some(Content::Hint(msg));
        let open = self.ensure_surface();
        let expire = cosmic::task::future(async move {
            tokio::time::sleep(std::time::Duration::from_millis(HINT_MS)).await;
            Message::Expire(epoch)
        });
        Task::batch([open, expire])
    }

    fn ensure_surface(&mut self) -> Task<Message> {
        if self.id.is_some() {
            return Task::none();
        }
        let id = window::Id::unique();
        self.id = Some(id);
        self.surface_size = None;
        get_layer_surface(SctkLayerSurfaceSettings {
            id,
            layer: Layer::Top,
            // Na wyłączność — inaczej spacja i Esc trafiałyby do menedżera
            // plików pod spodem.
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            margin: IcedMargin::default(),
            size: Some((None, None)),
            exclusive_zone: 0,
            namespace: "cosmic-quicklook".into(),
            ..Default::default()
        })
    }

    fn close(&mut self) -> Task<Message> {
        self.bump();
        self.content = None;
        self.files.clear();
        self.surface_size = None;
        match self.id.take() {
            Some(id) => destroy_layer_surface(id),
            None => Task::none(),
        }
    }

    // ---- renderowanie treści ----

    fn load_current(&mut self) -> Task<Message> {
        let Some(path) = self.files.get(self.index).cloned() else {
            return Task::none();
        };
        let epoch = self.bump();
        self.content = Some(Content::Loading(path.clone()));
        let (bw, bh) = self.content_box();
        let page = self.page;
        let opts = preview::Opts {
            text_max_kb: self.config.text_max_kb,
            pdf_dpi: self.config.pdf_dpi,
        };
        cosmic::task::future(async move {
            let p = tokio::task::spawn_blocking(move || preview::load(&path, page, bw, bh, opts))
                .await;
            match p {
                Ok(p) => Message::Loaded(epoch, Box::new(p)),
                Err(e) => Message::Selected(epoch, Err(e.to_string())),
            }
        })
    }

    fn pdf_pages(&self) -> Option<(usize, usize)> {
        match self.content.as_ref() {
            Some(Content::Ready(p)) => match &p.body {
                Body::Pdf { page, pages, .. } => Some((*page, *pages)),
                _ => None,
            },
            _ => None,
        }
    }

    fn blur(&self) -> Task<Message> {
        let (Some(id), Some((sw, sh))) = (self.id, self.surface_size) else {
            return Task::none();
        };
        let (cw, ch) = self.card_size();
        glass::blur_task_rects(
            id,
            self.glass,
            &[((sw - cw) / 2.0, (sh - ch) / 2.0, cw, ch)],
        )
    }
}

impl cosmic::Application for QuickLook {
    type Executor = cosmic::executor::Default;
    type Flags = Config;
    type Message = Message;
    const APP_ID: &'static str = "io.github.longer54.CosmicQuickLook";

    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, config: Self::Flags) -> (Self, Task<Message>) {
        core.window.show_headerbar = false;
        // Region rozmycia wycinamy sami (dokładnie pod kartą) — automatyczny
        // wziąłby całą powierzchnię, czyli cały ekran.
        core.set_auto_blur(Default::default());

        let glass = Glass::from_config(&config.glass);
        let app = QuickLook {
            core,
            config,
            glass,
            id: None,
            surface_size: None,
            files: Vec::new(),
            index: 0,
            page: 0,
            content: None,
            epoch: 0,
        };
        (app, Task::none())
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            ipc::subscription().map(Message::Ipc),
            glass::subscription(self.glass.level()).map(Message::Glass),
        ];
        subs.push(cosmic::iced::event::listen_with(|event, _status, id| {
            use cosmic::iced::keyboard::{Event as Kb, Key, key::Named};
            match event {
                cosmic::iced::Event::Keyboard(Kb::KeyPressed { ref key, .. }) => match key {
                    // Spacja zamyka tak samo jak Esc — w macOS-ie to ta sama
                    // spacja, która otwiera. Globalnie przypiąć jej nie
                    // można (zjadałaby pisanie), ale gdy podgląd jest na
                    // wierzchu, klawiatura należy do nas.
                    // Spacja przychodzi jako znak, nie jako klawisz nazwany.
                    Key::Named(Named::Escape) => Some(Message::Close),
                    Key::Character(c) if c.as_str() == " " => Some(Message::Close),
                    Key::Named(Named::Enter) => Some(Message::OpenExternal),
                    Key::Named(Named::ArrowRight) => Some(Message::Arrow(1)),
                    Key::Named(Named::ArrowLeft) => Some(Message::Arrow(-1)),
                    Key::Named(Named::ArrowDown | Named::PageDown) => Some(Message::Page(1)),
                    Key::Named(Named::ArrowUp | Named::PageUp) => Some(Message::Page(-1)),
                    Key::Named(Named::Tab) => Some(Message::File(1)),
                    _ => None,
                },
                cosmic::iced::Event::Window(window::Event::Resized(size)) => {
                    Some(Message::Resized(id, size.width, size.height))
                }
                _ => None,
            }
        }));
        Subscription::batch(subs)
    }

    fn view(&self) -> Element<'_, Message> {
        text::body("").into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if self.id != Some(id) {
            return text::body("").into();
        }
        self.card_view()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ipc(cmd) => match cmd {
                ipc::Command::Toggle => self.toggle(),
                ipc::Command::Show => {
                    if self.is_open() {
                        Task::none()
                    } else {
                        self.ask_selection()
                    }
                }
                ipc::Command::Hide => self.close(),
                ipc::Command::Path(p) => {
                    if self.is_open() && self.files.first() == Some(&p) {
                        self.close()
                    } else {
                        self.show_files(vec![p])
                    }
                }
            },
            Message::Selected(epoch, res) => {
                if epoch != self.epoch {
                    return Task::none();
                }
                match res {
                    Ok(files) if !files.is_empty() => self.show_files(files),
                    Ok(_) => self.show_hint("Nie widzę zaznaczonego pliku".into()),
                    Err(e) => self.show_hint(e),
                }
            }
            Message::Loaded(epoch, p) => {
                if epoch != self.epoch {
                    return Task::none();
                }
                self.content = Some(Content::Ready(p));
                self.blur()
            }
            Message::Expire(epoch) => {
                if epoch == self.epoch && matches!(self.content, Some(Content::Hint(_))) {
                    self.close()
                } else {
                    Task::none()
                }
            }
            Message::Close => self.close(),
            Message::OpenExternal => {
                if let Some(path) = self.files.get(self.index) {
                    let _ = std::process::Command::new("xdg-open")
                        .arg(path)
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();
                }
                if self.config.close_after_open {
                    self.close()
                } else {
                    Task::none()
                }
            }
            Message::File(step) => {
                if self.files.len() < 2 {
                    return Task::none();
                }
                let n = self.files.len() as i32;
                self.index = (((self.index as i32 + step) % n + n) % n) as usize;
                self.page = 0;
                self.load_current()
            }
            Message::Page(step) => match self.pdf_pages() {
                Some((page, pages)) => {
                    let next = (page as i32 + step).clamp(0, pages as i32 - 1) as usize;
                    if next == page {
                        return Task::none();
                    }
                    self.page = next;
                    self.load_current()
                }
                None => Task::none(),
            },
            Message::Arrow(step) => {
                // Strzałki robią to, co w danej chwili ma sens: kartkują
                // PDF-a, a przy jednostronicowym pliku przeskakują między
                // plikami z zaznaczenia.
                if matches!(self.pdf_pages(), Some((_, pages)) if pages > 1) {
                    self.update(Message::Page(step))
                } else {
                    self.update(Message::File(step))
                }
            }
            Message::Resized(id, w, h) => {
                if self.id != Some(id) {
                    return Task::none();
                }
                let first = self.surface_size.is_none();
                self.surface_size = Some((w, h));
                // Pierwszy realny rozmiar może się różnić od rozmiaru ekranu
                // (panel, dok) — przeliczamy podgląd pod właściwą kartę.
                if first && matches!(self.content, Some(Content::Loading(_) | Content::Ready(_))) {
                    Task::batch([self.load_current(), self.blur()])
                } else {
                    self.blur()
                }
            }
            Message::Glass(g) => {
                self.glass = g;
                self.blur()
            }
            Message::Noop => Task::none(),
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
// Widok
// =========================================================================

impl QuickLook {
    fn card_view(&self) -> Element<'_, Message> {
        let (cw, ch) = self.card_size();
        let inner = match self.content.as_ref() {
            Some(Content::Ready(p)) => self.ready_view(p, cw, ch),
            Some(Content::Loading(path)) => Self::simple_view(
                path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                "Ładowanie…".to_string(),
            ),
            Some(Content::Hint(msg)) => Self::simple_view("Quick Look".into(), msg.clone()),
            None => Self::simple_view("Quick Look".into(), String::new()),
        };

        let card = container(inner)
            .padding(PAD)
            .width(Length::Fixed(cw))
            .height(Length::Fixed(ch))
            .class(cosmic::theme::Container::custom(self.glass.container()));
        // Karta musi pochłonąć własne kliknięcie — inaczej dojdzie do tła
        // i podgląd zamknie się przy próbie zaznaczenia tekstu.
        let card = mouse_area(card).on_press(Message::Noop);

        let alpha = self.config.scrim_opacity.clamp(0.0, 1.0);
        let scrim = container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .class(cosmic::theme::Container::custom(move |_t: &Theme| {
                cosmic::iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, alpha))),
                    ..Default::default()
                }
            }));

        mouse_area(scrim).on_press(Message::Close).into()
    }

    fn simple_view<'a>(title: String, msg: String) -> Element<'a, Message> {
        column::with_children(vec![
            text::title4(title).into(),
            text::body(msg).class(dim()).into(),
        ])
        .spacing(8)
        .into()
    }

    fn ready_view<'a>(&'a self, p: &'a Preview, cw: f32, ch: f32) -> Element<'a, Message> {
        let body_h = (ch - CHROME_H).max(60.0);
        let body_w = (cw - 2.0 * PAD).max(60.0);

        let body: Element<'_, Message> = match &p.body {
            Body::Image { handle, w, h } | Body::Pdf { handle, w, h, .. } => {
                image(handle.clone()).width(*w).height(*h).into()
            }
            Body::Svg { path, w, h } => svg(svg::Handle::from_path(path))
                .width(*w)
                .height(*h)
                .into(),
            Body::Media { thumb, lines } => {
                let mut col = column::with_children(Vec::new()).spacing(8).align_x(Alignment::Center);
                if let Some((handle, w, h)) = thumb {
                    col = col.push(image(handle.clone()).width(*w).height(*h));
                }
                for l in lines {
                    col = col.push(text::body(l.as_str()).class(dim()));
                }
                col.into()
            }
            Body::Text {
                text: body,
                mono,
                truncated,
            } => {
                let t: Element<'_, Message> = if *mono {
                    text::monotext(body.as_str()).into()
                } else {
                    text::body(body.as_str()).into()
                };
                let mut col = column::with_children(vec![t]).spacing(6);
                if *truncated {
                    col = col.push(text::caption("… (dalszy ciąg obcięty)").class(dim()));
                }
                scrollable(col).width(body_w).height(body_h).into()
            }
            Body::Dir { entries, more } => {
                let mut col = column::with_children(Vec::new()).spacing(2);
                for e in entries {
                    col = col.push(text::body(e.as_str()));
                }
                if *more > 0 {
                    col = col.push(text::caption(format!("… i jeszcze {more}")).class(dim()));
                }
                scrollable(col).width(body_w).height(body_h).into()
            }
            Body::Info(msg) => container(text::body(msg.as_str()).class(dim()))
                .width(body_w)
                .height(body_h)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into(),
        };

        let body = container(body)
            .width(Length::Fill)
            .height(Length::Fixed(body_h))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center);

        column::with_children(vec![
            self.header(p, cw - 2.0 * PAD),
            divider::horizontal::default().into(),
            body.into(),
            self.footer(p),
        ])
        .spacing(8)
        .into()
    }

    /// Nagłówek musi zmieścić się w karcie: iced nie przycina tekstu, który
    /// się nie mieści — dłuższa nazwa pliku po prostu wylazłaby poza
    /// zaokrągloną krawędź (widziane na pierwszym screenie).
    fn header<'a>(&'a self, p: &'a Preview, w: f32) -> Element<'a, Message> {
        // Ile znaków wchodzi w wiersz — przy tych krojach ~2 px na znak dla
        // tytułu i ~1,5 dla podpisu; z zapasem, bo lepiej skrócić za wcześnie.
        let title = preview::elide(&p.title, (w / 9.5) as usize);
        let info = preview::elide(&p.info, (w / 5.6) as usize);
        let mut left = column::with_children(vec![
            text::title4(title).into(),
            text::caption(info).class(dim()).into(),
        ])
        .spacing(2);
        if self.files.len() > 1 {
            left = left.push(
                text::caption(format!("plik {} z {}", self.index + 1, self.files.len()))
                    .class(dim()),
            );
        }
        container(left).width(Length::Fixed(w)).into()
    }

    fn footer<'a>(&self, p: &'a Preview) -> Element<'a, Message> {
        let mut hints = vec!["Spacja / Esc — zamknij".to_string(), "Enter — otwórz".into()];
        if let Body::Pdf { page, pages, .. } = &p.body {
            if *pages > 1 {
                hints.push(format!("← → strona {} z {pages}", page + 1));
            }
        } else if self.files.len() > 1 {
            hints.push("← → następny plik".into());
        }
        text::caption(hints.join("   ·   ")).class(dim()).into()
    }
}
