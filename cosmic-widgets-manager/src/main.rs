//! Menedżer widżetów COSMIC / COSMIC widgets manager.

mod app;
mod catalog;
mod i18n;
mod obsidian;
mod settings;

fn main() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default().size(cosmic::iced::Size::new(760.0, 820.0))
        .size_limits(cosmic::iced::Limits::NONE.min_width(700.0).min_height(820.0));
    cosmic::app::run::<app::Manager>(settings, ())
}
