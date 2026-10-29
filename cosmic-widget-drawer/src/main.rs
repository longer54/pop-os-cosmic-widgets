//! Centrum powiadomień dla COSMIC — szuflada wysuwana od prawej krawędzi.
//!
//! Dwa tryby w jednym pliku wykonywalnym:
//! - bez argumentów — demon (autostart): trzyma dane, nasłuchuje gniazda,
//!   pilnuje gorącego paska przy krawędzi;
//! - `--toggle` / `--show` / `--hide` — klient: mówi działającemu demonowi,
//!   co ma zrobić (to podpinamy pod skrót klawiszowy), a gdy demona nie ma,
//!   uruchamia go i ponawia. To samo z końcówką `-files` dotyczy lewej
//!   szuflady (drzewo vaulta), a `--swipe-left` / `--swipe-right` to gesty
//!   gładzika: demon sam decyduje, którą szufladę ruszyć.

mod app;
mod calendar;
mod config;
mod files;
mod ipc;
mod panel;
mod sources;

fn main() -> cosmic::iced::Result {
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            eprintln!(
                "cosmic-widget-drawer [OPCJA]\n\
                 \n\
                 bez argumentu       demon (autostart)\n\
                 --toggle|--show|--hide         prawa szuflada (widgety)\n\
                 --toggle-files|--show-files|--hide-files   lewa (vault)\n\
                 --toggle-both                  obie naraz (Super+N)\n\
                 --swipe-left|--swipe-right     gest gładzika\n\
                 \n\
                 config: {}",
                config::Config::path().display()
            );
            return Ok(());
        }
        match ipc::Command::parse(&arg) {
            Some(cmd) => {
                if let Err(e) = ipc::send(cmd) {
                    eprintln!("cosmic-widget-drawer: {e}");
                    std::process::exit(1);
                }
                return Ok(());
            }
            None => {
                eprintln!("cosmic-widget-drawer: nieznany argument „{arg}” (zobacz --help)");
                std::process::exit(2);
            }
        }
    }

    let config = config::Config::load();
    let settings = cosmic::app::Settings::default()
        .no_main_window(true)
        .client_decorations(false)
        .transparent(true);

    cosmic::app::run::<app::Drawer>(settings, config)
}
