//! Widget layer-shell z dwiema kartami: zaległe/dzisiejsze zadania ze skanu
//! vaulta Obsidian (składnia wtyczki Tasks) i "Do zapamiętania" (zadania
//! otagowane `#pamięć`, bez terminu, sort po priorytecie).
//!
//! Każda karta ma WŁASNĄ powierzchnię layer-shell, więc obie przesuwa się
//! niezależnie (życzenie użytkownika 2026-08-26). Historia: najpierw były dwa
//! osobne procesy — synchronizacja odstępu między nimi (plik w cache +
//! przebudowa surface'a) okazała się zawodna, więc karty zlepiono w jedno
//! okno. Teraz są znów rozdzielone, ale w JEDNYM procesie i na wspólnym
//! mechanizmie `WidgetPos` (przeciąganie + `stack_below`), tym samym, który
//! trzyma w kolumnie kalendarz pod panelem — czyli bez tamtej zawodnej
//! ścieżki. Domyślnie karta pamięci stoi w stacku pod kartą zadań; puste
//! `memory_stack_below` w configu odwiązuje ją całkiem.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration as StdDuration;

use cosmic::app::{Core, Task};
use cosmic::iced::border::Border;
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{
    IcedMargin, SctkLayerSurfaceSettings,
};
use cosmic::iced::platform_specific::shell::commands::layer_surface::{
    destroy_layer_surface, get_layer_surface, set_keyboard_interactivity, set_margin,
    set_size, Anchor, KeyboardInteractivity, Layer,
};
use cosmic::iced::window;
use cosmic::iced::{Background, Color, Length, Subscription};
use cosmic::widget::{button, container, mouse_area, text, text_input};
use cosmic::Element;
use cosmic_widget_common::{desktop, drag, glass, Glass, Outcome, Remote, WidgetPos};

use crate::config::Config;
use crate::tasks::{
    append_memory_item, mark_task_done, scan_memory_items, scan_tasks, MemoryItem, Priority,
    TaskItem,
};

/// Strona zadań w Obsidianie, względem vaulta, bez rozszerzenia.
const TODAY_PAGE: &str = "Organizer/♨️Dziś";

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Scanned(Result<Vec<TaskItem>, String>),
    MemoryScanned(Result<Vec<MemoryItem>, String>),
    /// Szybki podwójny klik na wierszu zadania — odhacza je w pliku.
    MarkDone(usize),
    /// Szybki podwójny klik na wierszu "Do zapamiętania".
    MarkMemoryDone(usize),
    /// Przycisk "Dziś" w nagłówku — jedyne miejsce, które otwiera Obsidiana.
    OpenTodayPage,
    /// Przycisk "+" w sekcji pamięci — otwiera/zamyka pole szybkiego dodawania.
    ToggleInput,
    InputChanged(String),
    SubmitInput,
    /// Uchwyt ⠿ wciśnięty na danej karcie — start przeciągania.
    DragStart(Card),
    Drag(drag::DragEvent),
    StackTick,
    /// Zmiana pulpitu wirtualnego w KWin: `true` = jesteśmy na „swoim"
    /// pulpicie (pokaż powierzchnię), `false` = na innym (schowaj).
    DesktopVisible(bool),
    /// Nowy poziom oszklenia — użytkownik ruszył suwakiem przezroczystości doka.
    Glass(Glass),
}

/// Która z dwóch kart — każda ma własną powierzchnię i własną pozycję.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Card {
    Tasks,
    Memory,
}

pub struct Widget {
    core: Core,
    tasks_surface: window::Id,
    memory_surface: window::Id,
    config: Config,
    tasks_pos: WidgetPos,
    memory_pos: WidgetPos,
    /// Szkło kart: przezroczystość + region rozmycia tła.
    glass: Glass,
    tasks: Option<Vec<TaskItem>>,
    last_error: Option<String>,
    memory_items: Option<Vec<MemoryItem>>,
    memory_error: Option<String>,
    input_open: bool,
    input_value: String,
    /// Zadania odhaczone na serwerze, których zmiana jeszcze nie wróciła tu
    /// Syncthingiem. Lokalny skan wciąż widzi je jako niezrobione, więc
    /// chowamy je z listy — inaczej użytkownik kliknąłby to samo zadanie drugi raz.
    /// Wpis znika sam, gdy skan przestaje je pokazywać (czyli po synchronizacji).
    /// Osobno dla obu sekcji, bo każda ma własny skan — wspólny zbiór
    /// kasowałby wpisy tej drugiej przy każdym odświeżeniu.
    pending_done: HashSet<(PathBuf, String)>,
    pending_memory_done: HashSet<(PathBuf, String)>,
    /// Czy powierzchnia layer-shell istnieje w tej chwili. `false` = jesteśmy
    /// na innym pulpicie wirtualnym i widget jest schowany.
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
    namespace: &str,
    anchor: &str,
    margin_x: i32,
    margin_y: i32,
    width: u32,
    height: u32,
) -> SctkLayerSurfaceSettings {
    SctkLayerSurfaceSettings {
        id,
        // NIE zmieniać na Layer::Background (sprawdzone 2026-08-11): klik w
        // tapetę podnosi okno pulpitu plasmashella nad tamtą warstwę i widget
        // znika z ekranu. Pokazywaniem się na innych pulpitach zajmuje się
        // `desktop::subscription`, nie warstwa.
        layer: Layer::Bottom,
        // Klawiatura TYLKO na czas pisania w polu szybkiego dodawania
        // (Message::ToggleInput/SubmitInput). Na stałe włączona sprawiała,
        // że KWin po kliknięciu w widget przenosił go do warstwy aktywnej —
        // czyli nad okna aplikacji, na wszystkich pulpitach.
        keyboard_interactivity: KeyboardInteractivity::None,
        anchor: anchor_from_str(anchor),
        margin: IcedMargin {
            top: margin_y,
            right: margin_x,
            bottom: margin_y,
            left: margin_x,
        },
        size: Some((Some(width), Some(height))),
        exclusive_zone: 0,
        namespace: namespace.to_string(),
        ..Default::default()
    }
}

/// Minimalny percent-encoding — jak w cosmic-calendar-widget/src/vault.rs.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn due_label(t: &TaskItem) -> String {
    if t.overdue_days == 0 {
        "dziś".into()
    } else if t.overdue_days == 1 {
        "wczoraj".into()
    } else {
        format!("{} dni temu", t.overdue_days)
    }
}

impl Widget {
    fn fetch_task(&self) -> Task<Message> {
        let vault_path = self.config.vault_path.clone();
        let max_tasks = self.config.max_tasks;
        cosmic::task::future(async move { Message::Scanned(scan_tasks(&vault_path, max_tasks)) })
    }

    /// Dokąd kierować zapisy — kanoniczna kopia vaulta na serwerze.
    /// `None` (pusty `remote_host` w configu) = zapis lokalny, jak dawniej.
    fn remote(&self) -> Option<Remote> {
        Remote::from_config(&self.config.remote_host, &self.config.remote_vault_path)
    }

    fn fetch_memory(&self) -> Task<Message> {
        let vault_path = self.config.vault_path.clone();
        let max_items = self.config.max_memory_items;
        cosmic::task::future(async move {
            Message::MemoryScanned(scan_memory_items(&vault_path, max_items))
        })
    }

    fn tasks_card_height(&self) -> u32 {
        let mut h: u32 = 12 + 20; // padding + wąski pasek uchwytu
        if self.last_error.is_some() {
            h += 30;
        }
        match &self.tasks {
            None => h += 32,
            Some(tasks) if tasks.is_empty() => h += 32,
            // Zostawiony bufor po usunięciu tooltipów per-wiersz (były
            // niestabilne w tym layer-shell surface'ie — patrz komentarz przy
            // budowaniu wierszy). Lepiej trochę pustego miejsca u dołu karty
            // niż przycięte wiersze poza realnym rozmiarem powierzchni.
            Some(tasks) => {
                let cpl = self.chars_per_line(80 + 8);
                h += rows_height(tasks.iter().map(|t| t.text.as_str()), cpl) + 16;
                if tasks.iter().any(|t| t.is_waiting()) {
                    h += WAITING_DIVIDER_H;
                }
            }
        }
        h.clamp(70, 1600)
    }

    fn memory_card_height(&self) -> u32 {
        let mut h: u32 = 12 + 20; // padding + wąski pasek uchwytu
        if self.input_open {
            h += 44;
        }
        if self.memory_error.is_some() {
            h += 30;
        }
        match &self.memory_items {
            None => h += 32,
            Some(items) if items.is_empty() => h += 32,
            Some(items) => {
                let cpl = self.chars_per_line(24 + 8);
                h += rows_height(items.iter().map(|t| t.text.as_str()), cpl) + 16;
            }
        }
        h.clamp(70, 1600)
    }

    /// Ile znaków tekstu wpisu mieści się w jednym wierszu karty.
    /// `reserved` to szerokość kolumn przed tekstem (etykieta terminu /
    /// znaczek priorytetu) wraz z odstępem.
    fn chars_per_line(&self, reserved: u32) -> usize {
        // 28 = padding karty, 14 = kropka + jej odstęp.
        let text_px = self.config.width.saturating_sub(28 + 14 + reserved);
        (text_px / AVG_CHAR_PX).max(8) as usize
    }

    fn surface_of(&self, card: Card) -> window::Id {
        match card {
            Card::Tasks => self.tasks_surface,
            Card::Memory => self.memory_surface,
        }
    }

    fn pos_of(&self, card: Card) -> &WidgetPos {
        match card {
            Card::Tasks => &self.tasks_pos,
            Card::Memory => &self.memory_pos,
        }
    }

    fn pos_of_mut(&mut self, card: Card) -> &mut WidgetPos {
        match card {
            Card::Tasks => &mut self.tasks_pos,
            Card::Memory => &mut self.memory_pos,
        }
    }

    /// Klawiatura włączona tylko przy otwartym polu dodawania — inaczej
    /// KWin trzyma widget w warstwie aktywnej, nad oknami aplikacji.
    /// Dotyczy wyłącznie karty pamięci, bo tam jest pole tekstowe.
    fn keyboard_task(&self) -> Task<Message> {
        set_keyboard_interactivity(
            self.memory_surface,
            if self.input_open {
                KeyboardInteractivity::OnDemand
            } else {
                KeyboardInteractivity::None
            },
        )
    }

    /// Obie karty naraz — wysokość każdej zależy od własnej listy.
    fn resize_task(&mut self) -> Task<Message> {
        let th = self.tasks_card_height();
        let mh = self.memory_card_height();
        self.tasks_pos.set_height(th);
        self.memory_pos.set_height(mh);
        Task::batch([
            set_size(self.tasks_surface, Some(self.config.width), Some(th)),
            set_size(self.memory_surface, Some(self.config.width), Some(mh)),
            self.blur_task(),
        ])
    }

    /// Region rozmycia tła — osobno pod każdą z dwóch kart, żeby szkło nie
    /// wypełniło przezroczystej szpary między nimi. Kompozytor gubi region
    /// razem z powierzchnią, więc wołamy to po każdym jej utworzeniu i po
    /// każdej zmianie rozmiaru.
    fn blur_task(&self) -> Task<Message> {
        Task::batch([
            glass::blur_task_single(
                self.tasks_surface,
                self.glass,
                self.config.width,
                self.tasks_card_height(),
            ),
            glass::blur_task_single(
                self.memory_surface,
                self.glass,
                self.config.width,
                self.memory_card_height(),
            ),
        ])
    }

    /// Zastosuj aktualną pozycję danej karty na jej powierzchni.
    fn margin_task(&self, card: Card) -> Task<Message> {
        let pos = self.pos_of(card);
        set_margin(
            self.surface_of(card),
            pos.margin_y(),
            pos.margin_x(),
            pos.margin_y(),
            pos.margin_x(),
        )
    }

    /// Koniec przeciągania: utrwal nową pozycję karty w configu.
    fn save_position(&mut self, card: Card) {
        let pos = self.pos_of(card);
        let (x, stacked, offset, y) =
            (pos.margin_x(), pos.is_stacked(), pos.stack_offset_y(), pos.margin_y());
        match card {
        // `margin_y` zapisujemy ZAWSZE, także w stacku: to jedyna kopia
        // pozycji, która przeżyje wyczyszczenie `~/.cache`. Wcześniej
        // stackowany widget zapisywał wyłącznie `stack_offset_y`, więc
        // `margin_y` w configu zostawał skamieliną sprzed miesięcy.
            Card::Tasks => {
                self.config.margin_x = x;
                self.config.margin_y = y;
                if stacked {
                    self.config.stack_offset_y = offset;
                }
            }
            Card::Memory => {
                self.config.memory_margin_x = x;
                self.config.memory_margin_y = y;
                if stacked {
                    self.config.memory_stack_offset_y = offset;
                }
            }
        }
        self.config.save();
        self.pos_of(card).publish();
    }
}

impl cosmic::Application for Widget {
    type Executor = cosmic::executor::Default;
    type Flags = Config;
    type Message = Message;
    const APP_ID: &'static str = "io.github.longer54.CosmicWidgetTasks";

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

        let tasks_surface = window::Id::unique();
        let memory_surface = window::Id::unique();
        let tasks_pos = WidgetPos::new(
            "tasks",
            &config.anchor,
            config.margin_x,
            config.margin_y,
            config.width,
            Some(config.stack_below.clone()),
            config.stack_gap,
            config.stack_offset_y,
        );
        tasks_pos.publish();
        // Domyślnie karta pamięci stoi w stacku pod kartą zadań — czyli tam,
        // gdzie była, gdy obie siedziały w jednym oknie. Wyczyszczenie
        // `memory_stack_below` w configu robi z niej całkiem wolny widget.
        let memory_pos = WidgetPos::new(
            "memory",
            &config.memory_anchor,
            config.memory_margin_x,
            config.memory_margin_y,
            config.width,
            Some(config.memory_stack_below.clone()),
            config.memory_stack_gap,
            config.memory_stack_offset_y,
        );
        memory_pos.publish();

        // Powierzchnie POWSTAJĄ PO `WidgetPos` i z ich marginesów, nie
        // z surowego configu — patrz `WidgetPos::new` (config bywa
        // nieświeży, prawdziwą pozycję zna opublikowana geometria).
        let surface_tasks = get_layer_surface(surface_settings(
            tasks_surface,
            "cosmic-widget-tasks",
            &config.anchor,
            tasks_pos.margin_x(),
            tasks_pos.margin_y(),
            config.width,
            120,
        ));
        let surface_memory = get_layer_surface(surface_settings(
            memory_surface,
            "cosmic-widget-tasks-memory",
            &config.memory_anchor,
            memory_pos.margin_x(),
            memory_pos.margin_y(),
            config.width,
            120,
        ));

        let glass = Glass::from_config(&config.glass);

        let widget = Widget {
            core,
            tasks_surface,
            memory_surface,
            config,
            tasks_pos,
            memory_pos,
            glass,
            tasks: None,
            last_error: None,
            memory_items: None,
            memory_error: None,
            input_open: false,
            input_value: String::new(),
            pending_done: HashSet::new(),
            pending_memory_done: HashSet::new(),
            visible: true,
        };

        let fetch = Task::batch([widget.fetch_task(), widget.fetch_memory()]);
        let blur = widget.blur_task();
        (widget, Task::batch([surface_tasks, surface_memory, blur, fetch]))
    }

    fn subscription(&self) -> Subscription<Message> {
        let secs = self.config.refresh_secs.max(30);
        let mut subs =
            vec![cosmic::iced::time::every(StdDuration::from_secs(secs)).map(|_| Message::Tick)];
        if self.tasks_pos.dragging() || self.memory_pos.dragging() {
            subs.push(drag::subscription().map(Message::Drag));
        }
        // Schowany widget nie ma powierzchni, więc nie ma czego przesuwać.
        if (self.tasks_pos.is_stacked() || self.memory_pos.is_stacked()) && self.visible {
            subs.push(
                Subscription::batch([
                    self.tasks_pos.stack_subscription(),
                    self.memory_pos.stack_subscription(),
                ])
                .map(|()| Message::StackTick),
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

    fn view(&self) -> Element<'_, Message> {
        text::body("").into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if id == self.tasks_surface {
            self.build_tasks_card()
        } else if id == self.memory_surface {
            self.build_memory_card()
        } else {
            text::body("").into()
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => Task::batch([self.fetch_task(), self.fetch_memory()]),
            Message::Scanned(Ok(tasks)) => {
                self.pending_done
                    .retain(|(p, l)| tasks.iter().any(|t| &t.file_path == p && &t.raw_line == l));
                self.tasks = Some(
                    tasks
                        .into_iter()
                        .filter(|t| {
                            !self.pending_done.contains(&(t.file_path.clone(), t.raw_line.clone()))
                        })
                        .collect(),
                );
                self.last_error = None;
                self.resize_task()
            }
            Message::Scanned(Err(e)) => {
                self.last_error = Some(e);
                self.resize_task()
            }
            Message::MemoryScanned(Ok(items)) => {
                self.pending_memory_done
                    .retain(|(p, l)| items.iter().any(|t| &t.file_path == p && &t.raw_line == l));
                self.memory_items = Some(
                    items
                        .into_iter()
                        .filter(|t| {
                            !self.pending_memory_done
                                .contains(&(t.file_path.clone(), t.raw_line.clone()))
                        })
                        .collect(),
                );
                self.memory_error = None;
                self.resize_task()
            }
            Message::MemoryScanned(Err(e)) => {
                self.memory_error = Some(e);
                self.resize_task()
            }
            Message::OpenTodayPage => {
                let uri = format!(
                    "obsidian://open?vault={}&file={}",
                    percent_encode(&self.config.vault_name),
                    percent_encode(TODAY_PAGE)
                );
                let _ = std::process::Command::new("xdg-open").arg(uri).spawn();
                Task::none()
            }
            Message::MarkDone(idx) => {
                let remote = self.remote();
                let mut done = None;
                if let Some(tasks) = &self.tasks {
                    if let Some(t) = tasks.get(idx) {
                        if let Err(e) = mark_task_done(
                            remote.as_ref(),
                            &self.config.vault_path,
                            &t.file_path,
                            t.line_idx,
                            &t.raw_line,
                        ) {
                            self.last_error = Some(format!("nie mogę odhaczyć zadania: {e}"));
                            return Task::none();
                        }
                        done = Some((t.file_path.clone(), t.raw_line.clone()));
                    }
                }
                if let Some(key) = done {
                    self.pending_done.insert(key);
                }
                self.fetch_task()
            }
            Message::MarkMemoryDone(idx) => {
                let remote = self.remote();
                let mut done = None;
                if let Some(items) = &self.memory_items {
                    if let Some(t) = items.get(idx) {
                        if let Err(e) = mark_task_done(
                            remote.as_ref(),
                            &self.config.vault_path,
                            &t.file_path,
                            t.line_idx,
                            &t.raw_line,
                        ) {
                            self.memory_error = Some(format!("nie mogę odhaczyć: {e}"));
                            return Task::none();
                        }
                        done = Some((t.file_path.clone(), t.raw_line.clone()));
                    }
                }
                if let Some(key) = done {
                    self.pending_memory_done.insert(key);
                }
                self.fetch_memory()
            }
            Message::ToggleInput => {
                self.input_open = !self.input_open;
                if !self.input_open {
                    self.input_value.clear();
                }
                Task::batch([self.keyboard_task(), self.resize_task()])
            }
            Message::InputChanged(v) => {
                self.input_value = v;
                Task::none()
            }
            Message::SubmitInput => {
                let text = self.input_value.trim().to_string();
                if text.is_empty() {
                    return Task::none();
                }
                if let Err(e) = append_memory_item(
                    self.remote().as_ref(),
                    &self.config.vault_path,
                    &self.config.daily_notes_folder,
                    &self.config.daily_note_template,
                    &text,
                ) {
                    self.memory_error = Some(format!("nie mogę zapisać: {e}"));
                    return Task::none();
                }
                self.input_value.clear();
                self.input_open = false;
                Task::batch([self.keyboard_task(), self.fetch_memory()])
            }
            Message::DragStart(card) => {
                self.pos_of_mut(card).start_drag();
                Task::none()
            }
            // Event myszy dostają obie karty; `WidgetPos::on_event` bierze
            // pod uwagę tylko własną powierzchnię i tylko gdy sama jest
            // przeciągana, więc ta druga i tak go zignoruje.
            Message::Drag(ev) => {
                let mut tasks_out = Vec::new();
                for card in [Card::Tasks, Card::Memory] {
                    let surface = self.surface_of(card);
                    match self.pos_of_mut(card).on_event(&ev, surface) {
                        Outcome::Moved => tasks_out.push(self.margin_task(card)),
                        Outcome::Released => {
                            self.save_position(card);
                            tasks_out.push(self.margin_task(card));
                        }
                        Outcome::None => {}
                    }
                }
                Task::batch(tasks_out)
            }
            Message::Glass(g) => {
                self.glass = g;
                self.blur_task()
            }
            Message::StackTick => {
                let mut out = Vec::new();
                for card in [Card::Tasks, Card::Memory] {
                    if self.pos_of_mut(card).stack_tick() {
                        self.pos_of(card).publish();
                        out.push(self.margin_task(card));
                    }
                }
                Task::batch(out)
            }
            // Zejście na inny pulpit = zniszcz powierzchnię (KWin nie umie
            // przypiąć layer-shella do pulpitu); powrót = odtwórz ją z
            // aktualną pozycją i wysokością.
            Message::DesktopVisible(show) => {
                if show == self.visible {
                    return Task::none();
                }
                self.visible = show;
                if show {
                    self.tasks_surface = window::Id::unique();
                    self.memory_surface = window::Id::unique();
                    Task::batch([
                        get_layer_surface(surface_settings(
                            self.tasks_surface,
                            "cosmic-widget-tasks",
                            &self.config.anchor,
                            self.tasks_pos.margin_x(),
                            self.tasks_pos.margin_y(),
                            self.config.width,
                            self.tasks_card_height(),
                        )),
                        get_layer_surface(surface_settings(
                            self.memory_surface,
                            "cosmic-widget-tasks-memory",
                            &self.config.memory_anchor,
                            self.memory_pos.margin_x(),
                            self.memory_pos.margin_y(),
                            self.config.width,
                            self.memory_card_height(),
                        )),
                        self.blur_task(),
                    ])
                } else {
                    Task::batch([
                        destroy_layer_surface(self.tasks_surface),
                        destroy_layer_surface(self.memory_surface),
                    ])
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

// =========================================================================
// UI
// =========================================================================

const DIM: Color = Color::from_rgba(0.95, 0.95, 0.97, 0.55);
const RED: Color = Color::from_rgb(0.95, 0.30, 0.30);
const ORANGE: Color = Color::from_rgb(0.95, 0.65, 0.20);
const YELLOW: Color = Color::from_rgb(0.85, 0.80, 0.25);
const GRAY: Color = Color::from_rgba(0.95, 0.95, 0.97, 0.35);

fn dim_text() -> cosmic::theme::Text {
    cosmic::theme::Text::Color(DIM)
}

fn overdue_color(t: &TaskItem) -> Color {
    if t.overdue_days > 0 {
        RED
    } else {
        ORANGE
    }
}

/// Kolor kropki wg priorytetu (sekcja pamięci) — im pilniejsze, tym
/// cieplejsze; poniżej "średni" celowo przygaszone.
fn priority_color(p: Priority) -> Color {
    match p {
        Priority::Highest => RED,
        Priority::High => ORANGE,
        Priority::Medium => YELLOW,
        Priority::None | Priority::Low | Priority::Lowest => GRAY,
    }
}

/// Wysokość kreski „oczekujące” razem z odstępem kolumny.
const WAITING_DIVIDER_H: u32 = 28;

/// Kreska oddzielająca zadania „czekam na klienta” (status `[5]`).
fn waiting_divider<'a>() -> Element<'a, Message> {
    let line = || -> Element<'a, Message> {
        container(text::body(""))
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .class(cosmic::theme::Container::custom(|_| {
                cosmic::iced::widget::container::Style {
                    background: Some(Background::Color(GRAY)),
                    ..Default::default()
                }
            }))
            .into()
    };
    cosmic::widget::row::with_children(vec![
        line(),
        text::caption("⏳ czeka na klienta").class(dim_text()).into(),
        line(),
    ])
    .spacing(8)
    .align_y(cosmic::iced::Alignment::Center)
    .into()
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

/// Przybliżona szerokość znaku tekstu `body` w px — do policzenia, na ile
/// wierszy rozjedzie się wpis. Zaniżona celowo: lepiej zostawić parę pikseli
/// zapasu na dole karty niż uciąć ostatni wiersz.
const AVG_CHAR_PX: u32 = 7;

/// Wysokość listy wpisów w px. Karty mają JAWNĄ wysokość (musi się zgadzać
/// z wysokością powierzchni layer-shell i regionu rozmycia), więc długi wpis
/// zawinięty na kilka wierszy trzeba doliczyć — inaczej reszta tekstu jest
/// po prostu ucięta poza kartą.
fn rows_height<'a>(texts: impl Iterator<Item = &'a str>, chars_per_line: usize) -> u32 {
    texts.map(|t| 30 + 19 * (wrapped_lines(t, chars_per_line) - 1)).sum()
}

/// Na ile wierszy zawinie się tekst przy `chars_per_line` znakach w wierszu.
/// Zawijanie po słowach — tak jak robi to silnik tekstu iced.
fn wrapped_lines(text: &str, chars_per_line: usize) -> u32 {
    let mut lines = 1;
    let mut used = 0;
    for word in text.split_whitespace() {
        let w = word.chars().count();
        if used == 0 {
            used = w;
        } else if used + 1 + w <= chars_per_line {
            used += 1 + w;
        } else {
            lines += 1;
            used = w;
        }
        // Słowo dłuższe niż wiersz (długi link) łamie się w środku.
        while used > chars_per_line {
            lines += 1;
            used -= chars_per_line;
        }
    }
    lines
}

impl Widget {
    fn build_tasks_card(&self) -> Element<'_, Message> {
        let width = self.config.width as f32;
        let mut rows: Vec<Element<'_, Message>> = Vec::new();

        // ---- Nagłówek ----
        // W nagłówku zostaje SAM uchwyt ⠿: tytuł karty użytkownikowi niepotrzebny
        // (wie, co to za widget), a przycisków „Dziś”/„+” nigdy nie używał —
        // zabierały tylko wysokość (2026-08-29). Obsługa `OpenTodayPage`
        // i pola szybkiego dodawania została w kodzie, gdyby miały wrócić.
        let header_items: Vec<Element<'_, Message>> =
            vec![drag::handle(Message::DragStart(Card::Tasks))];
        rows.push(
            cosmic::widget::row::with_children(header_items)
                .spacing(8)
                .align_y(cosmic::iced::Alignment::Center)
                .into(),
        );
        // ---- Błąd ----
        if let Some(err) = &self.last_error {
            rows.push(text::caption(format!("błąd: {err}")).class(dim_text()).into());
        }

        // ---- Treść ----
        match &self.tasks {
            None => {
                if self.last_error.is_none() {
                    rows.push(text::caption("Ładowanie…").class(dim_text()).into());
                }
            }
            Some(tasks) if tasks.is_empty() => {
                rows.push(text::caption("Brak zaległych zadań 🎉").class(dim_text()).into());
            }
            Some(tasks) => {
                for (idx, t) in tasks.iter().enumerate() {
                    // Pierwsze oczekujące → kreska i podpis (sort w
                    // `scan_tasks` trzyma je zawsze na końcu listy).
                    if t.is_waiting() && (idx == 0 || !tasks[idx - 1].is_waiting()) {
                        rows.push(waiting_divider());
                    }
                    let (dot_color, body) = if t.is_waiting() {
                        (GRAY, text::body(t.text.clone()).class(dim_text()))
                    } else {
                        (overdue_color(t), text::body(t.text.clone()))
                    };
                    let row = cosmic::widget::row::with_children(vec![
                        dot(dot_color),
                        text::caption(due_label(t)).width(Length::Fixed(80.0)).class(dim_text()).into(),
                        body.width(Length::Fill).into(),
                    ])
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center);

                    let clickable = mouse_area(row).on_double_press(Message::MarkDone(idx));
                    rows.push(clickable.into());
                }
            }
        }

        let content = cosmic::widget::column::with_children(rows).spacing(6);

        container(content)
            .padding([6, 14])
            .class(cosmic::theme::Container::custom(self.glass.container()))
            .width(Length::Fixed(width))
            // Wysokość JAWNA, taka sama jak wysokość powierzchni i regionu
            // rozmycia (`tasks_card_height`). Bez tego karta kurczy się do
            // treści, a szkło — liczone z wysokości powierzchni — wystaje
            // spod niej rozmytym paskiem.
            .height(Length::Fixed(self.tasks_card_height() as f32))
            .into()
    }

    fn build_memory_card(&self) -> Element<'_, Message> {
        let width = self.config.width as f32;
        let mut rows: Vec<Element<'_, Message>> = Vec::new();

        // ---- Nagłówek ----
        // Sam uchwyt ⠿ — patrz nagłówek karty zadań. Własny, bo karta pamięci
        // ma osobną powierzchnię i przesuwa się niezależnie od zadań.
        let header_items: Vec<Element<'_, Message>> =
            vec![drag::handle(Message::DragStart(Card::Memory))];
        rows.push(
            cosmic::widget::row::with_children(header_items)
                .spacing(8)
                .align_y(cosmic::iced::Alignment::Center)
                .into(),
        );
        // ---- Pole szybkiego dodawania ----
        if self.input_open {
            let input = text_input::text_input("Co zapamiętać?", &self.input_value)
                .on_input(Message::InputChanged)
                .on_submit(|_| Message::SubmitInput)
                .width(Length::Fill);
            let save_btn = button::text("Zapisz").on_press(Message::SubmitInput);
            rows.push(
                cosmic::widget::row::with_children(vec![input.into(), save_btn.into()])
                    .spacing(6)
                    .align_y(cosmic::iced::Alignment::Center)
                    .into(),
            );
        }

        // ---- Błąd ----
        if let Some(err) = &self.memory_error {
            rows.push(text::caption(format!("błąd: {err}")).class(dim_text()).into());
        }

        // ---- Treść ----
        match &self.memory_items {
            None => {
                if self.memory_error.is_none() {
                    rows.push(text::caption("Ładowanie…").class(dim_text()).into());
                }
            }
            Some(items) if items.is_empty() => {
                rows.push(text::caption("Nic do zapamiętania 🎉").class(dim_text()).into());
            }
            Some(items) => {
                for (idx, t) in items.iter().enumerate() {
                    let row = cosmic::widget::row::with_children(vec![
                        dot(priority_color(t.priority)),
                        text::caption(t.priority.glyph()).width(Length::Fixed(24.0)).class(dim_text()).into(),
                        text::body(t.text.clone()).width(Length::Fill).into(),
                    ])
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center);

                    let clickable = mouse_area(row).on_double_press(Message::MarkMemoryDone(idx));
                    rows.push(clickable.into());
                }
            }
        }

        let content = cosmic::widget::column::with_children(rows).spacing(6);

        container(content)
            .padding([6, 14])
            .class(cosmic::theme::Container::custom(self.glass.container()))
            .width(Length::Fixed(width))
            // Wysokość JAWNA, taka sama jak wysokość powierzchni i regionu
            // rozmycia (`memory_card_height`). Bez tego karta kurczy się do
            // treści, a szkło — liczone z wysokości powierzchni — wystaje
            // spod niej rozmytym paskiem.
            .height(Length::Fixed(self.memory_card_height() as f32))
            .into()
    }
}
