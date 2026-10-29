//! Przeciąganie widgetu myszką: uchwyt ⠿ do nagłówka + subskrypcja
//! eventów myszy. Logika przeliczania pozycji jest w `WidgetPos`.

use cosmic::iced::event::Event;
use cosmic::iced::window;
use cosmic::iced::{mouse, Color, Subscription};

#[derive(Debug, Clone)]
pub enum DragEvent {
    Moved { window: window::Id, x: f32, y: f32 },
    Released,
}

/// Subskrypcja ruchów myszy — włączać TYLKO gdy `WidgetPos::dragging()`,
/// inaczej każdy ruch kursora nad widgetem budziłby update().
pub fn subscription() -> Subscription<DragEvent> {
    cosmic::iced::event::listen_with(|event, _status, window| match event {
        Event::Mouse(mouse::Event::CursorMoved { position }) => Some(DragEvent::Moved {
            window,
            x: position.x,
            y: position.y,
        }),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            Some(DragEvent::Released)
        }
        _ => None,
    })
}

/// Uchwyt ⠿ do nagłówka widgetu; wciśnięcie startuje przeciąganie.
/// Wewnętrzny mouse_area przechwytuje kliknięcie, więc nie odpala
/// on_press zewnętrznej karty (np. otwierania panelu w przeglądarce).
pub fn handle<M: Clone + 'static>(on_press: M) -> cosmic::Element<'static, M> {
    let grip = cosmic::widget::text::body("⠿")
        .class(cosmic::theme::Text::Color(Color::from_rgba(
            0.95, 0.95, 0.97, 0.40,
        )));
    cosmic::widget::mouse_area(cosmic::widget::container(grip).padding([0, 6]))
        .on_press(on_press)
        .into()
}
