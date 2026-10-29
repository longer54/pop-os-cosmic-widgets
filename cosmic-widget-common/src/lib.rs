//! Wspólny kod widgetów pulpitowych COSMIC.
//!
//! Mechanizmy:
//! - `drag` — uchwyt ⠿ w nagłówku i przeciąganie całej powierzchni
//!   layer-shell myszką (`set_margin` na żywo, zapis pozycji do configa
//!   po puszczeniu);
//! - `vault_io` — odczyt lokalny, ale zapis do vaulta przez kanoniczną
//!   kopię na serwerze (SSH), żeby dwa komputery nie mogły zapisać dwóch
//!   różnych wersji tej samej notatki i wyprodukować `*.sync-conflict-*`;
//! - `glass` — szkło w stylu COSMIC: przezroczystość kart z suwaka doka
//!   plus realne rozmycie tła (`ext-background-effect-v1`) w regionie
//!   dokładnie pod kartami;
//! - `geometry` + `WidgetPos` — publikacja geometrii do
//!   `~/.cache/cosmic-widgets/geometry/` i auto-rozsuwanie: widget z
//!   `stack_below = "<rodzic>"` trzyma się pod rodzicem i zjeżdża, gdy
//!   ten urośnie.

pub mod desktop;
pub mod drag;
pub mod geometry;
pub mod glass;
pub mod screen;
pub mod vault_io;
mod pos;

pub use drag::DragEvent;
pub use glass::Glass;
pub use geometry::Geometry;
pub use pos::{Outcome, WidgetPos};
pub use vault_io::Remote;
