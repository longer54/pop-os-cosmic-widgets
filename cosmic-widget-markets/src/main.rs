mod app;
mod chart_html;
mod config;
mod market;

fn main() -> cosmic::iced::Result {
    let config = config::Config::load();

    let settings = cosmic::app::Settings::default()
        .no_main_window(true)
        .client_decorations(false)
        .transparent(true);

    cosmic::app::run::<app::Widget>(settings, config)
}
