//! cosmic-okno — pokaz/schowaj/zamknij okna w COSMIC ze skrotow klawiszowych.
//!
//!   cosmic-okno lista
//!   cosmic-okno przelacz <app_id> [polecenie startowe]
//!       okno aktywne -> minimalizuj; jest, ale schowane -> pokaz;
//!       nie ma -> uruchom polecenie
//!   cosmic-okno zamknij-lub-schowaj <app_id>[,<app_id>...]
//!       aktywne okno z listy -> minimalizuj (dziala dalej w tle),
//!       kazde inne aktywne okno -> zamknij (jak zwykle Super+Q)
//!   cosmic-okno schowaj-gdy-sie-pojawi <app_id> [sekundy]
//!       czeka na okno i je minimalizuje (autostart "w tle")

use cosmic_protocols::toplevel_info::v1::client::{zcosmic_toplevel_handle_v1, zcosmic_toplevel_info_v1};
use cosmic_protocols::toplevel_management::v1::client::zcosmic_toplevel_manager_v1;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{event_created_child, Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1 as ext_handle, ext_foreign_toplevel_list_v1 as ext_list,
};

const MINIMIZED: u32 = 1;
const ACTIVATED: u32 = 2;

struct Okno {
    ext: ext_handle::ExtForeignToplevelHandleV1,
    cosmic: Option<zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1>,
    app_id: String,
    title: String,
    stany: Vec<u32>,
    ma_stan: bool,
    zamkniete: bool,
}

impl Okno {
    fn aktywne(&self) -> bool {
        self.stany.contains(&ACTIVATED) && !self.stany.contains(&MINIMIZED)
    }
}

#[derive(Default)]
struct Stan {
    okna: Vec<Okno>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Stan {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_seat::WlSeat, ()> for Stan {
    fn event(_: &mut Self, _: &wl_seat::WlSeat, _: wl_seat::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ext_list::ExtForeignToplevelListV1, ()> for Stan {
    fn event(stan: &mut Self, _: &ext_list::ExtForeignToplevelListV1, ev: ext_list::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let ext_list::Event::Toplevel { toplevel } = ev {
            stan.okna.push(Okno { ext: toplevel, cosmic: None, app_id: String::new(), title: String::new(), stany: vec![], ma_stan: false, zamkniete: false });
        }
    }
    event_created_child!(Stan, ext_list::ExtForeignToplevelListV1, [
        ext_list::EVT_TOPLEVEL_OPCODE => (ext_handle::ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ext_handle::ExtForeignToplevelHandleV1, ()> for Stan {
    fn event(stan: &mut Self, h: &ext_handle::ExtForeignToplevelHandleV1, ev: ext_handle::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        let Some(o) = stan.okna.iter_mut().find(|o| &o.ext == h) else { return };
        match ev {
            ext_handle::Event::AppId { app_id } => o.app_id = app_id,
            ext_handle::Event::Title { title } => o.title = title,
            ext_handle::Event::Closed => o.zamkniete = true,
            _ => {}
        }
    }
}

impl Dispatch<zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, ()> for Stan {
    fn event(_: &mut Self, _: &zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1, _: zcosmic_toplevel_info_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1, ()> for Stan {
    fn event(stan: &mut Self, h: &zcosmic_toplevel_handle_v1::ZcosmicToplevelHandleV1, ev: zcosmic_toplevel_handle_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let zcosmic_toplevel_handle_v1::Event::State { state } = ev {
            if let Some(o) = stan.okna.iter_mut().find(|o| o.cosmic.as_ref() == Some(h)) {
                o.ma_stan = true;
                o.stany = state.chunks_exact(4).map(|b| u32::from_ne_bytes([b[0], b[1], b[2], b[3]])).collect();
            }
        }
    }
}

impl Dispatch<zcosmic_toplevel_manager_v1::ZcosmicToplevelManagerV1, ()> for Stan {
    fn event(_: &mut Self, _: &zcosmic_toplevel_manager_v1::ZcosmicToplevelManagerV1, _: zcosmic_toplevel_manager_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

struct Sesja {
    conn: Connection,
    kolejka: wayland_client::EventQueue<Stan>,
    stan: Stan,
    manager: zcosmic_toplevel_manager_v1::ZcosmicToplevelManagerV1,
    seat: wl_seat::WlSeat,
}

fn polacz() -> Result<Sesja, String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("brak Waylanda: {e}"))?;
    let (globals, mut kolejka) = registry_queue_init::<Stan>(&conn).map_err(|e| e.to_string())?;
    let qh = kolejka.handle();
    let _lista: ext_list::ExtForeignToplevelListV1 = globals.bind(&qh, 1..=1, ()).map_err(|e| format!("ext_foreign_toplevel_list: {e}"))?;
    let info: zcosmic_toplevel_info_v1::ZcosmicToplevelInfoV1 = globals.bind(&qh, 2..=3, ()).map_err(|e| format!("zcosmic_toplevel_info: {e}"))?;
    let manager = globals.bind(&qh, 1..=4, ()).map_err(|e| format!("zcosmic_toplevel_manager: {e}"))?;
    let seat = globals.bind(&qh, 1..=1, ()).map_err(|e| format!("wl_seat: {e}"))?;
    let mut stan = Stan::default();
    kolejka.roundtrip(&mut stan).map_err(|e| e.to_string())?;
    for o in stan.okna.iter_mut() {
        o.cosmic = Some(info.get_cosmic_toplevel(&o.ext, &qh, ()));
    }
    // cosmic-comp wysyla stany dopiero przy najblizszym odswiezeniu, nie od razu
    for _ in 0..50 {
        kolejka.roundtrip(&mut stan).map_err(|e| e.to_string())?;
        if stan.okna.iter().all(|o| o.ma_stan || o.zamkniete) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    stan.okna.retain(|o| !o.zamkniete);
    Ok(Sesja { conn, kolejka, stan, manager, seat })
}

impl Sesja {
    fn znajdz(&self, app_ids: &[&str]) -> Option<&Okno> {
        // najpierw aktywne, potem dowolne pasujace
        let pasuje = |o: &&Okno| app_ids.iter().any(|a| o.app_id.eq_ignore_ascii_case(a));
        self.stan.okna.iter().filter(pasuje).find(|o| o.aktywne()).or_else(|| self.stan.okna.iter().find(pasuje))
    }
    fn aktywne(&self) -> Option<&Okno> {
        self.stan.okna.iter().find(|o| o.aktywne())
    }
    fn wyslij(mut self) {
        let _ = self.conn.flush();
        let _ = self.kolejka.roundtrip(&mut self.stan);
    }
}

fn uruchom(polecenie: &str) {
    // Brave startuje kilka sekund; drugie wcisniecie skrotu w tym czasie
    // nie moze odpalic drugiego okna
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let znacznik = std::path::Path::new(&dir).join("cosmic-okno-start");
    if let Ok(t) = std::fs::metadata(&znacznik).and_then(|m| m.modified()) {
        if t.elapsed().map(|e| e.as_secs() < 10).unwrap_or(false) {
            return;
        }
    }
    let _ = std::fs::write(&znacznik, polecenie);
    let _ = std::process::Command::new("setsid").args(["-f", "sh", "-c", polecenie]).spawn();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let kod = match args.first().map(String::as_str) {
        Some("lista") => polacz().map(|s| {
            for o in &s.stan.okna {
                println!("{}\t{:?}\t{}", o.app_id, o.stany, o.title);
            }
        }),
        Some("przelacz") if args.len() >= 2 => polacz().map(|s| {
            let cel = [args[1].as_str()];
            match s.znajdz(&cel) {
                Some(o) if o.aktywne() => {
                    s.manager.set_minimized(o.cosmic.as_ref().unwrap());
                    s.wyslij();
                }
                Some(o) => {
                    let h = o.cosmic.as_ref().unwrap();
                    s.manager.unset_minimized(h);
                    s.manager.activate(h, &s.seat);
                    s.wyslij();
                }
                None => {
                    if let Some(p) = args.get(2) {
                        uruchom(p);
                    }
                }
            }
        }),
        Some("zamknij-lub-schowaj") if args.len() >= 2 => polacz().map(|s| {
            let cele: Vec<&str> = args[1].split(',').collect();
            if let Some(o) = s.aktywne() {
                let h = o.cosmic.as_ref().unwrap();
                if cele.iter().any(|a| o.app_id.eq_ignore_ascii_case(a)) {
                    s.manager.set_minimized(h);
                } else {
                    s.manager.close(h);
                }
                s.wyslij();
            }
        }),
        Some("schowaj-gdy-sie-pojawi") if args.len() >= 2 => {
            let limit: u64 = args.get(2).and_then(|x| x.parse().ok()).unwrap_or(60);
            let start = std::time::Instant::now();
            loop {
                match polacz() {
                    Ok(s) => {
                        if let Some(o) = s.znajdz(&[args[1].as_str()]) {
                            // chwila, zeby okno zdazylo sie w pelni zmapowac
                            std::thread::sleep(std::time::Duration::from_millis(1500));
                            let _ = o;
                            drop(s);
                            if let Ok(s) = polacz() {
                                if let Some(o) = s.znajdz(&[args[1].as_str()]) {
                                    s.manager.set_minimized(o.cosmic.as_ref().unwrap());
                                    s.wyslij();
                                }
                            }
                            break Ok(());
                        }
                    }
                    Err(e) => eprintln!("{e}"),
                }
                if start.elapsed().as_secs() >= limit {
                    break Err("okno sie nie pojawilo".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
        _ => Err("uzycie: lista | przelacz <app_id> [polecenie] | zamknij-lub-schowaj <app_id,...> | schowaj-gdy-sie-pojawi <app_id> [s]".into()),
    };
    if let Err(e) = kod {
        eprintln!("cosmic-okno: {e}");
        std::process::exit(1);
    }
}
