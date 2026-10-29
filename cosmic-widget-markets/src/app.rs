//! Widget kursów aktywów — powierzchnia layer-shell z jednym aktywem na
//! ekranie: cena, zmiana, klik na cenie otwiera pełny wykres (zakresy
//! 24h/7D/1M/1R) w przeglądarce, nawigacja strzałkami w obrębie zakładek
//! kategorii (Ulubione/Krypto/US/GPW/Surowce/ETF) oraz dodawanie nowych
//! aktywów z wyszukiwarki (przycisk +, wzorzec pola tekstowego z
//! cosmic-widget-tasks — KeyboardInteractivity::OnDemand). Szkielet
//! layer-shell 1:1 jak cosmic-widget-weather.

use std::collections::HashMap;
use std::time::Duration as StdDuration;

use cosmic::app::{Core, Task};
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::{
    IcedMargin, SctkLayerSurfaceSettings,
};
use cosmic::iced::platform_specific::shell::commands::layer_surface::{
    destroy_layer_surface, get_layer_surface, set_keyboard_interactivity, set_margin, set_size, Anchor, KeyboardInteractivity, Layer,
};
use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit};
use cosmic::iced::window;
use cosmic::iced::{Color, Length, Subscription};
use cosmic::widget::{button, container, mouse_area, text, text_input};
use cosmic::Element;
use cosmic_widget_common::{desktop, drag, glass, Glass, Outcome, WidgetPos};

use crate::chart_html::{self, ChartSeries};
use crate::config::{AssetCfg, Config};
use crate::market::{self, History, Quote, Range, SearchHit, Source};

// =========================================================================
// Zakładki kategorii
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Fav,
    Krypto,
    Us,
    Gpw,
    Surowce,
    Etf,
}

pub const TABS: [Tab; 6] = [Tab::Fav, Tab::Krypto, Tab::Us, Tab::Gpw, Tab::Surowce, Tab::Etf];

impl Tab {
    fn label(&self) -> &'static str {
        match self {
            Tab::Fav => "Ulub",
            Tab::Krypto => "Kry",
            Tab::Us => "US",
            Tab::Gpw => "GPW",
            Tab::Surowce => "Sur",
            Tab::Etf => "ETF",
        }
    }

    fn matches(&self, a: &AssetCfg) -> bool {
        match self {
            Tab::Fav => a.favorite,
            Tab::Krypto => a.category == "krypto",
            Tab::Us => a.category == "us",
            Tab::Gpw => a.category == "gpw",
            Tab::Surowce => a.category == "surowce",
            Tab::Etf => a.category == "etf",
        }
    }

    fn for_category(cat: &str) -> Tab {
        match cat {
            "krypto" => Tab::Krypto,
            "gpw" => Tab::Gpw,
            "surowce" => Tab::Surowce,
            "etf" => Tab::Etf,
            _ => Tab::Us,
        }
    }
}

// =========================================================================
// Aplikacja
// =========================================================================

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    CryptoQuotes(Result<HashMap<String, Quote>, String>),
    YahooQuote(String, Result<(Quote, Vec<(i64, f64)>), String>),
    Prev,
    Next,
    SelectTab(Tab),
    OpenChart,
    ChartReady {
        asset_id: String,
        asset_name: String,
        symbol: String,
        currency: String,
        histories: Vec<(Range, History)>,
    },
    ToggleAdd,
    InputChanged(String),
    SubmitSearch,
    SearchDone(Result<Vec<SearchHit>, String>),
    AddHit(usize),
    ToggleFavorite,
    DeleteCurrent,
    DragStart,
    Drag(drag::DragEvent),
    StackTick,
    /// Zmiana pulpitu wirtualnego w KWin: `true` = jesteśmy na „swoim"
    /// pulpicie (pokaż powierzchnię), `false` = na innym (schowaj).
    DesktopVisible(bool),
    /// Nowy poziom oszklenia — użytkownik ruszył suwakiem przezroczystości doka.
    Glass(Glass),
}

pub struct Widget {
    core: Core,
    surface_id: window::Id,
    config: Config,
    pos: WidgetPos,
    /// Szkło karty: przezroczystość + region rozmycia tła.
    glass: Glass,
    quotes: HashMap<String, Quote>,
    quote_errors: HashMap<String, String>,
    history: HashMap<(String, Range), History>,
    tab: Tab,
    idx: usize,
    hist_error: Option<String>,
    cfg_error: Option<String>,
    add_open: bool,
    input_value: String,
    searching: bool,
    search_results: Vec<SearchHit>,
    search_error: Option<String>,
    confirm_delete: bool,
    /// Czy powierzchnia layer-shell istnieje w tej chwili. `false` = jesteśmy
    /// na innym pulpicie wirtualnym i widget jest schowany.
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
/// powrocie na swój pulpit wirtualny (`Message::DesktopVisible`), a nie
/// tylko raz przy starcie.
fn surface_settings(
    id: window::Id,
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
        // Klawiatura TYLKO na czas pisania w polu wyszukiwania (ToggleAdd).
        // Na stałe włączona sprawiała, że KWin po kliknięciu w widget
        // przenosił go do warstwy aktywnej — nad okna, na wszystkich pulpitach.
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
        namespace: "cosmic-widget-markets".into(),
        ..Default::default()
    }
}

impl Widget {
    /// Indeksy aktywów (w config.assets) należących do bieżącej zakładki.
    fn tab_assets(&self) -> Vec<usize> {
        self.config
            .assets
            .iter()
            .enumerate()
            .filter(|(_, a)| self.tab.matches(a))
            .map(|(i, _)| i)
            .collect()
    }

    fn current_asset(&self) -> Option<&AssetCfg> {
        let list = self.tab_assets();
        list.get(self.idx.min(list.len().saturating_sub(1)))
            .map(|&i| &self.config.assets[i])
    }

    /// Zbiorcze odświeżenie cen: 1 strzał CoinGecko + po strzale Yahoo na
    /// aktywo (Yahoo przy okazji oddaje wykres 24h → od razu do cache'u).
    fn fetch_quotes_task(&self) -> Task<Message> {
        let cg_ids: Vec<String> = self
            .config
            .assets
            .iter()
            .filter(|a| Source::from_str(&a.source) == Source::Coingecko)
            .map(|a| a.symbol.clone())
            .collect();
        let mut tasks = vec![cosmic::task::future(async move {
            Message::CryptoQuotes(market::fetch_crypto_quotes(cg_ids).await)
        })];
        for a in &self.config.assets {
            if Source::from_str(&a.source) == Source::Yahoo {
                let id = a.id.clone();
                let sym = a.symbol.clone();
                tasks.push(cosmic::task::future(async move {
                    Message::YahooQuote(id, market::fetch_yahoo_quote(sym).await)
                }));
            }
        }
        Task::batch(tasks)
    }

    /// Zbiera to, co już mamy świeże w cache (wg `Range::cache_ttl`), i
    /// dociąga resztę zakresów potrzebnych do pełnego wykresu. Dla krypto
    /// to tylko H24+Y1 (patrz `chart_source_ranges`) — D7/M1 wycina z Y1
    /// dopiero `build_chart_series` po powrocie `ChartReady`.
    fn open_chart_task(&self) -> Task<Message> {
        let Some(a) = self.current_asset().cloned() else {
            return Task::none();
        };
        let Some(q) = self.quotes.get(&a.id) else {
            return Task::none();
        };
        let source = Source::from_str(&a.source);
        let currency = q.currency.clone();
        let asset_id = a.id.clone();
        let asset_name = a.name.clone();
        let symbol = a.symbol.clone();
        let ranges = chart_source_ranges(source).to_vec();
        let cached: Vec<(Range, History)> = ranges
            .iter()
            .filter_map(|&r| {
                self.history.get(&(asset_id.clone(), r)).and_then(|h| {
                    (h.fetched_at.elapsed() < r.cache_ttl()).then(|| (r, h.clone()))
                })
            })
            .collect();
        cosmic::task::future(async move {
            let mut histories = cached;
            for r in ranges {
                if histories.iter().any(|(hr, _)| *hr == r) {
                    continue;
                }
                if let Ok(h) = market::fetch_history(source, symbol.clone(), r).await {
                    histories.push((r, h));
                }
            }
            Message::ChartReady { asset_id, asset_name, symbol, currency, histories }
        })
    }

    /// Cena bieżącego aktywa wisi >5 min bez odświeżenia (brak sieci/API).
    fn current_stale(&self) -> bool {
        self.current_asset()
            .and_then(|a| self.quotes.get(&a.id))
            .is_some_and(|q| q.fetched_at.elapsed().as_secs() > 300)
    }

    fn estimated_height(&self) -> u32 {
        // padding karty + nagłówek + cena + nawigacja + zakładki
        let mut h: u32 = 24 + 28 + 34 + 22 + 32 + 6 * 7;
        if self.add_open {
            h += 40; // pole wyszukiwania
            h += 26 * self.search_results.len() as u32;
            if self.searching || self.search_error.is_some() {
                h += 22;
            }
            h += 34; // przycisk usuwania bieżącego aktywa
        }
        if self.hist_error.is_some() {
            h += 22;
        }
        if self.cfg_error.is_some() {
            h += 22;
        }
        if self.current_stale() {
            h += 22;
        }
        h.clamp(120, 900)
    }

    /// Klawiatura włączona tylko przy otwartym polu wyszukiwania — inaczej
    /// KWin trzyma widget w warstwie aktywnej, nad oknami aplikacji.
    fn keyboard_task(&self) -> Task<Message> {
        set_keyboard_interactivity(
            self.surface_id,
            if self.add_open {
                KeyboardInteractivity::OnDemand
            } else {
                KeyboardInteractivity::None
            },
        )
    }

    fn resize_task(&mut self) -> Task<Message> {
        let h = self.estimated_height();
        self.pos.set_height(h);
        Task::batch([
            set_size(self.surface_id, Some(self.config.width), Some(h)),
            self.blur_task(),
        ])
    }

    /// Region rozmycia tła pod kartą. Kompozytor gubi go razem z
    /// powierzchnią, więc wołamy to po każdym utworzeniu powierzchni i po
    /// każdej zmianie jej rozmiaru.
    fn blur_task(&self) -> Task<Message> {
        glass::blur_task_single(
            self.surface_id,
            self.glass,
            self.config.width,
            self.estimated_height(),
        )
    }

    fn save_config(&mut self) {
        self.cfg_error = self.config.save().err().map(|e| format!("zapis configu: {e}"));
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
        self.save_config();
        self.pos.publish();
    }
}

/// Zakresy, które trzeba mieć w cache, żeby zbudować pełny wykres. Krypto:
/// tylko H24+Y1 — CoinGecko (darmowe) rate-limituje serie po ~5 strzałach
/// (429 na całą minutę), D7/M1 wycinamy lokalnie z danych rocznych (patrz
/// `build_chart_series`).
fn chart_source_ranges(source: Source) -> &'static [Range] {
    match source {
        Source::Yahoo => &[Range::H24, Range::D7, Range::M1, Range::Y1],
        Source::Coingecko => &[Range::H24, Range::Y1],
    }
}

fn range_label(r: Range) -> &'static str {
    match r {
        Range::H24 => "24h",
        Range::D7 => "7D",
        Range::M1 => "1M",
        Range::Y1 => "1R",
    }
}

/// Buduje serie do przełącznika zakresu w przeglądarce (24h/7D/1M/1R) z
/// tego, co faktycznie udało się pobrać. Dla krypto D7/M1 nie mają
/// osobnego wpisu w `histories` — wycina je z Y1 przez `window_slice`.
/// Zakres pomijany, jeśli finalnie ma < 2 punkty (nic do narysowania).
fn build_chart_series(histories: &[(Range, History)]) -> Vec<ChartSeries> {
    let find = |r: Range| histories.iter().find(|(hr, _)| *hr == r).map(|(_, h)| h);
    [Range::H24, Range::D7, Range::M1, Range::Y1]
        .into_iter()
        .filter_map(|r| {
            let points = if let Some(h) = find(r) {
                h.points.clone()
            } else {
                let y1 = find(Range::Y1)?;
                match r {
                    Range::D7 => market::window_slice(&y1.points, market::WINDOW_D7_SECS),
                    Range::M1 => market::window_slice(&y1.points, market::WINDOW_M1_SECS),
                    _ => return None,
                }
            };
            (points.len() >= 2).then(|| ChartSeries { label: range_label(r), points })
        })
        .collect()
}

/// Zapisuje wykres do `~/.cache/cosmic-widget-markets/chart.html` i
/// otwiera w domyślnej przeglądarce. Bez fallbacków — błąd wraca jako
/// `Err` i trafia do `hist_error` w wywołującym.
fn write_and_open_chart(html: &str) -> Result<(), String> {
    let dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("cosmic-widget-markets");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("chart.html");
    std::fs::write(&path, html).map_err(|e| e.to_string())?;
    std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

impl cosmic::Application for Widget {
    type Executor = cosmic::executor::Default;
    type Flags = Config;
    type Message = Message;
    const APP_ID: &'static str = "io.github.longer54.CosmicWidgetMarkets";

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
        // Powierzchnia POWSTAJE PO `WidgetPos` i z jego marginesów, nie
        // z surowego configu — patrz `WidgetPos::new` (config bywa
        // nieświeży, prawdziwą pozycję zna opublikowana geometria).
        let pos = WidgetPos::new(
            "markets",
            &config.anchor,
            config.margin_x,
            config.margin_y,
            config.width,
            Some(config.stack_below.clone()),
            config.stack_gap,
            config.stack_offset_y,
        );
        pos.publish();

        let surface_task = get_layer_surface(surface_settings(
            surface_id,
            &config.anchor,
            pos.margin_x(),
            pos.margin_y(),
            config.width,
            320,
        ));

        let glass = Glass::from_config(&config.glass);

        let widget = Widget {
            core,
            surface_id,
            config,
            pos,
            glass,
            quotes: HashMap::new(),
            quote_errors: HashMap::new(),
            history: HashMap::new(),
            tab: Tab::Fav,
            idx: 0,
            hist_error: None,
            cfg_error: None,
            add_open: false,
            input_value: String::new(),
            searching: false,
            search_results: Vec::new(),
            search_error: None,
            confirm_delete: false,
            visible: true,
        };

        let fetch = widget.fetch_quotes_task();
        let blur = widget.blur_task();
        (widget, Task::batch([surface_task, blur, fetch]))
    }

    fn subscription(&self) -> Subscription<Message> {
        let secs = self.config.refresh_secs.max(60);
        let mut subs =
            vec![cosmic::iced::time::every(StdDuration::from_secs(secs)).map(|_| Message::Tick)];
        if self.pos.dragging() {
            subs.push(drag::subscription().map(Message::Drag));
        }
        // Schowany widget nie ma powierzchni, więc nie ma czego przesuwać.
        if self.pos.is_stacked() && self.visible {
            subs.push(
                self.pos.stack_subscription().map(|()| Message::StackTick),
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
        if id != self.surface_id {
            return text::body("").into();
        }
        self.build_widget()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => self.fetch_quotes_task(),
            Message::CryptoQuotes(Ok(map)) => {
                for (id, q) in map {
                    self.quote_errors.remove(&id);
                    self.quotes.insert(id, q);
                }
                Task::none()
            }
            Message::CryptoQuotes(Err(e)) => {
                for a in &self.config.assets {
                    if Source::from_str(&a.source) == Source::Coingecko
                        && !self.quotes.contains_key(&a.id)
                    {
                        self.quote_errors.insert(a.id.clone(), e.clone());
                    }
                }
                Task::none()
            }
            Message::YahooQuote(id, Ok((q, pts))) => {
                self.quote_errors.remove(&id);
                self.quotes.insert(id.clone(), q);
                if pts.len() >= 2 {
                    self.history.insert(
                        (id, Range::H24),
                        History { points: pts, fetched_at: std::time::Instant::now() },
                    );
                }
                Task::none()
            }
            Message::YahooQuote(id, Err(e)) => {
                self.quote_errors.insert(id, e);
                Task::none()
            }
            Message::Prev | Message::Next => {
                let len = self.tab_assets().len();
                if len > 0 {
                    self.idx = match message {
                        Message::Prev => (self.idx + len - 1) % len,
                        _ => (self.idx + 1) % len,
                    };
                }
                self.confirm_delete = false;
                self.hist_error = None;
                self.resize_task()
            }
            Message::SelectTab(tab) => {
                self.tab = tab;
                self.idx = 0;
                self.confirm_delete = false;
                self.hist_error = None;
                self.resize_task()
            }
            Message::OpenChart => self.open_chart_task(),
            Message::ChartReady { asset_id, asset_name, symbol, currency, histories } => {
                for (r, h) in &histories {
                    self.history.insert((asset_id.clone(), *r), h.clone());
                }
                let series = build_chart_series(&histories);
                if series.is_empty() {
                    self.hist_error = Some("brak danych do wykresu".into());
                    return Task::none();
                }
                let html = chart_html::render(&asset_name, &symbol, &currency, &series);
                match write_and_open_chart(&html) {
                    Ok(()) => self.hist_error = None,
                    Err(e) => self.hist_error = Some(format!("nie udało się otworzyć wykresu: {e}")),
                }
                Task::none()
            }
            Message::ToggleAdd => {
                self.add_open = !self.add_open;
                self.input_value.clear();
                self.search_results.clear();
                self.search_error = None;
                self.searching = false;
                self.confirm_delete = false;
                Task::batch([self.keyboard_task(), self.resize_task()])
            }
            Message::InputChanged(v) => {
                self.input_value = v;
                Task::none()
            }
            Message::SubmitSearch => {
                let q = self.input_value.trim().to_string();
                if q.len() < 2 {
                    return Task::none();
                }
                self.searching = true;
                self.search_error = None;
                self.search_results.clear();
                let search = cosmic::task::future(async move {
                    Message::SearchDone(market::search(q).await)
                });
                Task::batch([search, self.resize_task()])
            }
            Message::SearchDone(res) => {
                self.searching = false;
                match res {
                    Ok(hits) => self.search_results = hits,
                    Err(e) => self.search_error = Some(e),
                }
                self.resize_task()
            }
            Message::AddHit(i) => {
                let Some(hit) = self.search_results.get(i).cloned() else {
                    return Task::none();
                };
                // Duplikat → tylko przeskocz na istniejące aktywo.
                if !self.config.assets.iter().any(|a| a.id == hit.id) {
                    self.config.assets.push(AssetCfg {
                        id: hit.id.clone(),
                        name: hit.name.clone(),
                        source: hit.source.clone(),
                        symbol: hit.id.clone(),
                        category: hit.category.clone(),
                        favorite: true,
                    });
                    self.save_config();
                }
                // Przełącz widok na nowe aktywo (w jego kategorii, żeby
                // na pewno było widoczne niezależnie od bieżącej zakładki).
                self.tab = Tab::for_category(&hit.category);
                let list = self.tab_assets();
                self.idx = list
                    .iter()
                    .position(|&ai| self.config.assets[ai].id == hit.id)
                    .unwrap_or(0);
                self.add_open = false;
                self.input_value.clear();
                self.search_results.clear();
                Task::batch([
                    self.keyboard_task(),
                    self.fetch_quotes_task(),
                    self.resize_task(),
                ])
            }
            Message::ToggleFavorite => {
                let list = self.tab_assets();
                if let Some(&ai) = list.get(self.idx) {
                    self.config.assets[ai].favorite = !self.config.assets[ai].favorite;
                    let went_off_fav = self.tab == Tab::Fav && !self.config.assets[ai].favorite;
                    let id = self.config.assets[ai].id.clone();
                    let cat = self.config.assets[ai].category.clone();
                    self.save_config();
                    // Na Ulubionych odznaczone aktywo znikałoby spod kursora,
                    // a pod gwiazdkę wsuwałoby się następne — seria kliknięć
                    // potrafiła skasować całą listę (stało się 2026-07-11).
                    // Zamiast tego przeskakujemy do kategorii aktywa: nic nie
                    // znika z ekranu, ponowny klik przywraca gwiazdkę.
                    if went_off_fav {
                        self.tab = Tab::for_category(&cat);
                        self.idx = self
                            .tab_assets()
                            .iter()
                            .position(|&i| self.config.assets[i].id == id)
                            .unwrap_or(0);
                    }
                }
                self.resize_task()
            }
            Message::DeleteCurrent => {
                if !self.confirm_delete {
                    self.confirm_delete = true;
                    return self.resize_task();
                }
                let list = self.tab_assets();
                if let Some(&ai) = list.get(self.idx) {
                    self.config.assets.remove(ai);
                    self.save_config();
                    let len = self.tab_assets().len();
                    if len == 0 {
                        self.idx = 0;
                    } else if self.idx >= len {
                        self.idx = len - 1;
                    }
                }
                self.confirm_delete = false;
                self.resize_task()
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
            // przypiąć layer-shella do pulpitu); powrót = odtwórz ją z
            // aktualną pozycją i wysokością.
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
                            self.estimated_height(),
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
const ACCENT: Color = Color::from_rgb(0.55, 0.75, 0.98);
const UP: Color = Color::from_rgb(0.30, 0.85, 0.45);
const DOWN: Color = Color::from_rgb(0.95, 0.40, 0.40);

fn dim_text() -> cosmic::theme::Text {
    cosmic::theme::Text::Color(DIM)
}

fn accent_text() -> cosmic::theme::Text {
    cosmic::theme::Text::Color(ACCENT)
}

/// Format ceny: tysiące ze spacją, liczba miejsc zależna od rzędu wielkości
/// (67 420 / 233,00 / 0,0745).
pub fn fmt_price(p: f64) -> String {
    let formatted = if p >= 1000.0 {
        let s = format!("{:.0}", p);
        let mut out = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push(' ');
            }
            out.push(c);
        }
        out
    } else if p >= 1.0 {
        format!("{:.2}", p)
    } else {
        format!("{:.4}", p)
    };
    formatted.replace('.', ",")
}

fn currency_label(code: &str) -> &str {
    match code {
        "USD" => "$",
        "PLN" => "zł",
        "EUR" => "€",
        "GBP" => "£",
        other => other,
    }
}

impl Widget {
    fn build_widget(&self) -> Element<'_, Message> {
        let width = self.config.width as f32;
        let mut rows: Vec<Element<'_, Message>> = Vec::new();

        let list = self.tab_assets();
        let asset = self.current_asset();

        // ---- Nagłówek: nazwa + gwiazdka ----
        match asset {
            Some(a) => {
                let star = button::text(if a.favorite { "★" } else { "☆" })
                    .class(cosmic::theme::Button::Transparent)
                    .on_press(Message::ToggleFavorite);
                rows.push(
                    cosmic::widget::row::with_children(vec![
                        drag::handle(Message::DragStart),
                        text::heading(a.name.clone())
                            .width(Length::Fill)
                            // Karta jest wąska (szerokość jak notatnik), a
                            // nazwy w rodzaju „S&P 500 Cons.Disc. (iShares)"
                            // zawijały się na drugą linię i wypychały
                            // zakładki poza jawną wysokość karty.
                            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
                            .into(),
                        text::caption(a.symbol.clone()).class(dim_text()).into(),
                        star.into(),
                    ])
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center)
                    .into(),
                );
            }
            None => {
                rows.push(
                    cosmic::widget::row::with_children(vec![
                        drag::handle(Message::DragStart),
                        text::heading("Brak aktywów").width(Length::Fill).into(),
                    ])
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center)
                    .into(),
                );
            }
        }

        // ---- Cena + zmiana ----
        if let Some(a) = asset {
            match self.quotes.get(&a.id) {
                Some(q) => {
                    let up = q.change_pct >= 0.0;
                    let arrow = if up { "▲" } else { "▼" };
                    let change_color =
                        cosmic::theme::Text::Color(if up { UP } else { DOWN });
                    let cur = currency_label(&q.currency);
                    let price_txt = if cur == "$" {
                        format!("${}", fmt_price(q.price))
                    } else {
                        format!("{} {}", fmt_price(q.price), cur)
                    };
                    let price_row = cosmic::widget::row::with_children(vec![
                        text::title3(price_txt).into(),
                        text::body(format!("{arrow} {:+.2}%", q.change_pct))
                            .class(change_color)
                            .into(),
                    ])
                    .spacing(10)
                    .align_y(cosmic::iced::Alignment::Center);
                    rows.push(mouse_area(price_row).on_press(Message::OpenChart).into());
                    // Odświeżanie idzie co refresh_secs; jak cena wisi dużo
                    // dłużej (brak sieci, API leży), pokazujemy jej wiek.
                    let age = q.fetched_at.elapsed().as_secs();
                    if age > 300 {
                        rows.push(
                            text::caption(format!("dane nieświeże ({} min)", age / 60))
                                .class(dim_text())
                                .into(),
                        );
                    }
                }
                None => {
                    let msg = match self.quote_errors.get(&a.id) {
                        Some(e) => format!("brak danych: {e}"),
                        None => "Ładowanie…".into(),
                    };
                    rows.push(text::caption(msg).class(dim_text()).into());
                }
            }
        }

        if let Some(e) = &self.hist_error {
            rows.push(text::caption(e.clone()).class(dim_text()).into());
        }

        // ---- Nawigacja (‹ bieżące/wszystkie ›) + dodawanie ----
        // Drobne przyciski (48×22) — pełnowymiarowe klocki przygniatały
        // resztę karty (feedback użytkownika 2026-07-14).
        let small_btn = |label: &'static str, msg: Option<Message>| {
            let mut b = button::custom(
                container(text::body(label))
                    .center_x(Length::Fixed(48.0))
                    .center_y(Length::Fixed(22.0)),
            )
            .class(cosmic::theme::Button::Standard)
            .padding([0, 0]);
            if let Some(m) = msg {
                b = b.on_press(m);
            }
            b
        };
        rows.push(
            cosmic::widget::row::with_children(vec![
                small_btn("‹", Some(Message::Prev)).into(),
                container(
                    text::caption(if list.is_empty() {
                        "0/0".to_string()
                    } else {
                        format!("{}/{}", self.idx.min(list.len() - 1) + 1, list.len())
                    })
                    .class(dim_text()),
                )
                .center_x(Length::Fixed(40.0))
                .into(),
                small_btn("›", Some(Message::Next)).into(),
                cosmic::iced::widget::space().width(Length::Fill).into(),
                small_btn("+", Some(Message::ToggleAdd)).into(),
            ])
            .spacing(4)
            .align_y(cosmic::iced::Alignment::Center)
            .into(),
        );

        // ---- Panel dodawania ----
        if self.add_open {
            let input = text_input::text_input("np. AMD, pepco, pepe…", &self.input_value)
                .on_input(Message::InputChanged)
                .on_submit(|_| Message::SubmitSearch)
                .width(Length::Fill);
            let search_btn = button::text("Szukaj").on_press(Message::SubmitSearch);
            rows.push(
                cosmic::widget::row::with_children(vec![input.into(), search_btn.into()])
                    .spacing(6)
                    .align_y(cosmic::iced::Alignment::Center)
                    .into(),
            );
            if self.searching {
                rows.push(text::caption("Szukam…").class(dim_text()).into());
            }
            if let Some(e) = &self.search_error {
                rows.push(text::caption(e.clone()).class(dim_text()).into());
            }
            for (i, hit) in self.search_results.iter().enumerate() {
                let row = cosmic::widget::row::with_children(vec![
                    text::caption(hit.category.clone())
                        .width(Length::Fixed(52.0))
                        .class(accent_text())
                        .into(),
                    text::body(hit.name.clone())
                        .width(Length::Fill)
                        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
                        .into(),
                    text::caption(hit.symbol.to_uppercase()).class(dim_text()).into(),
                ])
                .spacing(6)
                .align_y(cosmic::iced::Alignment::Center);
                rows.push(mouse_area(row).on_press(Message::AddHit(i)).into());
            }
            // Usuwanie bieżącego aktywa — tylko w trybie edycji, z
            // potwierdzeniem drugim kliknięciem.
            if let Some(a) = asset {
                let label = if self.confirm_delete {
                    format!("Na pewno usunąć {}?", a.name)
                } else {
                    format!("Usuń {}", a.name)
                };
                let del = button::custom(
                    text::caption(label).class(cosmic::theme::Text::Color(DOWN)),
                )
                .class(cosmic::theme::Button::Transparent)
                .on_press(Message::DeleteCurrent);
                rows.push(del.into());
            }
        }

        // ---- Błędy configu ----
        if let Some(e) = &self.cfg_error {
            rows.push(text::caption(e.clone()).class(dim_text()).into());
        }

        // ---- Zakładki kategorii ----
        let tab_buttons: Vec<Element<'_, Message>> = TABS
            .iter()
            .map(|t| {
                let active = *t == self.tab;
                let label = if active {
                    text::caption(t.label()).class(accent_text())
                } else {
                    text::caption(t.label()).class(dim_text())
                };
                button::custom(label)
                    .class(cosmic::theme::Button::Transparent)
                    .on_press(Message::SelectTab(*t))
                    .into()
            })
            .collect();
        rows.push(
            cosmic::widget::row::with_children(tab_buttons)
                .spacing(2)
                .align_y(cosmic::iced::Alignment::Center)
                .into(),
        );

        let content = cosmic::widget::column::with_children(rows).spacing(6);

        container(content)
            .padding([12, 14])
            .class(cosmic::theme::Container::custom(self.glass.container()))
            .width(Length::Fixed(width))
            // Wysokość JAWNA, taka sama jak wysokość powierzchni i regionu
            // rozmycia (`estimated_height`). Bez tego karta kurczy się do
            // treści, a szkło — liczone z wysokości powierzchni — wystaje
            // spod niej rozmytym paskiem.
            .height(Length::Fixed(self.estimated_height() as f32))
            .into()
    }
}

