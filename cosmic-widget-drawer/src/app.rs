//! Dwie szuflady wysuwane od krawędzi ekranu.
//!
//! - **prawa** — treść widgetów pulpitowych (pogoda, harmonogram, zadania,
//!   kursy), bo na pulpicie są przykryte oknami;
//! - **lewa** — drzewo katalogów vaulta Obsidiana; klik w notatkę otwiera ją.
//!
//! **Dlaczego layer-shell, a nie aplet panelu.** Popup apletu otwiera się
//! wyłącznie kliknięciem w ikonkę na panelu, jest do niej przyklejony i nie
//! da się go wywołać globalnym skrótem ani najechaniem na krawędź ekranu —
//! a użytkownik chciał dokładnie tego (skrót + gest, zamykanie Esc / kliknięciem
//! obok). Powierzchnia `zwlr_layer_shell_v1` na warstwie `Top` daje to
//! wszystko: wchodzi NAD okna, bierze klawiaturę na wyłączność (Esc),
//! a kliknięcie poza kartą widzimy sami, bo powierzchnia obejmuje cały ekran.
//!
//! Proces chodzi cały czas (autostart) i trzyma pobrane dane, żeby szuflada
//! wyjeżdżała od razu wypełniona. Powierzchnie powstają dopiero przy
//! otwarciu i giną po zamknięciu — zamknięte szuflady nie zajmują ani
//! piksela ekranu poza gorącym paskiem prawej krawędzi.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::{Datelike, Duration as ChronoDuration, Local, NaiveDate, Timelike};
use cosmic::app::{Core, Task};
use cosmic::iced::platform_specific::shell::commands::layer_surface::get_layer_surface;
use cosmic::iced::{Background, Color, Length, Padding, Subscription, window};
use cosmic::widget::{column, container, flex_row, mouse_area, row, scrollable, text, text_input};
use cosmic::{Element, Theme};
use cosmic_widget_common::{Glass, glass};

use crate::calendar::{self, Calendar};
use crate::config::Config;
use crate::files::{self, FileTree};
use crate::ipc;
use crate::panel::{self, Panel, Side};
use crate::sources::{
    self, MarketRow, agenda_config, ics, markets_config, tasks, tasks_config, weather,
    weather_config,
};

static WEEKDAYS: [&str; 7] = ["Pon", "Wt", "Śr", "Czw", "Pt", "Sob", "Ndz"];

const DIM: Color = Color::from_rgba(0.95, 0.95, 0.97, 0.55);
const ACCENT: Color = Color::from_rgb(0.55, 0.75, 0.98);
const UP: Color = Color::from_rgb(0.45, 0.82, 0.55);
const DOWN: Color = Color::from_rgb(0.92, 0.48, 0.48);

fn dim() -> cosmic::theme::Text {
    cosmic::theme::Text::Color(DIM)
}
fn accent() -> cosmic::theme::Text {
    cosmic::theme::Text::Color(ACCENT)
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Polecenie z gniazda (skrót klawiszowy albo gest).
    Ipc(ipc::Command),
    /// Zamknij tę szufladę, która jest otwarta (Esc, klik obok).
    Close,
    /// Klik w gorący pasek prawej krawędzi.
    Toggle,
    /// Klatka animacji wysuwania/chowania.
    Anim,
    EdgeEnter,
    EdgeExit,
    EdgeDwell(u64),
    /// Kompozytor podał realny rozmiar powierzchni.
    Resized(window::Id, f32, f32),
    Refresh,
    Clock,
    Agenda(Result<Vec<ics::AgendaEvent>, String>),
    Tasks(Result<Vec<tasks::TaskItem>, String>),
    Weather(Result<weather::WeatherData, String>),
    Markets(Result<Vec<MarketRow>, String>),
    Glass(Glass),
    // ---- drzewo vaulta: klik, przeciąganie, menu akcji ----
    /// Wciśnięcie lewego na wierszu — początek kliknięcia albo przeciągania.
    RowPress(PathBuf),
    /// Puszczenie lewego na wierszu: na tym samym = klik (otwórz/rozwiń),
    /// na katalogu innym niż wciśnięty = przenieś tam wciśnięty wpis.
    RowRelease(PathBuf),
    RowEnter(PathBuf),
    RowLeave(PathBuf),
    /// Prawy przycisk — pokaż/schowaj pasek akcji pod wierszem.
    RowMenu(PathBuf),
    /// Puszczenie na nagłówku — przeciągnięty wpis ląduje w korzeniu.
    DropRoot,
    /// Puszczenie gdziekolwiek indziej na karcie — koniec przeciągania.
    DragEnd,
    NewNote(PathBuf),
    NewFolder(PathBuf),
    Rename(PathBuf),
    EditInput(String),
    EditSubmit,
    /// Tryb „przenieś…" — wskazanie celu przyciskiem przy katalogu.
    MoveStart(PathBuf),
    MoveHere(PathBuf),
    Trash(PathBuf),
    OpenInObsidian(PathBuf),
    Undo,
    /// Porzuć edycję / menu / tryb przenoszenia (Esc, „anuluj").
    FsCancel,
    /// Klik w nagłówek drzewa — przeczytaj vault od nowa.
    FilesReload,
    /// Kalendarzyk pod drzewem: poprzedni/następny miesiąc, powrót do dziś.
    CalShift(i32),
    CalToday,
    /// Klik w dzień — otwórz notatkę dnia (tworząc ją, gdy jeszcze nie ma).
    CalOpen(NaiveDate),
    /// Kliknięcie w kartę — pochłania klik, żeby nie zamknął szuflady.
    Noop,
}

pub struct Drawer {
    core: Core,
    config: Config,
    glass: Glass,

    right: Panel,
    left: Panel,
    edge_id: Option<window::Id>,
    hover_serial: u64,

    // ---- dane prawej szuflady ----
    agenda: Option<Result<Vec<ics::AgendaEvent>, String>>,
    tasks: Option<Result<Vec<tasks::TaskItem>, String>>,
    weather: Option<Result<weather::WeatherData, String>>,
    markets: Option<Result<Vec<MarketRow>, String>>,
    fetched_at: Option<Instant>,
    now: chrono::DateTime<Local>,

    // ---- lewa szuflada ----
    tree: Option<FileTree>,
    cal: Option<Calendar>,
    vault_root: PathBuf,
    vault_name: String,
    daily_folder: String,
    daily_template: String,
    fs: FsUi,
}

/// Co się właśnie dzieje w drzewie poza zwykłym przeglądaniem.
#[derive(Default)]
struct FsUi {
    /// Wiersz z rozwiniętym paskiem akcji (PPM).
    menu: Option<PathBuf>,
    edit: Option<Edit>,
    /// Wpis czekający na wskazanie katalogu docelowego.
    moving: Option<PathBuf>,
    /// Wiersz, na którym wciśnięto lewy — źródło ewentualnego przeciągania.
    pressed: Option<PathBuf>,
    hovered: Option<PathBuf>,
    /// „Do kosza" wymaga drugiego kliknięcia.
    confirm_trash: Option<PathBuf>,
    /// Ostatni komunikat: (tekst, czy błąd).
    status: Option<(String, bool)>,
    /// Ostatnia operacja do cofnięcia: (gdzie jest teraz, gdzie była).
    undo: Option<(PathBuf, PathBuf)>,
}

impl FsUi {
    /// Czy trwa przeciąganie: wciśnięto jeden wiersz, kursor jest nad innym.
    fn dragging(&self) -> bool {
        matches!((&self.pressed, &self.hovered), (Some(p), Some(h)) if p != h)
    }
}

struct Edit {
    kind: EditKind,
    value: String,
}

#[derive(Clone, PartialEq)]
enum EditKind {
    Rename(PathBuf),
    NewNote(PathBuf),
    NewFolder(PathBuf),
}

const EDIT_ID: &str = "drawer-fs-edit";

impl Drawer {
    // ---- otwieranie i zamykanie ----

    /// Otwarcie jednej szuflady zamyka drugą — dwie naraz to dwie
    /// pełnoekranowe powierzchnie z klawiaturą na wyłączność, czyli pewny
    /// bałagan z fokusem.
    fn open_right(&mut self) -> Task<Message> {
        let close_other = self.left.close();
        Task::batch([close_other, self.right.open(), self.refresh_if_stale()])
    }

    /// Przygotowanie treści lewej szuflady (drzewo + kalendarzyk) bez
    /// otwierania powierzchni — wspólne dla „tylko lewa” i „obie naraz”.
    /// `false` = lewa szuflada jest wyłączona w configu.
    fn prepare_left(&mut self) -> bool {
        if !self.config.files.enabled {
            return false;
        }
        // Każde otwarcie zaczyna od czystego drzewa — bez pozostałości
        // po edycji czy przeciąganiu sprzed zamknięcia.
        self.fs = FsUi::default();
        // Drzewo budujemy przy pierwszym otwarciu, potem odświeżamy —
        // vault zmienia się między jednym zerknięciem a drugim.
        match self.tree.as_mut() {
            None => {
                self.tree = Some(FileTree::new(
                    self.vault_root.clone(),
                    self.config.files.show_all_files,
                ))
            }
            Some(tree) => tree.reload(),
        }
        let today = Local::now().date_naive();
        match self.cal.as_mut() {
            None => {
                self.cal = Some(Calendar::new(
                    self.vault_root.clone(),
                    self.daily_folder.clone(),
                    self.daily_template.clone(),
                    today,
                ))
            }
            // Wracamy na bieżący miesiąc i sprawdzamy, które dni mają już
            // notatkę — między jednym zerknięciem a drugim mogło przybyć.
            Some(cal) => {
                cal.go_to(today);
            }
        }
        true
    }

    fn open_left(&mut self) -> Task<Message> {
        if !self.prepare_left() {
            return Task::none();
        }
        let close_other = self.right.close();
        Task::batch([close_other, self.left.open()])
    }

    /// Obie szuflady jednocześnie — jedyne miejsce, gdzie druga NIE jest
    /// zamykana (użytkownik woli jeden skrót na oba panele niż dwa osobne).
    fn open_both(&mut self) -> Task<Message> {
        let mut jobs = vec![self.right.open(), self.refresh_if_stale()];
        if self.prepare_left() {
            jobs.push(self.left.open());
        }
        Task::batch(jobs)
    }

    fn close_all(&mut self) -> Task<Message> {
        Task::batch([self.right.close(), self.left.close()])
    }

    /// Gest gładzikiem. Ta sama strona ruchu znaczy co innego zależnie od
    /// tego, co jest otwarte: ruch w lewo wysuwa prawą szufladę (ciągnę ją
    /// z prawej krawędzi ku sobie) albo chowa lewą (odpycham ją do krawędzi).
    fn swipe(&mut self, to_left: bool) -> Task<Message> {
        if to_left {
            if self.left.is_showing() {
                self.left.close()
            } else {
                self.open_right()
            }
        } else if self.right.is_showing() {
            self.right.close()
        } else {
            self.open_left()
        }
    }

    // ---- zarządzanie plikami w drzewie ----

    /// Zwykły klik w wiersz: katalog rozwiń/zwiń, plik otwórz.
    fn activate(&mut self, path: PathBuf) -> Task<Message> {
        if path.is_dir() {
            if let Some(tree) = self.tree.as_mut() {
                tree.toggle(&path);
            }
            return Task::none();
        }
        files::open_entry(
            &path,
            &self.vault_root,
            &self.vault_name,
            &self.config.files.open_with,
        );
        if self.config.files.close_after_open {
            self.left.close()
        } else {
            Task::none()
        }
    }

    fn fs_busy(&self) -> bool {
        self.fs.edit.is_some() || self.fs.menu.is_some() || self.fs.moving.is_some()
    }

    fn fs_cancel(&mut self) {
        self.fs.edit = None;
        self.fs.menu = None;
        self.fs.moving = None;
        self.fs.confirm_trash = None;
        self.fs.pressed = None;
    }

    /// Po udanej operacji: komunikat + przeładowanie drzewa.
    fn fs_done(&mut self, msg: String) {
        self.fs.status = Some((msg, false));
        if let Some(tree) = self.tree.as_mut() {
            tree.reload();
        }
    }

    fn fs_error(&mut self, e: String) {
        self.fs.status = Some((e, true));
    }

    fn fs_move(&mut self, src: &std::path::Path, dir: &std::path::Path) {
        self.fs.menu = None;
        if src.parent() == Some(dir) {
            return;
        }
        match files::move_into(src, dir) {
            Ok(to) => {
                if let Some(tree) = self.tree.as_mut() {
                    tree.relocated(src, &to);
                    tree.expand(dir);
                }
                let dest = if dir == self.vault_root {
                    "korzeń vaulta".to_string()
                } else {
                    files::edit_label(dir)
                };
                self.fs_done(format!("„{}\" → {dest}", files::edit_label(&to)));
                self.fs.undo = Some((to, src.to_path_buf()));
            }
            Err(e) => self.fs_error(e),
        }
    }

    fn start_edit(&mut self, kind: EditKind, value: String) -> Task<Message> {
        self.fs.menu = None;
        self.fs.moving = None;
        // Nowy wpis pojawia się w rozwiniętym katalogu docelowym.
        if let EditKind::NewNote(dir) | EditKind::NewFolder(dir) = &kind {
            if let Some(tree) = self.tree.as_mut() {
                tree.expand(dir);
            }
        }
        self.fs.edit = Some(Edit { kind, value });
        let id = cosmic::widget::Id::new(EDIT_ID);
        Task::batch([text_input::focus(id.clone()), text_input::select_all(id)])
    }

    fn submit_edit(&mut self) -> Task<Message> {
        let Some(edit) = self.fs.edit.take() else {
            return Task::none();
        };
        let result = match &edit.kind {
            EditKind::NewNote(dir) => files::create_note(dir, &edit.value).map(|p| {
                self.fs_done(format!("utworzono „{}\"", files::edit_label(&p)));
                Some(p)
            }),
            EditKind::NewFolder(dir) => files::create_folder(dir, &edit.value).map(|p| {
                self.fs_done(format!("utworzono folder „{}\"", files::edit_label(&p)));
                None
            }),
            EditKind::Rename(path) => files::rename(path, &edit.value).map(|to| {
                if let Some(tree) = self.tree.as_mut() {
                    tree.relocated(path, &to);
                }
                if &to != path {
                    self.fs_done(format!("nazwa: „{}\"", files::edit_label(&to)));
                    self.fs.undo = Some((to, path.clone()));
                }
                None
            }),
        };
        match result {
            // Świeża notatka od razu otwiera się do pisania.
            Ok(Some(note)) => self.activate(note),
            Ok(None) => Task::none(),
            Err(e) => {
                self.fs_error(e);
                // Zostaw pole otwarte z tym, co wpisane — do poprawki.
                self.fs.edit = Some(edit);
                text_input::focus(cosmic::widget::Id::new(EDIT_ID))
            }
        }
    }

    fn panel_mut(&mut self, id: window::Id) -> Option<&mut Panel> {
        if self.right.id == Some(id) {
            Some(&mut self.right)
        } else if self.left.id == Some(id) {
            Some(&mut self.left)
        } else {
            None
        }
    }

    // ---- dane ----

    fn refresh_if_stale(&mut self) -> Task<Message> {
        let stale = match self.fetched_at {
            None => true,
            Some(t) => t.elapsed() >= Duration::from_secs(self.config.stale_secs.max(30)),
        };
        if stale { self.refresh() } else { Task::none() }
    }

    /// Pobierz wszystko, co włączone w `sections`. Configi źródeł czytamy za
    /// każdym razem od nowa — użytkownik edytuje je z poziomu widgetów (np. dokłada
    /// aktywo do kursów), a szuflada ma to widzieć bez restartu.
    fn refresh(&mut self) -> Task<Message> {
        self.fetched_at = Some(Instant::now());
        let mut jobs: Vec<Task<Message>> = Vec::new();

        if self.config.has_section("agenda") {
            let cfg = agenda_config::Config::load();
            if !cfg.ics_url.is_empty() {
                jobs.push(cosmic::task::future(async move {
                    Message::Agenda(ics::fetch_agenda(cfg.ics_url, cfg.days_ahead).await)
                }));
            }
        }
        if self.config.has_section("weather") {
            let cfg = weather_config::Config::load();
            jobs.push(cosmic::task::future(async move {
                Message::Weather(
                    weather::fetch_weather(cfg.lat, cfg.lon, cfg.imgw_station, cfg.forecast_days)
                        .await,
                )
            }));
        }
        if self.config.has_section("tasks") {
            let cfg = tasks_config::Config::load();
            jobs.push(cosmic::task::future(async move {
                Message::Tasks(sources::fetch_tasks(cfg.vault_path, cfg.max_tasks).await)
            }));
        }
        if self.config.has_section("markets") {
            let cfg = markets_config::Config::load();
            let limit = self.config.max_assets;
            jobs.push(cosmic::task::future(async move {
                Message::Markets(sources::fetch_markets(cfg.assets, limit).await)
            }));
        }
        Task::batch(jobs)
    }
}

impl cosmic::Application for Drawer {
    type Executor = cosmic::executor::Default;
    type Flags = Config;
    type Message = Message;
    const APP_ID: &'static str = "io.github.longer54.CosmicWidgetDrawer";

    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(mut core: Core, config: Config) -> (Self, Task<Message>) {
        core.window.show_headerbar = false;
        // Region rozmycia podajemy sami (patrz `Panel::blur`) — automatyczny
        // wziąłby CAŁY prostokąt powierzchni, czyli cały ekran.
        core.set_auto_blur(Default::default());

        let glass = Glass::from_config(&config.glass);
        // Vault bierzemy z configu widgetu zadań — jedna prawda o tej samej
        // ścieżce, zamiast drugiego wpisu do pilnowania.
        let vault_cfg = tasks_config::Config::load();

        let right = Panel::new(Side::Right, config.width, config.margin, config.animation_ms);
        let left = Panel::new(
            Side::Left,
            config.files.width,
            config.margin,
            config.animation_ms,
        );

        let mut drawer = Drawer {
            core,
            config,
            glass,
            right,
            left,
            edge_id: None,
            hover_serial: 0,
            agenda: None,
            tasks: None,
            weather: None,
            markets: None,
            fetched_at: None,
            now: Local::now(),
            tree: None,
            cal: None,
            vault_root: vault_cfg.vault_path,
            vault_name: vault_cfg.vault_name,
            daily_folder: vault_cfg.daily_notes_folder,
            daily_template: vault_cfg.daily_note_template,
            fs: FsUi::default(),
        };

        let mut startup = vec![drawer.refresh()];
        if drawer.config.edge_trigger {
            let id = window::Id::unique();
            drawer.edge_id = Some(id);
            let (_, screen_h) = cosmic_widget_common::screen::size().unwrap_or((1920, 1080));
            let h = (screen_h * drawer.config.edge_height_pct.clamp(5, 100) / 100).max(40);
            startup.push(get_layer_surface(panel::edge_settings(
                id,
                drawer.config.edge_width.max(1),
                h,
            )));
        }
        (drawer, Task::batch(startup))
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![ipc::subscription().map(Message::Ipc)];
        subs.push(glass::subscription(self.glass.level()).map(Message::Glass));

        if self.right.animating() || self.left.animating() {
            subs.push(cosmic::iced::time::every(Duration::from_millis(16)).map(|_| Message::Anim));
        }
        if self.right.is_open() {
            subs.push(
                cosmic::iced::time::every(Duration::from_secs(self.config.refresh_secs.max(30)))
                    .map(|_| Message::Refresh),
            );
            subs.push(cosmic::iced::time::every(Duration::from_secs(20)).map(|_| Message::Clock));
        }
        // Wyłącznie Esc i zmiana rozmiaru powierzchni. NIE nasłuchujemy tu
        // ruchów myszy — każdy drgnięty piksel budziłby `update()`.
        subs.push(cosmic::iced::event::listen_with(|event, _status, id| {
            use cosmic::iced::keyboard::{Event as Kb, Key, key::Named};
            match event {
                cosmic::iced::Event::Keyboard(Kb::KeyPressed {
                    key: Key::Named(Named::Escape),
                    ..
                }) => Some(Message::Close),
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
        if Some(id) == self.edge_id {
            return self.edge_view();
        }
        if self.right.id == Some(id) {
            return self.panel_view(&self.right, self.right_content());
        }
        if self.left.id == Some(id) {
            return self.panel_view(&self.left, self.left_content());
        }
        text::body("").into()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Ipc(cmd) => match cmd {
                ipc::Command::Show => self.open_right(),
                ipc::Command::Hide => self.right.close(),
                ipc::Command::Toggle => {
                    if self.right.is_showing() {
                        self.right.close()
                    } else {
                        self.open_right()
                    }
                }
                ipc::Command::ShowFiles => self.open_left(),
                ipc::Command::HideFiles => self.left.close(),
                ipc::Command::ToggleFiles => {
                    if self.left.is_showing() {
                        self.left.close()
                    } else {
                        self.open_left()
                    }
                }
                ipc::Command::ToggleBoth => {
                    if self.right.is_showing() || self.left.is_showing() {
                        self.close_all()
                    } else {
                        self.open_both()
                    }
                }
                ipc::Command::SwipeLeft => self.swipe(true),
                ipc::Command::SwipeRight => self.swipe(false),
            },
            // Esc najpierw porzuca to, co w toku w drzewie, dopiero drugi
            // zamyka szufladę.
            Message::Close => {
                if self.left.is_showing() && self.fs_busy() {
                    self.fs_cancel();
                    Task::none()
                } else {
                    self.close_all()
                }
            }
            Message::Toggle => {
                if self.right.is_showing() {
                    self.right.close()
                } else {
                    self.open_right()
                }
            }
            Message::Anim => {
                let mut jobs = Vec::new();
                if self.right.animating() && self.right.anim_done() {
                    jobs.push(self.right.finish_anim());
                    jobs.push(self.right.blur(self.glass));
                }
                if self.left.animating() && self.left.anim_done() {
                    jobs.push(self.left.finish_anim());
                    jobs.push(self.left.blur(self.glass));
                }
                Task::batch(jobs)
            }
            Message::EdgeEnter => {
                self.hover_serial = self.hover_serial.wrapping_add(1);
                let serial = self.hover_serial;
                let wait = Duration::from_millis(self.config.edge_dwell_ms);
                cosmic::task::future(async move {
                    tokio::time::sleep(wait).await;
                    Message::EdgeDwell(serial)
                })
            }
            Message::EdgeExit => {
                // Unieważnia „dojrzewanie” w locie: kursor tylko przemknął.
                self.hover_serial = self.hover_serial.wrapping_add(1);
                Task::none()
            }
            Message::EdgeDwell(serial) => {
                let swiezo_zamknieta = self.right.reopen_blocked();
                if serial == self.hover_serial && !self.right.is_open() && !swiezo_zamknieta {
                    self.open_right()
                } else {
                    Task::none()
                }
            }
            Message::Resized(id, w, h) => {
                let level = self.glass;
                let Some(p) = self.panel_mut(id) else {
                    return Task::none();
                };
                if p.surface_size == Some((w, h)) {
                    return Task::none();
                }
                p.surface_size = Some((w, h));
                p.blur(level)
            }
            Message::Refresh => self.refresh(),
            Message::Clock => {
                self.now = Local::now();
                Task::none()
            }
            Message::Agenda(r) => {
                self.agenda = Some(r);
                Task::none()
            }
            Message::Tasks(r) => {
                self.tasks = Some(r);
                Task::none()
            }
            Message::Weather(r) => {
                self.weather = Some(r);
                Task::none()
            }
            Message::Markets(r) => {
                self.markets = Some(r);
                Task::none()
            }
            Message::Glass(g) => {
                self.glass = g;
                Task::batch([self.right.blur(g), self.left.blur(g)])
            }
            Message::RowPress(path) => {
                self.fs.pressed = Some(path);
                Task::none()
            }
            Message::RowRelease(target) => {
                let Some(src) = self.fs.pressed.take() else {
                    return Task::none();
                };
                if src == target {
                    return self.activate(target);
                }
                if target.is_dir() {
                    self.fs_move(&src, &target);
                }
                Task::none()
            }
            Message::RowEnter(path) => {
                self.fs.hovered = Some(path);
                Task::none()
            }
            Message::RowLeave(path) => {
                if self.fs.hovered.as_ref() == Some(&path) {
                    self.fs.hovered = None;
                }
                Task::none()
            }
            Message::RowMenu(path) => {
                self.fs.confirm_trash = None;
                self.fs.menu = if self.fs.menu.as_ref() == Some(&path) {
                    None
                } else {
                    Some(path)
                };
                Task::none()
            }
            Message::DropRoot => {
                if let Some(src) = self.fs.pressed.take() {
                    let root = self.vault_root.clone();
                    self.fs_move(&src, &root);
                }
                Task::none()
            }
            Message::DragEnd => {
                self.fs.pressed = None;
                Task::none()
            }
            Message::NewNote(dir) => self.start_edit(EditKind::NewNote(dir), String::new()),
            Message::NewFolder(dir) => self.start_edit(EditKind::NewFolder(dir), String::new()),
            Message::Rename(path) => {
                let label = files::edit_label(&path);
                self.start_edit(EditKind::Rename(path), label)
            }
            Message::EditInput(v) => {
                if let Some(e) = self.fs.edit.as_mut() {
                    e.value = v;
                }
                Task::none()
            }
            Message::EditSubmit => self.submit_edit(),
            Message::MoveStart(path) => {
                self.fs.menu = None;
                self.fs.moving = Some(path);
                Task::none()
            }
            Message::MoveHere(dir) => {
                if let Some(src) = self.fs.moving.take() {
                    self.fs_move(&src, &dir);
                }
                Task::none()
            }
            Message::Trash(path) => {
                if self.fs.confirm_trash.as_ref() != Some(&path) {
                    self.fs.confirm_trash = Some(path);
                    return Task::none();
                }
                self.fs.confirm_trash = None;
                self.fs.menu = None;
                match files::trash(&path, &self.vault_root) {
                    Ok(to) => {
                        self.fs_done(format!("„{}\" w koszu (.trash)", files::edit_label(&path)));
                        self.fs.undo = Some((to, path));
                    }
                    Err(e) => self.fs_error(e),
                }
                Task::none()
            }
            Message::OpenInObsidian(path) => {
                self.fs.menu = None;
                files::open_entry(&path, &self.vault_root, &self.vault_name, "obsidian");
                if self.config.files.close_after_open {
                    self.left.close()
                } else {
                    Task::none()
                }
            }
            Message::Undo => {
                if let Some((now, was)) = self.fs.undo.take() {
                    match files::restore(&now, &was) {
                        Ok(()) => {
                            if let Some(tree) = self.tree.as_mut() {
                                tree.relocated(&now, &was);
                            }
                            self.fs_done("cofnięte".into());
                        }
                        Err(e) => self.fs_error(e),
                    }
                }
                Task::none()
            }
            Message::FsCancel => {
                self.fs_cancel();
                Task::none()
            }
            Message::FilesReload => {
                if let Some(tree) = self.tree.as_mut() {
                    tree.reload();
                }
                if let Some(cal) = self.cal.as_mut() {
                    cal.rescan();
                }
                Task::none()
            }
            Message::CalShift(months) => {
                if let Some(cal) = self.cal.as_mut() {
                    cal.shift(months);
                }
                Task::none()
            }
            Message::CalToday => {
                if let Some(cal) = self.cal.as_mut() {
                    cal.go_to(Local::now().date_naive());
                }
                Task::none()
            }
            Message::CalOpen(day) => {
                let Some(cal) = self.cal.as_mut() else {
                    return Task::none();
                };
                match cal.ensure_note(day) {
                    Ok(path) => {
                        files::open_entry(
                            &path,
                            &self.vault_root,
                            &self.vault_name,
                            &self.config.files.open_with,
                        );
                        if self.config.files.close_after_open {
                            return self.left.close();
                        }
                    }
                    Err(e) => eprintln!(
                        "cosmic-widget-drawer: nie mogę utworzyć notatki dnia {day}: {e}"
                    ),
                }
                Task::none()
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
// Widok — wspólna oprawa obu szuflad
// =========================================================================

impl Drawer {
    /// Gorący pasek: przezroczysty, sam nic nie rysuje. Kliknięcie w niego
    /// też otwiera szufladę — dla tych kilku pikseli to jedyne sensowne
    /// działanie, a jest po co kliknąć, gdy ktoś wyłączy najeżdżanie.
    fn edge_view(&self) -> Element<'_, Message> {
        mouse_area(
            container(text::body(""))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_enter(Message::EdgeEnter)
        .on_exit(Message::EdgeExit)
        .on_press(Message::Toggle)
        .into()
    }

    /// Karta przy swojej krawędzi plus przyciemnione tło łapiące kliknięcie
    /// „obok”. Wysuwanie NIE rusza marginesem powierzchni (to budziłoby
    /// kompozytor co klatkę) — karta jest odsuwana od swojej krawędzi
    /// paddingiem, a z drugiej strony dopycha ją pusty pas.
    fn panel_view<'a>(&'a self, p: &Panel, content: Element<'a, Message>) -> Element<'a, Message> {
        let m = self.config.margin as f32;
        let offset = p.offset();

        let card = container(content)
            .padding([12, 14])
            .class(cosmic::theme::Container::custom(self.glass.container()))
            .width(Length::Fixed(p.width as f32))
            .height(Length::Fill);

        // Karta pochłania kliknięcia (mouse_area łapie zdarzenie, więc nie
        // dochodzi do `Close` z tła) — bez tego klik w treść zamykałby panel.
        let card = mouse_area(card).on_press(Message::Noop);

        let (pad, filler_first) = match p.side {
            Side::Right => (
                Padding {
                    top: m,
                    right: m,
                    bottom: m,
                    left: 0.0,
                },
                true,
            ),
            Side::Left => (
                Padding {
                    top: m,
                    right: 0.0,
                    bottom: m,
                    // Wysuwanie w lewo idzie UJEMNYM paddingiem: `Length`
                    // obcina ujemne szerokości do zera, więc pas dopychający
                    // (tak jak po prawej) nic by tu nie dał.
                    left: m - offset,
                },
                false,
            ),
        };
        let panel_box = container(card).padding(pad);

        let filler = || {
            container(text::body(""))
                .width(Length::Fill)
                .height(Length::Fill)
        };
        let pusher = |w: f32| {
            container(text::body(""))
                .width(Length::Fixed(w))
                .height(Length::Fill)
        };

        let mut line = row::with_children(Vec::new());
        if filler_first {
            line = line.push(filler());
            line = line.push(panel_box);
            if offset > 0.5 {
                line = line.push(pusher(offset));
            }
        } else {
            // Lewa chowa się ujemnym paddingiem (wyżej), nie pasem.
            line = line.push(panel_box);
            line = line.push(filler());
        }

        // Tło przyciemniające — gaśnie razem z wysuwaniem karty.
        let alpha = self.config.scrim_opacity.clamp(0.0, 1.0) * (1.0 - p.slide());
        let scrim = container(line)
            .width(Length::Fill)
            .height(Length::Fill)
            .class(cosmic::theme::Container::custom(move |_t: &Theme| {
                cosmic::iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, alpha))),
                    ..Default::default()
                }
            }));

        mouse_area(scrim).on_press(Message::Close).into()
    }
}

// =========================================================================
// Widok — prawa szuflada (treść widgetów)
// =========================================================================

impl Drawer {
    fn right_content(&self) -> Element<'_, Message> {
        let mut body: Vec<Element<'_, Message>> = Vec::new();
        for name in &self.config.sections {
            let section = match name.as_str() {
                "weather" => self.weather_section(),
                "agenda" => self.agenda_section(),
                "tasks" => self.tasks_section(),
                "markets" => self.markets_section(),
                _ => continue,
            };
            if !body.is_empty() {
                body.push(cosmic::widget::divider::horizontal::default().into());
            }
            body.push(section);
        }

        let list = scrollable(column::with_children(body).spacing(10))
            .scrollbar_width(0.0)
            .scroller_width(0.0)
            .scrollbar_padding(0.0)
            .width(Length::Fill)
            .height(Length::Fill);

        column::with_children(vec![
            self.header(),
            container(list).height(Length::Fill).into(),
        ])
        .spacing(10)
        .into()
    }

    /// Nagłówek: godzina i data. Kliknięcie odświeża dane od ręki.
    fn header(&self) -> Element<'_, Message> {
        let clock = format!("{:02}:{:02}", self.now.hour(), self.now.minute());
        let date = format!(
            "{}, {:02}.{:02}.{}",
            WEEKDAYS[self.now.weekday().num_days_from_monday() as usize],
            self.now.day(),
            self.now.month(),
            self.now.year()
        );
        let stamp = match self.fetched_at {
            Some(t) => format!("odświeżono {} s temu", t.elapsed().as_secs()),
            None => "—".into(),
        };
        mouse_area(
            container(
                column::with_children(vec![
                    text::title3(clock).into(),
                    text::caption(date).class(dim()).into(),
                    text::caption(stamp).class(dim()).into(),
                ])
                .spacing(2),
            )
            .width(Length::Fill),
        )
        .on_press(Message::Refresh)
        .into()
    }

    fn section_title(title: &str) -> Element<'_, Message> {
        text::caption_heading(title.to_string())
            .class(accent())
            .into()
    }

    fn weather_section(&self) -> Element<'_, Message> {
        let mut rows: Vec<Element<'_, Message>> = vec![Self::section_title("Pogoda")];
        match &self.weather {
            None => rows.push(text::caption("Ładowanie…").class(dim()).into()),
            Some(Err(e)) => rows.push(text::caption(format!("błąd: {e}")).class(dim()).into()),
            Some(Ok(data)) => {
                if let Some(c) = &data.current {
                    rows.push(
                        row::with_children(vec![
                            text::title2(format!("{:.0}°", c.temperature)).into(),
                            column::with_children(vec![
                                text::caption(format!(
                                    "wiatr {:.0} km/h {}",
                                    c.wind_speed,
                                    weather::wind_compass(c.wind_dir_deg)
                                ))
                                .class(dim())
                                .into(),
                                text::caption(format!(
                                    "wilgotność {:.0}% · {:.0} hPa",
                                    c.humidity, c.pressure
                                ))
                                .class(dim())
                                .into(),
                            ])
                            .spacing(2)
                            .into(),
                        ])
                        .spacing(12)
                        .align_y(cosmic::iced::Alignment::Center)
                        .into(),
                    );
                }
                for d in data.forecast.iter().take(4) {
                    rows.push(
                        row::with_children(vec![
                            text::caption(day_label(d.date))
                                .width(Length::Fixed(74.0))
                                .class(dim())
                                .into(),
                            text::caption(weather::wmo_desc(d.code).to_string())
                                .width(Length::Fill)
                                .into(),
                            text::caption(format!("{:.0}°/{:.0}°", d.tmax, d.tmin)).into(),
                        ])
                        .spacing(8)
                        .into(),
                    );
                }
                if let Some(w) = &data.warning {
                    rows.push(text::caption(w.clone()).class(dim()).into());
                }
            }
        }
        column::with_children(rows).spacing(6).into()
    }

    fn agenda_section(&self) -> Element<'_, Message> {
        let mut rows: Vec<Element<'_, Message>> = vec![Self::section_title("Harmonogram")];
        match &self.agenda {
            None => rows.push(text::caption("Ładowanie…").class(dim()).into()),
            Some(Err(e)) => rows.push(text::caption(format!("błąd: {e}")).class(dim()).into()),
            Some(Ok(events)) if events.is_empty() => {
                rows.push(text::caption("Brak wydarzeń").class(dim()).into())
            }
            Some(Ok(events)) => {
                let limit = agenda_config::Config::load().max_events;
                let mut last_day: Option<NaiveDate> = None;
                for ev in events.iter().take(limit) {
                    let d = ev.start.date_naive();
                    if last_day != Some(d) {
                        rows.push(text::caption_heading(day_label(d)).class(dim()).into());
                        last_day = Some(d);
                    }
                    rows.push(
                        row::with_children(vec![
                            text::caption(time_label(ev))
                                .width(Length::Fixed(92.0))
                                .class(dim())
                                .into(),
                            text::body(ev.summary.clone()).width(Length::Fill).into(),
                        ])
                        .spacing(8)
                        .into(),
                    );
                }
            }
        }
        column::with_children(rows).spacing(5).into()
    }

    fn tasks_section(&self) -> Element<'_, Message> {
        let mut rows: Vec<Element<'_, Message>> = vec![Self::section_title("Zadania")];
        match &self.tasks {
            None => rows.push(text::caption("Ładowanie…").class(dim()).into()),
            Some(Err(e)) => rows.push(text::caption(format!("błąd: {e}")).class(dim()).into()),
            Some(Ok(items)) if items.is_empty() => {
                rows.push(text::caption("Nic zaległego 🎉").class(dim()).into())
            }
            Some(Ok(items)) => {
                for t in items {
                    let when = if t.overdue_days > 0 {
                        format!("{} dni", t.overdue_days)
                    } else {
                        "dziś".into()
                    };
                    rows.push(
                        row::with_children(vec![
                            text::caption(when)
                                .width(Length::Fixed(58.0))
                                .class(dim())
                                .into(),
                            text::body(t.text.clone()).width(Length::Fill).into(),
                            text::caption(t.priority.glyph().to_string()).into(),
                        ])
                        .spacing(8)
                        .into(),
                    );
                }
            }
        }
        column::with_children(rows).spacing(5).into()
    }

    fn markets_section(&self) -> Element<'_, Message> {
        let mut rows: Vec<Element<'_, Message>> = vec![Self::section_title("Kursy")];
        match &self.markets {
            None => rows.push(text::caption("Ładowanie…").class(dim()).into()),
            Some(Err(e)) => rows.push(text::caption(format!("błąd: {e}")).class(dim()).into()),
            Some(Ok(items)) => {
                for q in items {
                    let color = if q.change_pct >= 0.0 { UP } else { DOWN };
                    rows.push(
                        row::with_children(vec![
                            text::body(q.name.clone()).width(Length::Fill).into(),
                            text::caption(format!("{} {}", price_label(q.price), q.currency))
                                .into(),
                            text::caption(format!("{:+.1}%", q.change_pct))
                                .width(Length::Fixed(58.0))
                                .align_x(cosmic::iced::alignment::Horizontal::Right)
                                .class(cosmic::theme::Text::Color(color))
                                .into(),
                        ])
                        .spacing(8)
                        .into(),
                    );
                }
            }
        }
        column::with_children(rows).spacing(5).into()
    }
}

// =========================================================================
// Widok — lewa szuflada (drzewo vaulta)
// =========================================================================

/// Wcięcie jednego poziomu zagnieżdżenia w px.
const INDENT: f32 = 14.0;

impl Drawer {
    fn left_content(&self) -> Element<'_, Message> {
        let title = self
            .vault_root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Vault".into());
        let root = self.vault_root.clone();

        // Nagłówek: klik = odśwież, a przy przeciąganiu to cel „korzeń
        // vaulta" (stąd `on_release`).
        let hint = if self.fs.dragging() {
            "upuść tutaj = do korzenia vaulta"
        } else {
            "klik = odśwież · PPM na pozycji = akcje"
        };
        let header: Element<'_, Message> = mouse_area(
            container(
                column::with_children(vec![
                    text::title3(title).into(),
                    text::caption(hint).class(dim()).into(),
                ])
                .spacing(2),
            )
            .width(Length::Fill),
        )
        .on_press(Message::FilesReload)
        .on_release(Message::DropRoot)
        .into();

        let toolbar: Element<'_, Message> = flex_row(vec![
            chip("＋ notatka", Message::NewNote(root.clone()), false),
            chip("＋ folder", Message::NewFolder(root.clone()), false),
        ])
        .spacing(6)
        .into();

        let mut top: Vec<Element<'_, Message>> = vec![header, toolbar];

        if let Some(src) = &self.fs.moving {
            top.push(
                column::with_children(vec![
                    text::caption(format!(
                        "Przenoszę „{}\" — kliknij „⤵ tutaj\" przy folderze",
                        files::edit_label(src)
                    ))
                    .class(accent())
                    .into(),
                    flex_row(vec![
                        chip("⤵ do korzenia", Message::MoveHere(root.clone()), false),
                        chip("anuluj", Message::FsCancel, false),
                    ])
                    .spacing(6)
                    .into(),
                ])
                .spacing(4)
                .into(),
            );
        }
        if let Some((msg, err)) = &self.fs.status {
            let mut line = vec![
                text::caption(msg.clone())
                    .class(if *err {
                        cosmic::theme::Text::Color(DOWN)
                    } else {
                        dim()
                    })
                    .width(Length::Fill)
                    .into(),
            ];
            if self.fs.undo.is_some() {
                line.push(chip("↶ cofnij", Message::Undo, false));
            }
            top.push(
                row::with_children(line)
                    .spacing(6)
                    .align_y(cosmic::iced::Alignment::Center)
                    .into(),
            );
        }

        // Pole nowego wpisu w korzeniu idzie na samą górę listy.
        let mut rows: Vec<Element<'_, Message>> = Vec::new();
        if let Some(e) = &self.fs.edit {
            if matches!(&e.kind, EditKind::NewNote(d) | EditKind::NewFolder(d) if *d == root) {
                rows.push(self.edit_row(e, 0));
            }
        }
        match &self.tree {
            None => rows.push(text::caption("Ładowanie…").class(dim()).into()),
            Some(tree) => {
                let items = tree.rows();
                if items.is_empty() {
                    rows.push(
                        text::caption(format!("Pusto w {}", tree.root().display()))
                            .class(dim())
                            .into(),
                    );
                }
                for r in items {
                    rows.push(self.tree_row(&r));
                    let path = &r.entry.path;
                    if self.fs.menu.as_ref() == Some(path) {
                        rows.push(self.action_bar(&r));
                    }
                    // Nowy wpis w tym (rozwiniętym) katalogu — tuż pod nim.
                    if let Some(e) = &self.fs.edit {
                        if matches!(&e.kind, EditKind::NewNote(d) | EditKind::NewFolder(d) if d == path)
                        {
                            rows.push(self.edit_row(e, r.depth + 1));
                        }
                    }
                }
            }
        }

        let list = scrollable(column::with_children(rows).spacing(3))
            .scrollbar_width(0.0)
            .scroller_width(0.0)
            .scrollbar_padding(0.0)
            .width(Length::Fill)
            .height(Length::Fill);

        // Puszczenie przycisku w pustym miejscu listy kończy przeciąganie.
        let list = mouse_area(container(list).height(Length::Fill).width(Length::Fill))
            .on_release(Message::DragEnd);

        let mut parts: Vec<Element<'_, Message>> =
            vec![column::with_children(top).spacing(6).into(), list.into()];
        if let Some(cal) = &self.cal {
            parts.push(cosmic::widget::divider::horizontal::default().into());
            parts.push(self.calendar_view(cal));
        }
        column::with_children(parts).spacing(10).into()
    }

    /// Jeden wiersz drzewa. Lewy: klik = otwórz/rozwiń, przeciągnięcie na
    /// katalog = przenieś; prawy: pasek akcji.
    fn tree_row(&self, r: &files::Row) -> Element<'_, Message> {
        let path = &r.entry.path;
        let indent = INDENT * r.depth as f32;

        // Zmiana nazwy — pole edycji w miejscu wiersza.
        if let Some(e) = &self.fs.edit {
            if e.kind == EditKind::Rename(path.clone()) {
                return self.edit_row(e, r.depth);
            }
        }

        let glyph = if r.entry.is_dir {
            if r.expanded { "▾" } else { "▸" }
        } else {
            "·"
        };
        let dragged = self.fs.dragging() && self.fs.pressed.as_ref() == Some(path);
        let label: Element<'_, Message> = if r.entry.is_dir {
            text::body(r.entry.label.clone())
                .width(Length::Fill)
                .class(accent())
                .into()
        } else if dragged {
            text::body(r.entry.label.clone())
                .width(Length::Fill)
                .class(dim())
                .into()
        } else {
            text::body(r.entry.label.clone()).width(Length::Fill).into()
        };
        let mut items = vec![
            container(text::body("")).width(Length::Fixed(indent)).into(),
            text::body(glyph).width(Length::Fixed(16.0)).class(dim()).into(),
            label,
        ];
        // Tryb „przenieś…": cel wskazuje się przyciskiem przy katalogu,
        // a zwykły klik dalej rozwija — żeby dało się zejść głębiej.
        if r.entry.is_dir {
            if let Some(src) = &self.fs.moving {
                if !path.starts_with(src) && src.parent() != Some(path.as_path()) {
                    items.push(chip("⤵ tutaj", Message::MoveHere(path.clone()), false));
                }
            }
        }
        let line = row::with_children(items)
            .spacing(2)
            .align_y(cosmic::iced::Alignment::Center);

        // Podświetlenie: cel przeciągania (katalog pod kursorem) albo
        // wiersz z otwartym paskiem akcji.
        let hovered = self.fs.hovered.as_ref() == Some(path);
        let highlight = (self.fs.dragging() && hovered && r.entry.is_dir)
            || self.fs.menu.as_ref() == Some(path);
        let boxed = container(line)
            .width(Length::Fill)
            .class(cosmic::theme::Container::custom(move |_t: &Theme| {
                cosmic::iced::widget::container::Style {
                    background: highlight.then(|| {
                        Background::Color(Color::from_rgba(0.55, 0.75, 0.98, 0.22))
                    }),
                    border: cosmic::iced::Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }));

        mouse_area(boxed)
            .on_press(Message::RowPress(path.clone()))
            .on_release(Message::RowRelease(path.clone()))
            .on_right_press(Message::RowMenu(path.clone()))
            .on_enter(Message::RowEnter(path.clone()))
            .on_exit(Message::RowLeave(path.clone()))
            .into()
    }

    /// Pasek akcji pod wierszem (po PPM).
    fn action_bar(&self, r: &files::Row) -> Element<'_, Message> {
        let path = r.entry.path.clone();
        let mut chips = Vec::new();
        if r.entry.is_dir {
            chips.push(chip("＋ notatka", Message::NewNote(path.clone()), false));
            chips.push(chip("＋ folder", Message::NewFolder(path.clone()), false));
        }
        chips.push(chip("✎ nazwa", Message::Rename(path.clone()), false));
        chips.push(chip("⇄ przenieś", Message::MoveStart(path.clone()), false));
        if !r.entry.is_dir && files::is_note(&path) && self.config.files.open_with != "obsidian" {
            chips.push(chip("◆ w Obsidianie", Message::OpenInObsidian(path.clone()), false));
        }
        let confirm = self.fs.confirm_trash.as_ref() == Some(&path);
        chips.push(chip(
            if confirm { "🗑 na pewno?" } else { "🗑 do kosza" },
            Message::Trash(path),
            confirm,
        ));
        let indent = INDENT * r.depth as f32 + 18.0;
        container(flex_row(chips).spacing(4))
            .padding(Padding {
                top: 2.0,
                right: 0.0,
                bottom: 4.0,
                left: indent,
            })
            .into()
    }

    /// Pole nazwy: nowa notatka / nowy folder / zmiana nazwy.
    fn edit_row<'a>(&'a self, e: &'a Edit, depth: usize) -> Element<'a, Message> {
        let (glyph, placeholder) = match &e.kind {
            EditKind::NewNote(_) => ("·", "nazwa notatki"),
            EditKind::NewFolder(_) => ("▸", "nazwa folderu"),
            EditKind::Rename(_) => ("✎", "nowa nazwa"),
        };
        let input = text_input::text_input(placeholder, &e.value)
            .id(cosmic::widget::Id::new(EDIT_ID))
            .on_input(Message::EditInput)
            .on_submit(|_| Message::EditSubmit)
            .width(Length::Fill);
        row::with_children(vec![
            container(text::body(""))
                .width(Length::Fixed(INDENT * depth as f32))
                .into(),
            text::body(glyph).width(Length::Fixed(16.0)).class(dim()).into(),
            input.into(),
            chip("✓", Message::EditSubmit, false),
            chip("✕", Message::FsCancel, false),
        ])
        .spacing(4)
        .align_y(cosmic::iced::Alignment::Center)
        .into()
    }

    /// Kalendarzyk pod drzewem — klik w dzień otwiera notatkę dnia
    /// (`<vault>/<folder>/RRRR-MM-DD.md`), tworząc ją z szablonu, gdy jej
    /// jeszcze nie ma. Dni z gotową notatką są jasne, puste — przygaszone,
    /// dziś na kolor akcentu.
    fn calendar_view<'a>(&'a self, cal: &'a Calendar) -> Element<'a, Message> {
        let today = Local::now().date_naive();

        let arrow = |glyph: &'static str, msg: Message| -> Element<'a, Message> {
            mouse_area(
                container(text::body(glyph).class(dim()))
                    .padding([0, 6])
                    .center_y(Length::Fixed(22.0)),
            )
            .on_press(msg)
            .into()
        };

        let head = row::with_children(vec![
            arrow("‹", Message::CalShift(-1)),
            mouse_area(
                container(text::caption_heading(cal.month_label()).class(accent()))
                    .width(Length::Fill)
                    .center_x(Length::Fill),
            )
            .on_press(Message::CalToday)
            .into(),
            arrow("›", Message::CalShift(1)),
        ])
        .align_y(cosmic::iced::Alignment::Center);

        // Komórki są KWADRATOWE (życzenie użytkownika 2026-09-18): bok liczony
        // z szerokości karty minus jej padding, żeby siedem kolumn
        // wypełniało dokładnie tyle, ile jest miejsca, i ta sama liczba
        // idzie na wysokość.
        let cell = ((self.config.files.width as f32 - 28.0) / 7.0).max(18.0);

        let mut grid: Vec<Element<'a, Message>> = vec![
            head.into(),
            row::with_children(
                calendar::WEEKDAY_HEADS
                    .iter()
                    .map(|d| {
                        container(text::caption(*d).class(dim()))
                            .width(Length::Fixed(cell))
                            .center_x(Length::Fixed(cell))
                            .center_y(Length::Fixed(cell * 0.6))
                            .into()
                    })
                    .collect::<Vec<Element<'a, Message>>>(),
            )
            .into(),
        ];

        for week in cal.weeks() {
            let mut cols: Vec<Element<'a, Message>> = Vec::with_capacity(7);
            for slot in week {
                let Some(day) = slot else {
                    cols.push(
                        container(text::body(""))
                            .width(Length::Fixed(cell))
                            .height(Length::Fixed(cell))
                            .into(),
                    );
                    continue;
                };
                let label = text::body(format!("{}", day.day()));
                let label = if day == today {
                    label.class(accent())
                } else if cal.has_note(day) {
                    label
                } else {
                    label.class(dim())
                };
                cols.push(
                    mouse_area(
                        container(label)
                            .width(Length::Fixed(cell))
                            .height(Length::Fixed(cell))
                            .center_x(Length::Fixed(cell))
                            .center_y(Length::Fixed(cell)),
                    )
                    .on_press(Message::CalOpen(day))
                    .into(),
                );
            }
            grid.push(row::with_children(cols).into());
        }

        column::with_children(grid).spacing(4).into()
    }
}

fn day_label(d: NaiveDate) -> String {
    let today = Local::now().date_naive();
    if d == today {
        return "Dziś".into();
    }
    if d == today + ChronoDuration::days(1) {
        return "Jutro".into();
    }
    let wd = WEEKDAYS[d.weekday().num_days_from_monday() as usize];
    format!("{} {:02}.{:02}", wd, d.day(), d.month())
}

fn time_label(ev: &ics::AgendaEvent) -> String {
    if ev.all_day {
        "cały dzień".into()
    } else {
        format!("{:02}:{:02}", ev.start.hour(), ev.start.minute())
    }
}

/// Cena z sensowną liczbą miejsc po przecinku: grosze mają znaczenie przy
/// akcjach, ale nie przy bitcoinie, a przy shitcoinach liczą się ułamki.
fn price_label(v: f64) -> String {
    let a = v.abs();
    if a >= 1000.0 {
        format!("{v:.0}")
    } else if a >= 1.0 {
        format!("{v:.2}")
    } else {
        format!("{v:.4}")
    }
}

#[cfg(test)]
mod tests {
    use super::price_label;

    #[test]
    fn cena_ma_tyle_miejsc_ile_trzeba() {
        assert_eq!(price_label(64231.7), "64232");
        assert_eq!(price_label(231.456), "231.46");
        assert_eq!(price_label(0.12345), "0.1235");
    }
}

/// Mały przycisk-pastylka w drzewie. `danger` = czerwony (potwierdzenie
/// „do kosza").
fn chip<'a>(label: &'a str, msg: Message, danger: bool) -> Element<'a, Message> {
    let bg = if danger {
        Color::from_rgba(0.92, 0.48, 0.48, 0.35)
    } else {
        Color::from_rgba(1.0, 1.0, 1.0, 0.08)
    };
    mouse_area(
        container(text::caption(label))
            .padding([2, 8])
            .class(cosmic::theme::Container::custom(move |_t: &Theme| {
                cosmic::iced::widget::container::Style {
                    background: Some(Background::Color(bg)),
                    border: cosmic::iced::Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })),
    )
    .on_press(msg)
    .into()
}
