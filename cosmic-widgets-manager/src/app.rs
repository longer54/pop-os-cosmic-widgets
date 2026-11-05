use crate::catalog::{self, WIDGETS};
use crate::i18n::{polish, t};
use crate::obsidian;
use crate::settings::{self, Kind};
use cosmic::app::{Core, Task};
use cosmic::dialog::file_chooser;
use cosmic::iced::{Color, Length};
use cosmic::widget::{self, button, column, row, text};
use cosmic::{Application, ApplicationExt, Element, executor};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum Message {
    Refresh,
    Toggle(usize),
    Autostart(usize, bool),
    PickVault,
    VaultPicked(Option<PathBuf>),
    OpenSettings(usize),
    CloseSettings,
    SetField(usize, String),
    SaveSettings,
    Restart(usize),
    /// Ukryty tryb: CWM_SCREENSHOT=<plik.png> — zrzut samego okna aplikacji.
    Shot,
    ShotDone(cosmic::iced::core::window::Screenshot),
}

struct Form {
    widget: usize,
    values: Vec<String>,
}

pub struct Manager {
    core: Core,
    info: String,
    vault: Option<String>,
    form: Option<Form>,
    /// Widżet, który trzeba zrestartować po zapisie ustawień.
    restart_hint: Option<usize>,
    shot_path: Option<String>,
}

const GREEN: Color = Color::from_rgb(0.30, 0.78, 0.40);
const GREY: Color = Color::from_rgb(0.55, 0.55, 0.58);
const RED: Color = Color::from_rgb(0.85, 0.35, 0.35);

fn app_title() -> &'static str {
    t("Widżety COSMIC", "COSMIC Widgets")
}

fn dot(color: Color) -> Element<'static, Message> {
    text::body("●").class(cosmic::theme::Text::Color(color)).into()
}

impl Application for Manager {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "io.github.longer54.CosmicWidgetsManager";

    fn core(&self) -> &Core { &self.core }
    fn core_mut(&mut self) -> &mut Core { &mut self.core }

    fn init(core: Core, _: ()) -> (Self, Task<Message>) {
        let mut app = Self { core, info: String::new(), vault: obsidian::current_vault(), form: None, restart_hint: None, shot_path: std::env::var("CWM_SCREENSHOT").ok().filter(|p| !p.is_empty()) };
        // Ukryte, do zrzutów w README: CWM_OPEN_SETTINGS=<indeks widżetu>
        if let Some(i) = std::env::var("CWM_OPEN_SETTINGS").ok().and_then(|v| v.parse::<usize>().ok()).filter(|i| *i < WIDGETS.len()) {
            app.form = Some(Form { widget: i, values: settings::load(WIDGETS[i].bin) });
        }
        app.set_header_title(app_title().into());
        let id = app.core.main_window_id();
        let t = id.map(|id| app.set_window_title(app_title().into(), id)).unwrap_or_else(Task::none);
        (app, t)
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Refresh => {}
            Message::Toggle(i) => {
                let w = &WIDGETS[i];
                if catalog::is_running(w.bin) {
                    catalog::stop(w.bin);
                    self.info = format!("{}: {}", t("Zatrzymano", "Stopped"), w.label());
                } else {
                    self.info = match catalog::start(w.bin) {
                        Ok(()) => format!("{}: {}", t("Uruchomiono", "Started"), w.label()),
                        Err(e) => e,
                    };
                }
            }
            Message::Autostart(i, on) => {
                let w = &WIDGETS[i];
                let state = match (on, polish()) {
                    (true, true) => "włączone",
                    (false, true) => "wyłączone",
                    (true, false) => "enabled",
                    (false, false) => "disabled",
                };
                self.info = match catalog::set_autostart(w, on) {
                    Ok(()) => format!("{} {}: {}", t("Uruchamianie przy starcie", "Start at login"), state, w.label()),
                    Err(e) => e,
                };
            }
            Message::PickVault => {
                let title = t("Wybierz folder Obsidiana (z podfolderem .obsidian)", "Choose your Obsidian folder (containing .obsidian)");
                return cosmic::task::future(async move {
                    let r = file_chooser::open::Dialog::new().title(title).open_folder().await;
                    Message::VaultPicked(match r {
                        Ok(resp) => resp.url().to_file_path().ok(),
                        Err(_) => None,
                    })
                });
            }
            Message::VaultPicked(None) => {}
            Message::VaultPicked(Some(p)) => {
                self.info = match obsidian::apply_vault(&p) {
                    Ok(n) => {
                        self.vault = Some(p.to_string_lossy().into_owned());
                        if polish() {
                            format!("Folder Obsidiana podpięty ({n} widżetów). Zrestartuj działające widżety, żeby wczytały zmianę.")
                        } else {
                            format!("Obsidian folder linked ({n} widgets). Restart running widgets to apply it.")
                        }
                    }
                    Err(e) => e,
                };
            }
            Message::OpenSettings(i) => {
                self.form = Some(Form { widget: i, values: settings::load(WIDGETS[i].bin) });
                self.info.clear();
                self.restart_hint = None;
            }
            Message::CloseSettings => self.form = None,
            Message::SetField(k, v) => {
                if let Some(f) = &mut self.form {
                    if let Some(slot) = f.values.get_mut(k) {
                        *slot = v;
                    }
                }
            }
            Message::SaveSettings => {
                if let Some(f) = &self.form {
                    let w = &WIDGETS[f.widget];
                    match settings::save(w.bin, &f.values) {
                        Ok(n) => {
                            self.restart_hint = Some(f.widget);
                            self.info = if polish() {
                                format!("Zapisano ({n} zmian). Zrestartuj widżet, żeby zadziałało.")
                            } else {
                                format!("Saved ({n} changes). Restart the widget to apply.")
                            };
                        }
                        Err(e) => {
                            self.restart_hint = None;
                            self.info = format!("{}: {e}", t("Błędna wartość", "Invalid value"));
                        }
                    }
                }
            }
            Message::Shot => {
                if let (Some(id), Some(_)) = (self.core.main_window_id(), &self.shot_path) {
                    return cosmic::iced::window::screenshot(id).map(|s| cosmic::Action::App(Message::ShotDone(s)));
                }
            }
            Message::ShotDone(shot) => {
                if let Some(path) = self.shot_path.take() {
                    let res = (|| -> Result<(), String> {
                        let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
                        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), shot.size.width, shot.size.height);
                        enc.set_color(png::ColorType::Rgba);
                        enc.set_depth(png::BitDepth::Eight);
                        let mut w = enc.write_header().map_err(|e| e.to_string())?;
                        w.write_image_data(&shot.rgba).map_err(|e| e.to_string())
                    })();
                    if let Err(e) = res { eprintln!("screenshot: {e}"); std::process::exit(1); }
                    std::process::exit(0);
                }
            }
            Message::Restart(i) => {
                let w = &WIDGETS[i];
                catalog::stop(w.bin);
                std::thread::sleep(std::time::Duration::from_millis(500));
                self.info = match catalog::start(w.bin) {
                    Ok(()) => format!("{}: {}", t("Zrestartowano", "Restarted"), w.label()),
                    Err(e) => e,
                };
                self.restart_hint = None;
            }
        }
        Task::none()
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Message> {
        if self.shot_path.is_some() {
            cosmic::iced::time::every(std::time::Duration::from_secs(3)).map(|_| Message::Shot)
        } else {
            cosmic::iced::Subscription::none()
        }
    }

    fn view(&self) -> Element<'_, Message> {
        if let Some(f) = &self.form {
            return self.settings_view(f);
        }
        let mut list = column::with_capacity(32).spacing(6).padding(14);

        list = list.push(
            row::with_capacity(2)
                .align_y(cosmic::iced::Alignment::Center)
                .push(text::title3(t("Widżety i ustawienia", "Widgets and settings")).width(Length::Fill))
                .push(button::standard(t("Odśwież", "Refresh")).on_press(Message::Refresh)),
        );
        list = list.push(widget::divider::horizontal::heavy());

        list = list.push(text::title3(t("Folder Obsidiana", "Obsidian folder")));
        let vault_line = match &self.vault {
            Some(p) => format!("{}: {p}", t("Aktualny", "Current")),
            None => t("Nie wybrano. Wskaż folder, w którym jest .obsidian.", "Not set. Pick the folder that contains .obsidian.").to_string(),
        };
        list = list.push(
            row::with_capacity(2).spacing(12).align_y(cosmic::iced::Alignment::Center)
                .push(text::body(vault_line).width(Length::Fill))
                .push(button::suggested(t("Wybierz folder…", "Choose folder…")).on_press(Message::PickVault)),
        );

        list = list.push(widget::divider::horizontal::heavy());
        list = list.push(text::title3(t("Widżety", "Widgets")));
        for (i, w) in WIDGETS.iter().enumerate() {
            let installed = catalog::binary_path(w.bin).is_some();
            let running = installed && catalog::is_running(w.bin);
            let (color, status) = if !installed {
                (RED, t("nie zainstalowany", "not installed"))
            } else if running {
                (GREEN, t("działa", "running"))
            } else {
                (GREY, t("zatrzymany", "stopped"))
            };
            let toggle_btn = if !installed {
                button::standard(t("Uruchom", "Start"))
            } else if running {
                button::destructive(t("Zatrzymaj", "Stop")).on_press(Message::Toggle(i))
            } else {
                button::suggested(t("Uruchom", "Start")).on_press(Message::Toggle(i))
            };
            let head = row::with_capacity(3).spacing(8).align_y(cosmic::iced::Alignment::Center)
                .push(dot(color))
                .push(text::heading(w.label()))
                .push(text::caption(status));
            let info = column::with_capacity(2).spacing(2).push(head).push(text::body(w.desc())).width(Length::Fill);
            let mut auto = widget::toggler(catalog::has_autostart(w.bin)).label(t("Uruchamiaj przy starcie", "Start at login").to_string());
            if installed {
                auto = auto.on_toggle(move |on| Message::Autostart(i, on));
            }
            let settings_btn = button::standard(t("Ustawienia", "Settings")).on_press(Message::OpenSettings(i));
            if i > 0 {
                list = list.push(widget::divider::horizontal::light());
            }
            list = list.push(
                row::with_capacity(4).spacing(12).align_y(cosmic::iced::Alignment::Center)
                    .push(info)
                    .push(auto.width(Length::Shrink))
                    .push(settings_btn)
                    .push(toggle_btn),
            );
        }

        list = list.push(widget::divider::horizontal::heavy());
        list = list.push(self.info_row());

        widget::scrollable(list).into()
    }
}

impl Manager {
    fn info_row(&self) -> Element<'_, Message> {
        let mut r = row::with_capacity(2).spacing(12).align_y(cosmic::iced::Alignment::Center)
            .push(text::body(self.info.clone()).width(Length::Fill));
        if let Some(i) = self.restart_hint {
            if catalog::is_running(WIDGETS[i].bin) {
                r = r.push(button::suggested(t("Restart", "Restart")).on_press(Message::Restart(i)));
            }
        }
        r.into()
    }

    fn settings_view<'a>(&'a self, f: &'a Form) -> Element<'a, Message> {
        let w = &WIDGETS[f.widget];
        let mut col = column::with_capacity(24).spacing(10).padding(16);
        col = col.push(
            row::with_capacity(2).align_y(cosmic::iced::Alignment::Center)
                .push(text::title3(format!("{} — {}", t("Ustawienia", "Settings"), w.label())).width(Length::Fill))
                .push(button::standard(t("Wróć", "Back")).on_press(Message::CloseSettings)),
        );
        col = col.push(widget::divider::horizontal::heavy());
        col = col.push(text::caption(format!("~/.config/{}/config.toml", w.bin)));
        for (k, fld) in settings::fields(w.bin).iter().enumerate() {
            let val = &f.values[k];
            let input: Element<'a, Message> = match fld.kind {
                Kind::Anchor => {
                    let labels: &'static [&'static str] = if polish() { &settings::ANCHORS_PL } else { &settings::ANCHORS_EN };
                    let sel = settings::ANCHORS.iter().position(|a| a == val);
                    widget::dropdown(labels, sel, move |idx| Message::SetField(k, settings::ANCHORS[idx].to_string())).into()
                }
                Kind::Bool => widget::toggler(val == "true")
                    .on_toggle(move |on| Message::SetField(k, on.to_string()))
                    .into(),
                _ => widget::text_input(fld.default, val.as_str())
                    .on_input(move |s| Message::SetField(k, s))
                    .width(Length::Fixed(380.0))
                    .into(),
            };
            col = col.push(
                row::with_capacity(2).spacing(12).align_y(cosmic::iced::Alignment::Center)
                    .push(text::body(fld.label()).width(Length::Fixed(260.0)))
                    .push(input),
            );
        }
        col = col.push(widget::divider::horizontal::heavy());
        col = col.push(
            row::with_capacity(2).spacing(12)
                .push(button::suggested(t("Zapisz", "Save")).on_press(Message::SaveSettings))
                .push(button::standard(t("Wróć", "Back")).on_press(Message::CloseSettings)),
        );
        col = col.push(self.info_row());
        widget::scrollable(col).into()
    }
}
