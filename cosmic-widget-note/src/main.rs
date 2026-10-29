mod app;
mod config;
mod ipc;
mod note;

/// Bez argumentów — widget (autostart). `--new` (skrót Super+Shift+N) /
/// `--edit` — klient: mówi działającemu widgetowi, co zrobić.
fn main() -> cosmic::iced::Result {
    if let Some(arg) = std::env::args().nth(1) {
        match ipc::Command::parse(&arg) {
            Some(cmd) => {
                if let Err(e) = ipc::send(cmd) {
                    eprintln!("cosmic-widget-note: {e}");
                    std::process::exit(1);
                }
                return Ok(());
            }
            None => {
                eprintln!(
                    "cosmic-widget-note [--new | --edit]\n\
                     \n\
                     bez argumentu  widget (autostart)\n\
                     --new          odłóż treść do archiwum, nowa pusta notatka\n\
                     --edit         kursor do pisania w obecnej notatce"
                );
                std::process::exit(2);
            }
        }
    }

    let config = config::Config::load();

    let settings = cosmic::app::Settings::default()
        .no_main_window(true)
        .client_decorations(false)
        .transparent(true);

    cosmic::app::run::<app::Widget>(settings, config)
}
