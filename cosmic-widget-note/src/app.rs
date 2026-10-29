//! Widget „Notatka” — kartka na pulpicie (layer-shell, jak kalendarz),
//! w której użytkownik pisze wprost na pulpicie. Jedna, OGÓLNA notatka (nie
//! dzienna), trzymana jako zwykły plik `.md` w vaultcie Obsidian, więc
//! ta sama treść jest widoczna i edytowalna w Obsidianie.
//!
//! Dwa stany karty:
//! - **podgląd** — klawiatura powierzchni wyłączona
//!   (`KeyboardInteractivity::None`), więc widget nie kradnie fokusu oknom;
//! - **edycja** — po kliknięciu w kartkę: klawiatura `OnDemand`, kursor w
//!   edytorze, autozapis po chwili bezczynności. `Esc` (albo „Gotowe”)
//!   kończy edycję i zapisuje.
//!
//! Zapisy idą tam, gdzie zapisy widgetu zadań — na kanoniczną kopię vaulta
//! na serwerze (SSH), żeby nie robić `*.sync-conflict-*`; odczyt jest lokalny.

use std::time::{Duration, Instant};

use cosmic::app::{Core, Task};
use cosmic::iced::border::Border;
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{
    IcedMargin, SctkLayerSurfaceSettings,
};
use cosmic::iced::platform_specific::shell::commands::layer_surface::{
    destroy_layer_surface, get_layer_surface, set_keyboard_interactivity, set_margin, Anchor,
    KeyboardInteractivity, Layer,
};
use cosmic::iced::widget::container;
use cosmic::iced::{keyboard, window, Background, Color, Length, Padding, Shadow, Subscription, Vector};
use cosmic::widget::{mouse_area, text, text_editor};
use cosmic::Element;
use cosmic_widget_common::{desktop, drag, glass, Glass, Outcome, WidgetPos};

use crate::config::Config;
use crate::ipc;
use crate::note::{self, Note};

/// Identyfikator edytora — po nim wołamy fokus po wejściu w tryb edycji.
const EDITOR_ID: &str = "cosmic-widget-note-editor";

/// Kremowa kartka jak w Notatkach macOS (to o ten wygląd prosił użytkownik).
const PAPER_BG: Color = Color::from_rgb(0.988, 0.965, 0.878);
const INK: Color = Color::from_rgb(0.16, 0.15, 0.13);
const INK_DIM: Color = Color::from_rgba(0.16, 0.15, 0.13, 0.55);
const PAPER_SELECTION: Color = Color::from_rgba(0.95, 0.78, 0.28, 0.55);
/// Kolory dla wariantu „szkło” (`paper = false`) — jak w reszcie widgetów.
const GLASS_INK: Color = Color::from_rgba(0.95, 0.95, 0.97, 0.92);
const GLASS_DIM: Color = Color::from_rgba(0.95, 0.95, 0.97, 0.55);

const RADIUS: f32 = 16.0;
/// Nieudany zapis (np. padnięty SSH) — nie próbuj częściej niż co tyle.
const RETRY: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub enum Message {
    /// Edycja w polu tekstowym.
    Edit(text_editor::Action),
    /// Klik w kartkę — wejście w tryb pisania.
    StartEdit,
    /// „Gotowe” albo `Esc` — zapisz i wyjdź z trybu pisania.
    StopEdit,
    /// `Ctrl+S` — zapisz, ale zostań w edycji.
    SaveNow,
    /// Zegar: autozapis w trakcie pisania, poza edycją — odświeżenie
    /// treści zmienionej w Obsidianie albo przyniesionej Syncthingiem.
    Tick,
    /// Otwórz notatkę poza widgetem — plik `.md` systemowo albo (gdy
    /// `open_in_obsidian = true`) w Obsidianie.
    OpenNote,
    DragStart,
    Drag(cosmic_widget_common::DragEvent),
    StackTick,
    DesktopVisible(bool),
    Glass(cosmic_widget_common::Glass),
    /// Polecenie z gniazda (skrót Super+Shift+N).
    Ipc(ipc::Command),
}

pub struct Widget {
    core: Core,
    surface_id: window::Id,
    config: Config,
    pos: WidgetPos,
    glass: Glass,
    note: Note,
    content: text_editor::Content,
    /// Treść, o której wiemy, że jest zapisana — do porównania z plikiem
    /// przy odświeżaniu (bez tego każdy tick nadpisywałby kursor).
    saved_text: String,
    editing: bool,
    dirty: bool,
    last_edit: Option<Instant>,
    last_attempt: Option<Instant>,
    saved_at: Option<String>,
    error: Option<String>,
    /// Komunikat po odłożeniu notatki do archiwum — znika przy pisaniu.
    notice: Option<String>,
    visible: bool,
}

fn anchor_from_str(s: &str) -> Anchor {
    match s {
        "top-right" => Anchor::TOP | Anchor::RIGHT,
        "bottom-left" => Anchor::BOTTOM | Anchor::LEFT,
        "bottom-right" => Anchor::BOTTOM | Anchor::RIGHT,
        _ => Anchor::TOP | Anchor::LEFT,
    }
}

/// Ustawienia powierzchni layer-shell. Wydzielone, bo widget odtwarza ją po
/// powrocie na swój pulpit wirtualny (`Message::DesktopVisible`).
fn surface_settings(
    id: window::Id,
    anchor: &str,
    margin_x: i32,
    margin_y: i32,
    width: u32,
    height: u32,
    keyboard: bool,
) -> SctkLayerSurfaceSettings {
    SctkLayerSurfaceSettings {
        id,
        // Layer::Bottom jak w kalendarzu — Background chowa widget pod
        // tapetą plasmashella (sprawdzone 2026-08-11).
        layer: Layer::Bottom,
        // W trybie pisania `Exclusive`, nie `OnDemand`: samo przełączenie
        // interaktywności na `OnDemand` nie daje powierzchni fokusu
        // klawiatury dopóki użytkownik nie kliknie jej PONOWNIE (klik, który
        // włączył edycję, kompozytor rozpatrzył jeszcze przy `None`), więc
        // pierwsze wpisane znaki znikały — kartka wyglądała na „zablokowaną”.
        // `Exclusive` kieruje klawiaturę tu natychmiast po commitcie; edycja
        // to i tak stan chwilowy, kończony `Esc`/„Gotowe”.
        keyboard_interactivity: if keyboard {
            KeyboardInteractivity::Exclusive
        } else {
            KeyboardInteractivity::None
        },
        anchor: anchor_from_str(anchor),
        margin: IcedMargin {
            top: margin_y,
            right: margin_x,
            bottom: margin_y,
            left: margin_x,
        },
        size: Some((Some(width), Some(height))),
        exclusive_zone: 0,
        namespace: "cosmic-widget-note".into(),
        ..Default::default()
    }
}

impl cosmic::Application for Widget {
    type Executor = cosmic::executor::Default;
    type Flags = Config;
    type Message = Message;
    const APP_ID: &'static str = "io.github.longer54.CosmicWidgetNote";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, config: Config) -> (Self, Task<Message>) {
        core.window.show_headerbar = false;
        // Region rozmycia podajemy sami, dokładnie pod kartą (glass.rs).
        core.set_auto_blur(Default::default());

        let surface_id = window::Id::unique();
        let note = Note::from_config(&config);
        let text = note.load().unwrap_or_else(|e| {
            eprintln!("cosmic-widget-note: nie mogę odczytać {}: {e}", note.path().display());
            String::new()
        });

        // Powierzchnia POWSTAJE PO `WidgetPos` i z jego marginesów, nie
        // z surowego configu — patrz `WidgetPos::new` (config bywa
        // nieświeży, prawdziwą pozycję zna opublikowana geometria).
        let mut pos = WidgetPos::new(
            "note",
            &config.anchor,
            config.margin_x,
            config.margin_y,
            config.width,
            Some(config.stack_below.clone()),
            config.stack_gap,
            config.stack_offset_y,
        );
        pos.set_height(config.height);

        let surface_task = get_layer_surface(surface_settings(
            surface_id,
            &config.anchor,
            pos.margin_x(),
            pos.margin_y(),
            config.width,
            config.height,
            false,
        ));

        let glass = Glass::from_config(&config.glass);
        let widget = Widget {
            core,
            surface_id,
            pos,
            glass,
            content: text_editor::Content::with_text(&text),
            saved_text: note::normalize(&text),
            note,
            config,
            editing: false,
            dirty: false,
            last_edit: None,
            last_attempt: None,
            saved_at: None,
            notice: None,
            error: None,
            visible: true,
        };

        let blur = widget.blur_task();
        (widget, Task::batch([surface_task, blur]))
    }

    /// Główne okno jest wyłączone — pusty widok.
    fn view(&self) -> Element<'_, Message> {
        text::body("").into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if id != self.surface_id {
            return text::body("").into();
        }
        self.build_card()
    }

    fn subscription(&self) -> Subscription<Message> {
        // W trakcie pisania (albo z niezapisaną treścią) tykamy co sekundę —
        // to od tego zależy autozapis. Poza tym rzadko, tylko żeby złapać
        // zmianę zrobioną w Obsidianie.
        let period = if self.editing || self.dirty {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(self.config.reload_secs.max(2))
        };
        let mut subs = vec![
            cosmic::iced::time::every(period).map(|_| Message::Tick),
            ipc::subscription().map(Message::Ipc),
        ];

        if self.pos.dragging() {
            subs.push(drag::subscription().map(Message::Drag));
        }
        if self.pos.is_stacked() && self.visible {
            subs.push(self.pos.stack_subscription().map(|()| Message::StackTick));
        }
        if !self.config.paper {
            subs.push(glass::subscription(self.glass.level()).map(Message::Glass));
        }
        if self.config.only_on_desktop > 0 {
            subs.push(desktop::subscription(self.config.only_on_desktop).map(Message::DesktopVisible));
        }
        Subscription::batch(subs)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(action) => {
                let edited = action.is_edit();
                self.content.perform(action);
                if edited {
                    self.notice = None;
                    self.dirty = true;
                    self.last_edit = Some(Instant::now());
                }
                Task::none()
            }
            Message::StartEdit => {
                if self.editing {
                    return Task::none();
                }
                self.editing = true;
                Task::batch([
                    self.keyboard_task(),
                    // Operacja fokusu jest wspólna dla wszystkich pól
                    // tekstowych iceda — `text_editor` też ją obsługuje,
                    // więc ta funkcja z modułu `text_input` ustawia kursor
                    // w naszym edytorze.
                    cosmic::widget::text_input::focus(cosmic::widget::Id::new(EDITOR_ID)),
                ])
            }
            Message::Ipc(ipc::Command::Edit) => self.update(Message::StartEdit),
            Message::Ipc(ipc::Command::New) => {
                // Najpierw archiwum, dopiero potem czyszczenie — przy
                // nieudanym zapisie do archiwum kartka zostaje nietknięta.
                let text = self.content.text();
                if !note::normalize(&text).is_empty() {
                    match self.note.archive(&text) {
                        Ok(path) => {
                            let name = path
                                .file_stem()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            self.notice = Some(format!(
                                "poprzednia → {}/{name}",
                                self.config.archive_folder
                            ));
                        }
                        Err(e) => {
                            eprintln!("cosmic-widget-note: nie mogę zarchiwizować: {e}");
                            self.error = Some(format!("archiwum: {e}"));
                            return Task::none();
                        }
                    }
                }
                self.content = text_editor::Content::new();
                self.save();
                if self.editing {
                    cosmic::widget::text_input::focus(cosmic::widget::Id::new(EDITOR_ID))
                } else {
                    self.update(Message::StartEdit)
                }
            }
            Message::StopEdit => {
                self.editing = false;
                if self.dirty {
                    self.save();
                }
                self.keyboard_task()
            }
            Message::SaveNow => {
                self.save();
                Task::none()
            }
            Message::Tick => {
                if self.dirty {
                    let idle = self
                        .last_edit
                        .is_none_or(|t| t.elapsed() >= Duration::from_secs(self.config.autosave_secs));
                    // Po nieudanym zapisie nie dobijamy się do serwera co
                    // sekundę — próbujemy co RETRY.
                    let may_retry = self.error.is_none()
                        || self.last_attempt.is_none_or(|t| t.elapsed() >= RETRY);
                    if idle && may_retry {
                        self.save();
                    }
                } else if !self.editing {
                    self.reload_if_changed();
                }
                Task::none()
            }
            Message::OpenNote => {
                // Otworzyć da się tylko istniejący plik — jeśli użytkownik jeszcze
                // nic nie zapisał, tworzymy notatkę teraz.
                if self.dirty || !self.note.path().exists() {
                    self.save();
                }
                if self.config.open_in_obsidian {
                    note::open_uri(&self.note.open_uri());
                } else {
                    note::open_file(self.note.path());
                }
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
                            self.config.height,
                            self.editing,
                        )),
                        self.blur_task(),
                    ])
                } else {
                    // Zjeżdżając na inny pulpit nie gub tego, co napisane.
                    if self.dirty {
                        self.save();
                    }
                    destroy_layer_surface(self.surface_id)
                }
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

impl Widget {
    /// Klawiatura włączona TYLKO w trybie pisania — inaczej kompozytor
    /// trzymałby widget w warstwie aktywnej, nad oknami aplikacji
    /// (ta sama pułapka co przy polu „Do zapamiętania” w widgetcie zadań).
    /// W edycji `Exclusive`, nie `OnDemand` — patrz komentarz w
    /// `surface_settings`: `OnDemand` po przełączeniu w locie nie łapie
    /// fokusu klawiatury bez kolejnego kliknięcia i nie dało się pisać.
    fn keyboard_task(&self) -> Task<Message> {
        set_keyboard_interactivity(
            self.surface_id,
            if self.editing {
                KeyboardInteractivity::Exclusive
            } else {
                KeyboardInteractivity::None
            },
        )
    }

    fn blur_task(&self) -> Task<Message> {
        // Kremowa kartka jest nieprzezroczysta — nie ma czego rozmywać.
        if self.config.paper {
            return Task::none();
        }
        glass::blur_task_single(self.surface_id, self.glass, self.config.width, self.config.height)
    }

    fn margin_task(&self) -> Task<Message> {
        set_margin(
            self.surface_id,
            self.pos.margin_y(),
            self.pos.margin_x(),
            self.pos.margin_y(),
            self.pos.margin_x(),
        )
    }

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

    fn save(&mut self) {
        let text = self.content.text();
        self.last_attempt = Some(Instant::now());
        match self.note.save(&text) {
            Ok(()) => {
                self.saved_text = note::normalize(&text);
                self.dirty = false;
                self.error = None;
                self.saved_at = Some(jiff::Zoned::now().strftime("%H:%M").to_string());
            }
            Err(e) => {
                eprintln!("cosmic-widget-note: nie mogę zapisać notatki: {e}");
                self.error = Some(e.to_string());
            }
        }
    }

    /// Notatkę można zmienić też w Obsidianie (albo na drugim komputerze —
    /// Syncthing przyniesie plik). Poza trybem pisania podciągamy taką
    /// zmianę; własny zapis rozpoznajemy po `saved_text`, więc powrót
    /// naszej treści z Syncthinga nie rusza kursora.
    fn reload_if_changed(&mut self) {
        let Ok(disk) = self.note.load() else { return };
        let disk = note::normalize(&disk);
        if disk == self.saved_text {
            return;
        }
        self.content = text_editor::Content::with_text(&disk);
        self.saved_text = disk;
    }
}

// =========================================================================
// UI
// =========================================================================

impl Widget {
    fn ink(&self) -> Color {
        if self.config.paper { INK } else { GLASS_INK }
    }

    fn dim(&self) -> Color {
        if self.config.paper { INK_DIM } else { GLASS_DIM }
    }

    fn build_card(&self) -> Element<'_, Message> {
        let ink = self.ink();
        let dim = self.dim();

        // ---- Nagłówek: uchwyt ⠿ · tytuł · akcja po prawej ----
        let action: Element<'_, Message> = if self.editing {
            mouse_area(
                cosmic::widget::container(
                    text::body("Gotowe").class(cosmic::theme::Text::Color(ink)),
                )
                .padding([0, 4]),
            )
            .on_press(Message::StopEdit)
            .into()
        } else {
            let label = if self.config.open_in_obsidian {
                "Obsidian ↗"
            } else {
                // Zwykły plik .md w systemowym edytorze — tak jak kalendarz
                // z wyłączonym przełącznikiem „Obsidian" (życzenie użytkownika).
                "Otwórz ↗"
            };
            mouse_area(
                cosmic::widget::container(
                    text::caption(label).class(cosmic::theme::Text::Color(dim)),
                )
                .padding([0, 4]),
            )
            .on_press(Message::OpenNote)
            .into()
        };

        // Cały pasek nagłówka (uchwyt ⠿ + tytuł) jest chwytem do
        // przeciągania — sam ⠿ bywał trudny do trafienia, a na kremowej
        // kartce dodatkowo niewidoczny (uchwyt z common jest na sztywno
        // jasny, czyli biały na papierze). Przyciski po prawej mają własny
        // mouse_area, więc łapią klik przed przeciąganiem.
        let title = mouse_area(
            cosmic::widget::container(
                text::heading(self.config.title.clone())
                    .class(cosmic::theme::Text::Color(ink)),
            )
            .width(Length::Fill),
        )
        .on_press(Message::DragStart);

        let header = cosmic::widget::row::with_children(vec![
            grip(dim),
            title.into(),
            action,
        ])
        .spacing(6)
        .align_y(cosmic::iced::Alignment::Center);

        // ---- Treść notatki ----
        // W edycji: `text_editor` (kółko myszy przewija w nim natywnie).
        // Poza edycją edytor i tak nie łapie zdarzeń, więc dłuższą notatkę
        // pokazujemy jako zwykły tekst w `scrollable` — kółko przewija
        // treść, pasek po prawej, a klik w tekst dalej wchodzi w pisanie.
        let paper = self.config.paper;
        let font_size = f32::from(self.config.font_size);

        let content_view: Element<'_, Message> = if self.editing {
            text_editor(&self.content)
                .id(cosmic::widget::Id::new(EDITOR_ID))
                .placeholder("Kliknij i pisz…")
                .padding(Padding::ZERO)
                .size(font_size)
                .height(Length::Fill)
                .class(cosmic::theme::style::iced::TextEditor::Custom(Box::new(
                    move |_theme, _status| text_editor::Style {
                        // Tło i ramka są na karcie — samo pole ma zniknąć.
                        background: Background::Color(Color::TRANSPARENT),
                        border: Border {
                            radius: 0.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        placeholder: if paper { INK_DIM } else { GLASS_DIM },
                        value: if paper { INK } else { GLASS_INK },
                        selection: if paper {
                            PAPER_SELECTION
                        } else {
                            Color::from_rgba(0.4, 0.6, 1.0, 0.45)
                        },
                    },
                )))
                .on_action(Message::Edit)
                .key_binding(|kp| match kp.key.as_ref() {
                    // Esc kończy pisanie (i zapisuje), Ctrl+S zapisuje
                    // w locie — reszta klawiszy jak w zwykłym edytorze.
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        Some(text_editor::Binding::Custom(Message::StopEdit))
                    }
                    keyboard::Key::Character("s") if kp.modifiers.control() => {
                        Some(text_editor::Binding::Custom(Message::SaveNow))
                    }
                    _ => text_editor::Binding::from_key_press(kp),
                })
                .into()
        } else {
            let raw = self.content.text();
            let shown = raw.trim_end_matches('\n');
            let inner: Element<'_, Message> = if shown.trim().is_empty() {
                text("Kliknij i pisz…")
                    .size(font_size)
                    .width(Length::Fill)
                    .class(cosmic::theme::Text::Color(dim))
                    .into()
            } else {
                text(shown.to_string())
                    .size(font_size)
                    .width(Length::Fill)
                    .class(cosmic::theme::Text::Color(ink))
                    .into()
            };
            // Bez widocznego paska — użytkownik go nie chce; kółko myszy przewija
            // i tak (obsługa scrolla nie zależy od widoczności paska).
            cosmic::widget::scrollable(inner)
                .scrollbar_width(0.0)
                .scroller_width(0.0)
                .scrollbar_padding(0.0)
                .height(Length::Fill)
                .width(Length::Fill)
                .into()
        };

        // ---- Stopka: co się dzieje z zapisem ----
        let status = if let Some(err) = &self.error {
            format!("nie zapisano: {err}")
        } else if self.dirty {
            "piszę…".to_string()
        } else if let Some(n) = &self.notice {
            n.clone()
        } else if let Some(at) = &self.saved_at {
            format!("zapisano {at}  ·  {}", self.note.label())
        } else {
            self.note.label().to_string()
        };

        let body = cosmic::widget::column::with_children(vec![
            header.into(),
            cosmic::widget::container(content_view)
                .height(Length::Fill)
                .padding([8, 0, 6, 0])
                .into(),
            text::caption(status)
                .class(cosmic::theme::Text::Color(dim))
                .into(),
        ]);

        let card = cosmic::widget::container(body)
            .padding([12, 14])
            .width(Length::Fixed(self.config.width as f32))
            .height(Length::Fixed(self.config.height as f32))
            .class(if self.config.paper {
                cosmic::theme::Container::custom(paper_card())
            } else {
                cosmic::theme::Container::custom(self.glass.container())
            });

        if self.editing {
            card.into()
        } else {
            // Poza trybem pisania edytor nie łapie zdarzeń (nie ma
            // `on_action`), więc klik w dowolne miejsce kartki trafia tutaj
            // i włącza pisanie.
            mouse_area(card).on_press(Message::StartEdit).into()
        }
    }
}

/// Uchwyt ⠿ w kolorze pasującym do karty (odpowiednik `drag::handle`
/// z cosmic-widget-common, który ma kolor na sztywno).
fn grip(color: Color) -> Element<'static, Message> {
    let glyph = text::body("⠿").class(cosmic::theme::Text::Color(color));
    mouse_area(cosmic::widget::container(glyph).padding([0, 6]))
        .on_press(Message::DragStart)
        .into()
}

/// Kremowa kartka jak w Notatkach macOS: ciepłe tło, ciemny tekst,
/// zaokrąglone rogi i miękki cień.
fn paper_card() -> impl Fn(&cosmic::Theme) -> container::Style + 'static {
    move |_theme| container::Style {
        icon_color: Some(INK),
        text_color: Some(INK),
        background: Some(Background::Color(PAPER_BG)),
        border: Border {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.10),
            width: 1.0,
            radius: RADIUS.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
            offset: Vector::new(0.0, 6.0),
            blur_radius: 24.0,
        },
        ..Default::default()
    }
}
