use crate::catalog::{self, WIDGETS};
use crate::obsidian;
use cosmic::app::{Core, Task};
use cosmic::dialog::file_chooser;
use cosmic::iced::Length;
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
    Info(String),
}

pub struct Manager {
    core: Core,
    info: String,
    vault: Option<String>,
}

impl Application for Manager {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "io.github.longer54.CosmicWidgetsManager";

    fn core(&self) -> &Core { &self.core }
    fn core_mut(&mut self) -> &mut Core { &mut self.core }

    fn init(core: Core, _: ()) -> (Self, Task<Message>) {
        let mut app = Self { core, info: String::new(), vault: obsidian::current_vault() };
        app.set_header_title("Widżety COSMIC".into());
        let id = app.core.main_window_id();
        let t = id.map(|id| app.set_window_title("Widżety COSMIC".into(), id)).unwrap_or_else(Task::none);
        (app, t)
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Refresh => {}
            Message::Toggle(i) => {
                let w = &WIDGETS[i];
                if catalog::is_running(w.bin) {
                    catalog::stop(w.bin);
                    self.info = format!("Zatrzymano: {}", w.label);
                } else {
                    self.info = match catalog::start(w.bin) {
                        Ok(()) => format!("Uruchomiono: {}", w.label),
                        Err(e) => e,
                    };
                }
            }
            Message::Autostart(i, on) => {
                let w = &WIDGETS[i];
                self.info = match catalog::set_autostart(w, on) {
                    Ok(()) => format!("Autostart {}: {}", if on { "włączony" } else { "wyłączony" }, w.label),
                    Err(e) => e,
                };
            }
            Message::PickVault => {
                return cosmic::task::future(async {
                    let r = file_chooser::open::Dialog::new().title("Wybierz folder vaulta Obsidiana").open_folder().await;
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
                        format!("Vault podpięty ({n} widżetów). Zrestartuj działające widżety, żeby wczytały zmianę.")
                    }
                    Err(e) => e,
                };
            }
            Message::Info(s) => self.info = s,
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let mut list = column::with_capacity(WIDGETS.len() + 4).spacing(8).padding(16);

        list = list.push(
            row::with_capacity(2)
                .spacing(12)
                .push(text::title2("Widżety COSMIC").width(Length::Fill))
                .push(button::suggested("Odśwież").on_press(Message::Refresh)),
        );
        list = list.push(widget::divider::horizontal::default());
        list = list.push(text::title3("Obsidian"));
        let vault_line = match &self.vault {
            Some(v) => format!("Vault: {v}"),
            None => "Vault nie jest ustawiony".to_string(),
        };
        list = list.push(
            row::with_capacity(2)
                .spacing(12)
                .push(text::body(vault_line).width(Length::Fill))
                .push(button::suggested("Wybierz folder…").on_press(Message::PickVault)),
        );

        list = list.push(widget::divider::horizontal::default());
        list = list.push(text::title3("Widżety"));
        for (i, w) in WIDGETS.iter().enumerate() {
            let installed = catalog::binary_path(w.bin).is_some();
            let running = catalog::is_running(w.bin);
            let toggle_btn = if !installed {
                button::standard("Nie zainstalowany")
            } else if running {
                button::destructive("Zatrzymaj").on_press(Message::Toggle(i))
            } else {
                button::suggested("Uruchom").on_press(Message::Toggle(i))
            };
            let info = column::with_capacity(2)
                .push(text::heading(w.label))
                .push(text::body(w.desc))
                .width(Length::Fill);
            let mut auto = widget::toggler(catalog::has_autostart(w.bin)).label(String::from("Autostart"));
            if installed {
                auto = auto.on_toggle(move |on| Message::Autostart(i, on));
            }
            list = list.push(
                row::with_capacity(3).spacing(12).align_y(cosmic::iced::Alignment::Center)
                    .push(info)
                    .push(auto.width(Length::Shrink))
                    .push(toggle_btn),
            );
        }

        list = list.push(widget::divider::horizontal::default());
        list = list.push(text::body(self.info.clone()));

        widget::scrollable(list).into()
    }
}
