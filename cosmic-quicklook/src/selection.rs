//! Skąd wziąć ścieżkę pliku zaznaczonego w menedżerze plików.
//!
//! To jest najtrudniejszy kawałek całego Quick Looka. COSMIC Files (ani
//! Dolphin) nie wystawia API, D-Busa ani niczego, co powiedziałoby „user ma
//! zaznaczone /home/user/x.pdf". Jedyne, co menedżer plików robi na żądanie
//! z zaznaczeniem, to **kopiowanie do schowka**: wtedy wystawia
//! `text/uri-list` (widać to w binarce `cosmic-files`, stała
//! `text/uri-list` obok `x-special/gnome-copied-files`).
//!
//! Dlatego sekwencja jest taka:
//!
//! 1. sprawdź, które okno jest na wierzchu (`ext-foreign-toplevel-list` +
//!    `cosmic-toplevel-info`) — jeśli to nie menedżer plików, nie ruszamy
//!    schowka i mówimy o tym. **To nie jest kosmetyka:** Ctrl+C wysłane do
//!    terminala to SIGINT, czyli ubity build;
//! 2. zapamiętaj obecną zawartość schowka (typ + bajty);
//! 3. udaj Ctrl+C **wirtualną klawiaturą Waylanda**
//!    (`zwp_virtual_keyboard_manager_v1`) — u autora nie ma ani `wtype`, ani
//!    `ydotool`, a i tak byłyby gorsze: `ydotool` chce `/dev/uinput`, czyli
//!    roota, a my mamy własne połączenie z kompozytorem za darmo;
//! 4. przeczytaj `text/uri-list` (`wl-paste`);
//! 5. **odtwórz schowek** — użytkownik nie może stracić tego, co skopiował.
//!
//! Wirtualna klawiatura potrzebuje własnej mapy klawiszy: definiujemy
//! minimalną z dwoma klawiszami (`Control_L` i `c`), więc znamy ich kody na
//! pewno, niezależnie od układu klawiatury użytkownika (na Dvoraku „c" siedzi
//! gdzie indziej niż na QWERTY).

use std::collections::HashMap;
use std::io::{Seek, Write as _};
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::time::Duration;

use cosmic_protocols::toplevel_info::v1::client::{
    zcosmic_toplevel_handle_v1::{self, State as CosmicState, ZcosmicToplevelHandleV1},
    zcosmic_toplevel_info_v1::{self, ZcosmicToplevelInfoV1},
};
use wayland_client::backend::ObjectId;
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
    ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1},
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

/// Mapa klawiszy wirtualnej klawiatury. Kody xkb = kody evdev + 8, więc
/// `<LCTL> = 37` to evdev 29 (KEY_LEFTCTRL), a `<AC03> = 54` to evdev 46
/// (KEY_C). Te dwa evdevowe numery wysyłamy potem w `key()`.
const KEYMAP: &str = r#"xkb_keymap {
xkb_keycodes "quicklook" {
	minimum = 8;
	maximum = 255;
	<LCTL> = 37;
	<AC03> = 54;
	<DOWN> = 116;
	<SPCE> = 65;
	<RGHT> = 114;
	<LWIN> = 133;
};
xkb_types "quicklook" { include "complete" };
xkb_compat "quicklook" { include "complete" };
xkb_symbols "quicklook" {
	key <LCTL> { [ Control_L ] };
	key <AC03> { [ c, C ] };
	key <DOWN> { [ Down ] };
	key <SPCE> { [ space ] };
	key <RGHT> { [ Right ] };
	key <LWIN> { [ Super_L ] };
	modifier_map Control { <LCTL> };
	modifier_map Mod4 { <LWIN> };
};
};
"#;

const KEY_LEFTCTRL: u32 = 29;
const KEY_C: u32 = 46;
const KEY_LEFTMETA: u32 = 125;
/// Maska realnego modyfikatora Control w mapie wyżej (Shift=1, Lock=2,
/// Control=4 — kolejność z `include "complete"`).
const MOD_CONTROL: u32 = 4;
/// Stany klawisza jak w `wl_keyboard::key` (protokół podaje surowe liczby).
const PRESSED: u32 = 1;
const RELEASED: u32 = 0;

#[derive(Debug)]
pub enum GrabError {
    /// Na wierzchu nie ma menedżera plików — trzymamy ręce z dala od Ctrl+C.
    NotFileManager(String),
    /// Menedżer plików jest, ale nic nie zaznaczono (albo skopiował coś,
    /// co nie jest plikiem).
    NoSelection,
    Wayland(String),
}

impl std::fmt::Display for GrabError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFileManager(app) if app.is_empty() => {
                write!(f, "na wierzchu nie ma menedżera plików")
            }
            Self::NotFileManager(app) => write!(f, "na wierzchu jest „{app}”, nie menedżer plików"),
            Self::NoSelection => write!(f, "nie widzę zaznaczonego pliku"),
            Self::Wayland(e) => write!(f, "{e}"),
        }
    }
}

// =========================================================================
// Stan pojedynczego „zerknięcia” na kompozytor
// =========================================================================

#[derive(Default)]
struct Toplevel {
    app_id: String,
    activated: bool,
}

#[derive(Default)]
struct State {
    seat: Option<wl_seat::WlSeat>,
    vk_manager: Option<ZwpVirtualKeyboardManagerV1>,
    toplevels: HashMap<ObjectId, Toplevel>,
    /// Uchwyt COSMIC-owy → uchwyt `ext` (stany przychodzą tym pierwszym,
    /// a `app_id` tym drugim).
    cosmic_to_ext: HashMap<ObjectId, ObjectId>,
    info: Option<ZcosmicToplevelInfoV1>,
    /// Uchwyty, którym trzeba jeszcze dorobić odpowiednik COSMIC-owy.
    pending: Vec<ExtForeignToplevelHandleV1>,
}

impl State {
    fn active_app_id(&self) -> Option<String> {
        self.toplevels
            .values()
            .find(|t| t.activated)
            .map(|t| t.app_id.clone())
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "wl_seat" => {
                state.seat = Some(registry.bind(name, version.min(5), qh, ()));
            }
            "zwp_virtual_keyboard_manager_v1" => {
                state.vk_manager = Some(registry.bind(name, 1, qh, ()));
            }
            "ext_foreign_toplevel_list_v1" => {
                let _: ExtForeignToplevelListV1 = registry.bind(name, 1, qh, ());
            }
            "zcosmic_toplevel_info_v1" => {
                state.info = Some(registry.bind(name, version.min(3), qh, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            state.toplevels.insert(toplevel.id(), Toplevel::default());
            state.pending.push(toplevel);
        }
    }

    wayland_client::event_created_child!(State, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                if let Some(t) = state.toplevels.get_mut(&handle.id()) {
                    t.app_id = app_id;
                }
            }
            ext_foreign_toplevel_handle_v1::Event::Closed => {
                state.toplevels.remove(&handle.id());
            }
            _ => {}
        }
    }
}

impl Dispatch<ZcosmicToplevelInfoV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZcosmicToplevelInfoV1,
        _: zcosmic_toplevel_info_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    wayland_client::event_created_child!(State, ZcosmicToplevelInfoV1, [
        zcosmic_toplevel_info_v1::EVT_TOPLEVEL_OPCODE => (ZcosmicToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZcosmicToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ZcosmicToplevelHandleV1,
        event: zcosmic_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if std::env::var_os("QUICKLOOK_DEBUG").is_some() {
            eprintln!("  cosmic event: {event:?}");
        }
        let zcosmic_toplevel_handle_v1::Event::State { state: states } = event else {
            return;
        };
        if std::env::var_os("QUICKLOOK_DEBUG").is_some() {
            eprintln!("  state event: {states:?}");
        }
        let activated = states
            .chunks_exact(4)
            .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .any(|s| s == u32::from(CosmicState::Activated));
        let Some(ext_id) = state.cosmic_to_ext.get(&handle.id()).cloned() else {
            return;
        };
        if let Some(t) = state.toplevels.get_mut(&ext_id) {
            t.activated = activated;
        }
    }
}

macro_rules! ignore_events {
    ($($proxy:ty),* $(,)?) => {$(
        impl Dispatch<$proxy, ()> for State {
            fn event(
                _: &mut Self,
                _: &$proxy,
                _: <$proxy as Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }
    )*};
}
ignore_events!(
    wl_seat::WlSeat,
    ZwpVirtualKeyboardManagerV1,
    ZwpVirtualKeyboardV1
);

// =========================================================================
// Właściwa robota
// =========================================================================

/// Ścieżka pliku zaznaczonego w menedżerze plików na wierzchu.
///
/// Blokuje (rozmowa z kompozytorem + czekanie, aż menedżer odpowie na
/// Ctrl+C) — wołać z `spawn_blocking`, nie z pętli iced.
pub fn grab(file_managers: &[String], settle_ms: u64) -> Result<Vec<PathBuf>, GrabError> {
    let (_conn, mut queue, qh, mut state) = probe().map_err(GrabError::Wayland)?;

    let app_id = state.active_app_id().unwrap_or_default();
    if !is_file_manager(&app_id, file_managers) {
        return Err(GrabError::NotFileManager(app_id));
    }

    let saved = Clipboard::save();
    // Czyścimy PRZED Ctrl+C: gdyby menedżer nic nie skopiował (nic nie jest
    // zaznaczone), w schowku zostałby STARY `text/uri-list` z jakiegoś
    // wcześniejszego kopiowania i podglądalibyśmy nie ten plik.
    Clipboard::clear();

    // 4. Ctrl+C wirtualną klawiaturą.
    let (Some(seat), Some(manager)) = (state.seat.clone(), state.vk_manager.clone()) else {
        return Err(GrabError::Wayland(
            "kompozytor nie daje wirtualnej klawiatury".into(),
        ));
    };
    let keyboard = manager.create_virtual_keyboard(&seat, &qh, ());
    upload_keymap(&keyboard).map_err(|e| GrabError::Wayland(e.to_string()))?;
    let mut t = 0;
    let mut tick = || {
        t += 10;
        t
    };
    keyboard.modifiers(MOD_CONTROL, 0, 0, 0);
    keyboard.key(tick(), KEY_LEFTCTRL, PRESSED);
    keyboard.key(tick(), KEY_C, PRESSED);
    keyboard.key(tick(), KEY_C, RELEASED);
    keyboard.key(tick(), KEY_LEFTCTRL, RELEASED);
    keyboard.modifiers(0, 0, 0, 0);
    queue
        .roundtrip(&mut state)
        .map_err(|e| GrabError::Wayland(e.to_string()))?;

    // 5. menedżer musi zdążyć przejąć schowek.
    std::thread::sleep(Duration::from_millis(settle_ms));
    let uris = read_clipboard_type("text/uri-list");
    keyboard.destroy();
    let _ = queue.roundtrip(&mut state);

    // 6. schowek z powrotem taki, jaki był.
    saved.restore();

    let paths = parse_uri_list(&uris);
    if paths.is_empty() {
        return Err(GrabError::NoSelection);
    }
    Ok(paths)
}

/// Wciśnięcie dowolnego klawisza (kod evdev) — tylko do diagnostyki
/// (`--wcisnij`), żeby dało się przetestować całą drogę bez ręki na
/// klawiaturze.
pub fn tap(evdev: u32, mods: u32) -> Result<(), String> {
    let (_conn, mut queue, qh, mut state) = probe()?;
    let (Some(seat), Some(manager)) = (state.seat.clone(), state.vk_manager.clone()) else {
        return Err("brak wirtualnej klawiatury".into());
    };
    let keyboard = manager.create_virtual_keyboard(&seat, &qh, ());
    upload_keymap(&keyboard).map_err(|e| e.to_string())?;
    // Modyfikator wciskamy FIZYCZNIE (osobnym `key`), a nie tylko przez
    // `modifiers()`: kompozytor dopasowuje globalne skróty do własnego
    // stanu xkb, który aktualizują zdarzenia klawiszy — samo `modifiers()`
    // ustawia stan po stronie klienta i skrót się nie odpala.
    let mod_key = match mods {
        0 => None,
        m if m & 64 != 0 => Some(KEY_LEFTMETA),
        _ => Some(KEY_LEFTCTRL),
    };
    if let Some(k) = mod_key {
        keyboard.modifiers(mods, 0, 0, 0);
        keyboard.key(5, k, PRESSED);
    }
    keyboard.key(10, evdev, PRESSED);
    keyboard.key(20, evdev, RELEASED);
    if let Some(k) = mod_key {
        keyboard.key(25, k, RELEASED);
        keyboard.modifiers(0, 0, 0, 0);
    }
    queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(80));
    keyboard.destroy();
    let _ = queue.roundtrip(&mut state);
    Ok(())
}

/// Sam `app_id` okna na wierzchu — do diagnostyki (`--kto`).
pub fn active_app_id() -> Result<String, String> {
    let (_conn, _queue, _qh, state) = probe()?;
    if std::env::var_os("QUICKLOOK_DEBUG").is_some() {
        for t in state.toplevels.values() {
            eprintln!("  okno app_id={:?} aktywne={}", t.app_id, t.activated);
        }
    }
    Ok(state.active_app_id().unwrap_or_default())
}

/// Połączenie z kompozytorem + świeża lista okien (z `app_id` i tym, które
/// jest aktywne).
///
/// **Pułapka, przez którą to najpierw nie działało:** cosmic-comp odpowiada
/// na `get_cosmic_toplevel` dopiero w swojej następnej pętli, a nie w tym
/// samym `roundtrip`. Dwa obroty pod rząd wracają PUSTE — wygląda to jak
/// „kompozytor nie wspiera stanów”, a to tylko wyścig. Dlatego dopytujemy w
/// pętli, aż ktoś się zgłosi jako aktywny (albo minie `WAIT_MAX`).
fn probe() -> Result<
    (
        Connection,
        wayland_client::EventQueue<State>,
        QueueHandle<State>,
        State,
    ),
    String,
> {
    let conn = Connection::connect_to_env().map_err(|e| e.to_string())?;
    let mut queue = conn.new_event_queue::<State>();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());
    let mut state = State::default();

    // 1. globale, 2. lista okien z ich `app_id`.
    queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
    queue.roundtrip(&mut state).map_err(|e| e.to_string())?;

    // 3. do każdego okna dorabiamy uchwyt COSMIC-owy — tylko on mówi, które
    //    jest aktywne (`ext-foreign-toplevel-list` v1 tego nie ma).
    if let Some(info) = state.info.clone() {
        for ext in std::mem::take(&mut state.pending) {
            let cosmic = info.get_cosmic_toplevel(&ext, &qh, ());
            state.cosmic_to_ext.insert(cosmic.id(), ext.id());
        }
        const WAIT_MAX: Duration = Duration::from_millis(250);
        const STEP: Duration = Duration::from_millis(15);
        let deadline = std::time::Instant::now() + WAIT_MAX;
        loop {
            queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
            if state.active_app_id().is_some() || std::time::Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(STEP);
        }
    }
    Ok((conn, queue, qh, state))
}

/// Porównanie `app_id` z listą z configu — bez wielkości liter i bez
/// przedrostka odwróconej domeny, żeby `com.system76.CosmicFiles` pasowało
/// też do wpisu `cosmicfiles`.
fn is_file_manager(app_id: &str, list: &[String]) -> bool {
    if app_id.is_empty() {
        return false;
    }
    let short = |s: &str| s.rsplit('.').next().unwrap_or(s).to_lowercase();
    let a_full = app_id.to_lowercase();
    let a_short = short(app_id);
    list.iter().any(|w| {
        let w_full = w.to_lowercase();
        w_full == a_full || short(w) == a_short
    })
}

fn upload_keymap(keyboard: &ZwpVirtualKeyboardV1) -> std::io::Result<()> {
    let mut file = tempfile::tempfile()?;
    file.write_all(KEYMAP.as_bytes())?;
    // Kompozytor czyta mapę jak C-string — bez zera na końcu bywa „invalid
    // keymap”.
    file.write_all(&[0])?;
    file.flush()?;
    file.rewind()?;
    let size = KEYMAP.len() as u32 + 1;
    keyboard.keymap(
        u32::from(wayland_client::protocol::wl_keyboard::KeymapFormat::XkbV1),
        file.as_fd(),
        size,
    );
    Ok(())
}

// =========================================================================
// Schowek
// =========================================================================

/// Zapamiętana zawartość schowka (jeden typ — ten, który system podaje jako
/// pierwszy). To świadome uproszczenie: gdyby odtwarzać wszystkie typy
/// naraz, trzeba by własnego serwera schowka.
struct Clipboard {
    mime: Option<String>,
    data: Vec<u8>,
}

impl Clipboard {
    fn save() -> Self {
        let types = run(&["wl-paste", "--list-types"]).unwrap_or_default();
        let types: Vec<String> = String::from_utf8_lossy(&types)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        let mime = pick_mime(&types);
        let data = match &mime {
            Some(m) => run(&["wl-paste", "--no-newline", "--type", m]).unwrap_or_default(),
            None => Vec::new(),
        };
        Self { mime, data }
    }

    fn clear() {
        let _ = std::process::Command::new("wl-copy")
            .arg("--clear")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    fn restore(self) {
        match self.mime {
            Some(m) if !self.data.is_empty() => {
                if let Ok(mut child) = std::process::Command::new("wl-copy")
                    .args(["--type", &m])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(&self.data);
                    }
                    let _ = child.wait();
                }
            }
            // Schowek był pusty — niech taki zostanie.
            _ => {
                let _ = std::process::Command::new("wl-copy")
                    .arg("--clear")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }
    }
}

/// Który typ ze schowka warto zapamiętać i oddać z powrotem.
///
/// `wl-copy` potrafi wystawić tylko JEDEN typ naraz, więc odtworzenie
/// schowka jest z natury stratne. Wybieramy ten, który realnie da się
/// wkleić: tekst przed obrazem, a wewnętrzne tokeny przeglądarek
/// (`chromium/x-internal-...`, pierwsze na liście Chrome'a) na samym końcu —
/// inaczej użytkownik po podglądzie wkleja binarny śmieć zamiast skopiowanego
/// zdania.
fn pick_mime(types: &[String]) -> Option<String> {
    const PREFERRED: [&str; 6] = [
        "text/plain;charset=utf-8",
        "text/plain",
        "text/uri-list",
        "text/html",
        "image/png",
        "image/jpeg",
    ];
    for want in PREFERRED {
        if let Some(t) = types.iter().find(|t| t.eq_ignore_ascii_case(want)) {
            return Some(t.clone());
        }
    }
    types
        .iter()
        .find(|t| !t.contains("x-internal") && !t.starts_with("chromium/"))
        .or_else(|| types.first())
        .cloned()
}

fn read_clipboard_type(mime: &str) -> Vec<u8> {
    run(&["wl-paste", "--no-newline", "--type", mime]).unwrap_or_default()
}

fn run(cmd: &[&str]) -> Option<Vec<u8>> {
    let out = std::process::Command::new(cmd[0])
        .args(&cmd[1..])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// `text/uri-list` → ścieżki. Bierzemy tylko `file://` — podgląd czegoś,
/// czego nie ma na dysku, i tak by się nie udał.
pub fn parse_uri_list(bytes: &[u8]) -> Vec<PathBuf> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.strip_prefix("file://"))
        .map(percent_decode)
        .map(PathBuf::from)
        .collect()
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::{is_file_manager, parse_uri_list, percent_decode};

    #[test]
    fn czyta_uri_list_z_menedzera() {
        let paths = parse_uri_list(b"file:///home/user/a%20b.pdf\r\nfile:///tmp/x.png\r\n");
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].to_str().unwrap(), "/home/user/a b.pdf");
    }

    #[test]
    fn pomija_nieplikowe_uri() {
        assert!(parse_uri_list(b"https://example.com\n").is_empty());
    }

    #[test]
    fn dekoduje_polskie_znaki() {
        assert_eq!(percent_decode("/dom/za%C5%BC%C3%B3%C5%82%C4%87.md"), "/dom/zażółć.md");
    }

    #[test]
    fn woli_tekst_od_wewnetrznych_tokenow_przegladarki() {
        let types = vec![
            "chromium/x-internal-source-rfh-token".to_string(),
            "text/plain;charset=utf-8".to_string(),
            "text/plain".to_string(),
        ];
        assert_eq!(
            super::pick_mime(&types).as_deref(),
            Some("text/plain;charset=utf-8")
        );
        assert_eq!(super::pick_mime(&[]), None);
    }

    #[test]
    fn rozpoznaje_menedzer_po_koncowce_app_id() {
        let list = vec!["com.system76.CosmicFiles".to_string(), "dolphin".to_string()];
        assert!(is_file_manager("com.system76.CosmicFiles", &list));
        assert!(is_file_manager("org.kde.dolphin", &list));
        assert!(!is_file_manager("com.system76.CosmicTerm", &list));
        assert!(!is_file_manager("", &list));
    }
}
