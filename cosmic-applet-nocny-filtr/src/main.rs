//! Aplet panelu COSMIC do nocnego filtra swiatla niebieskiego.
//!
//! Ikonka w panelu, klik otwiera popup z przelacznikiem i suwakiem
//! temperatury. Sam nic nie nakłada na ekran — kolory ustawia demon
//! `drm-colortemp`, a aplet wola pomocnika roota (`nocny-filtr-apply`),
//! ktory zapisuje temperature i robi mignięcie konsoli, bo tylko wtedy
//! COSMIC oddaje ekran na moment.

mod config;

use cosmic::app::{Core, Task};
use cosmic::iced::core::window;
use cosmic::iced::window::Id;
use cosmic::iced::{Rectangle, Subscription};
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{list_column, settings, slider, text, toggler};
use cosmic::{Application, Element};
use std::time::Duration;

use config::{Demon, Stan, Tryb, KROK_KELWINY, MAX_KELWINY, MIN_KELWINY};

const ID: &str = "io.github.longer54.CosmicAppletNocnyFiltr";

pub struct Aplet {
    core: Core,
    popup: Option<Id>,
    stan: Stan,
    demon: Demon,
}

#[derive(Clone, Debug)]
pub enum Message {
    PopupClosed(Id),
    Surface(cosmic::surface::Action),
    Wlaczony(bool),
    Automatyczny(bool),
    Temperatura(u32),
    ZastosujTemperature,
    Odswiez,
}

impl Aplet {
    fn filtr_aktywny(&self) -> bool {
        self.stan.tryb != Tryb::Wylaczony
    }

    fn opis_harmonogramu(&self) -> String {
        format!(
            "Noc od {:02}:00 do {:02}:00 · {} K",
            self.demon.sunset_hour, self.demon.sunrise_hour, self.demon.night_temp
        )
    }

    fn zawartosc_popupu(&self) -> Element<'_, Message> {
        let automatyczny = self.stan.tryb == Tryb::Automatyczny;

        let mut kolumna = list_column().add(settings::item(
            "Nocny filtr",
            toggler(self.filtr_aktywny()).on_toggle(Message::Wlaczony),
        ));

        if self.filtr_aktywny() {
            kolumna = kolumna.add(settings::item(
                "Automatycznie wg godzin",
                toggler(automatyczny).on_toggle(Message::Automatyczny),
            ));

            if automatyczny {
                kolumna = kolumna.add(cosmic::applet::padded_control(text::caption(
                    self.opis_harmonogramu(),
                )));
            } else {
                kolumna = kolumna
                    .add(settings::item(
                        "Temperatura",
                        text::body(format!("{} K", self.stan.temperatura)),
                    ))
                    .add(cosmic::applet::padded_control(
                        slider(
                            MIN_KELWINY..=MAX_KELWINY,
                            self.stan.temperatura.clamp(MIN_KELWINY, MAX_KELWINY),
                            Message::Temperatura,
                        )
                        .step(KROK_KELWINY)
                        .on_release(Message::ZastosujTemperature),
                    ));
            }
        }

        kolumna = kolumna.add(cosmic::applet::padded_control(text::caption(
            "Przy zmianie ekran mignie na chwilę.",
        )));

        self.core.applet.popup_container(kolumna).into()
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
        (
            Aplet {
                core,
                popup: None,
                stan: config::wczytaj_stan(),
                demon: config::wczytaj_demona(),
            },
            Task::none(),
        )
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
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(action),
                ));
            }
            Message::Wlaczony(wlaczony) => {
                if wlaczony {
                    // Wracamy do trybu, ktory byl przed wylaczeniem: jesli
                    // uzytkownik nigdy nie ruszal suwaka, to harmonogram.
                    self.stan.tryb = Tryb::Automatyczny;
                    config::zastosuj("auto");
                } else {
                    self.stan.tryb = Tryb::Wylaczony;
                    config::zastosuj("off");
                }
                config::zapisz_stan(&self.stan);
            }
            Message::Automatyczny(automatyczny) => {
                if automatyczny {
                    self.stan.tryb = Tryb::Automatyczny;
                    config::zastosuj("auto");
                } else {
                    self.stan.tryb = Tryb::Wlaczony;
                    config::zastosuj(&self.stan.temperatura.to_string());
                }
                config::zapisz_stan(&self.stan);
            }
            Message::Temperatura(kelwiny) => {
                // W trakcie przeciagania tylko podglad — kazde drgniecie suwaka
                // oznaczaloby mignięcie ekranu.
                self.stan.temperatura = kelwiny;
            }
            Message::ZastosujTemperature => {
                self.stan.tryb = Tryb::Wlaczony;
                config::zapisz_stan(&self.stan);
                config::zastosuj(&self.stan.temperatura.to_string());
            }
            Message::Odswiez => {
                self.demon = config::wczytaj_demona();
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let ikona = if self.filtr_aktywny() {
            "weather-clear-night-symbolic"
        } else {
            "weather-clear-symbolic"
        };

        let otwarty = self.popup;
        let przycisk = self
            .core
            .applet
            .icon_button(ikona)
            .on_press_with_rectangle(move |offset, bounds| {
                if let Some(id) = otwarty {
                    Message::Surface(destroy_popup(id))
                } else {
                    Message::Surface(app_popup::<Aplet>(
                        |_| Default::default(),
                        move |state: &mut Aplet| {
                            let nowe_id = Id::unique();
                            state.popup = Some(nowe_id);
                            state.demon = config::wczytaj_demona();

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
                        Some(Box::new(move |state: &Aplet| {
                            state.zawartosc_popupu().map(cosmic::Action::App)
                        })),
                    ))
                }
            });

        let podpowiedz = match self.stan.tryb {
            Tryb::Wylaczony => "Nocny filtr wyłączony".to_string(),
            Tryb::Automatyczny => format!("Nocny filtr automatyczny · {}", self.opis_harmonogramu()),
            Tryb::Wlaczony => format!("Nocny filtr: {} K", self.stan.temperatura),
        };

        self.core
            .applet
            .applet_tooltip::<Message>(
                przycisk,
                podpowiedz,
                self.popup.is_some(),
                Message::Surface,
                None,
            )
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        text::body("").into()
    }

    fn subscription(&self) -> Subscription<Message> {
        cosmic::iced::time::every(Duration::from_secs(60)).map(|_| Message::Odswiez)
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<Aplet>(())
}
