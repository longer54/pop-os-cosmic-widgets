//! Menedżer widżetów COSMIC — GUI do włączania, autostartu i podpięcia Obsidiana.

mod app;
mod catalog;
mod obsidian;

fn main() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default().size(cosmic::iced::Size::new(720.0, 640.0));
    cosmic::app::run::<app::Manager>(settings, ())
}
