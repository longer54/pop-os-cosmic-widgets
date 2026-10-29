//! Quick Look dla COSMIC — spacja pokazuje podgląd zaznaczonego pliku.
//!
//! Dwa tryby w jednym pliku wykonywalnym (jak `cosmic-widget-drawer`):
//! - bez argumentów — demon (autostart): nasłuchuje gniazda i tworzy
//!   powierzchnię dopiero na żądanie;
//! - `--toggle` / `--show` / `--hide` / ścieżka pliku — klient: mówi
//!   działającemu demonowi, co pokazać (to podpinamy pod skrót), a gdy
//!   demona nie ma, uruchamia go i od razu przekazuje polecenie.
//!
//! Dlaczego **Super+Spacja**, a nie goła spacja jak w macOS: globalny skrót
//! na samą spację zjadałby ją WSZĘDZIE — także przy zmianie nazwy pliku w
//! menedżerze. Wayland nie daje aplikacjom przechwytywania klawiszy „tylko
//! gdy okno X ma fokus", a przełączanie wpisu w configu skrótów przy każdej
//! zmianie fokusu to wyścig z `cosmic-settings`. Za to gdy podgląd JUŻ jest
//! na wierzchu, klawiatura należy do niego — i wtedy spacja zamyka, dokładnie
//! jak u Apple'a.

mod app;
mod config;
mod ipc;
mod preview;
mod selection;

fn main() -> cosmic::iced::Result {
    let config = config::Config::load();
    let arg = std::env::args().nth(1).unwrap_or_default();

    match arg.as_str() {
        "" => {}
        "--help" | "-h" => {
            eprintln!(
                "cosmic-quicklook [OPCJA | PLIK]\n\
                 \n\
                 bez argumentu   demon (autostart)\n\
                 --toggle        pokaż podgląd zaznaczonego pliku / schowaj\n\
                 --show/--hide   to samo, ale w jedną stronę\n\
                 PLIK            podgląd konkretnego pliku\n\
                 --kto           diagnostyka: app_id okna na wierzchu\n\
                 --zaznaczenie   diagnostyka: co widzi w menedżerze plików\n\
                 --wcisnij KOD   diagnostyka: wciśnij klawisz (kod evdev)\n\
                 \n\
                 config: {}",
                config::Config::path().display()
            );
            return Ok(());
        }
        "--kto" => {
            match selection::active_app_id() {
                Ok(a) if a.is_empty() => println!("nie widzę aktywnego okna"),
                Ok(a) => println!("aktywne okno: {a}"),
                Err(e) => println!("błąd: {e}"),
            }
            return Ok(());
        }
        "--wcisnij" => {
            // Diagnostyka: wciśnij klawisz o podanym kodzie evdev wirtualną
            // klawiaturą (108 = strzałka w dół). Tym testowałem całą drogę
            // „zaznacz plik → Ctrl+C → schowek” bez ręki na klawiaturze.
            let code: u32 = std::env::args()
                .nth(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(108);
            let mods: u32 = std::env::args()
                .nth(3)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            match selection::tap(code, mods) {
                Ok(()) => println!("wysłane: {code}"),
                Err(e) => println!("błąd: {e}"),
            }
            return Ok(());
        }
        "--zaznaczenie" => {
            match selection::grab(&config.file_managers, config.settle_ms) {
                Ok(paths) => {
                    for p in paths {
                        println!("{}", p.display());
                    }
                }
                Err(e) => println!("nic: {e}"),
            }
            return Ok(());
        }
        other => {
            let Some(cmd) = ipc::Command::from_arg(other) else {
                eprintln!("cosmic-quicklook: nie znam „{other}” i nie ma takiego pliku");
                std::process::exit(2);
            };
            if let Err(e) = ipc::send(&cmd) {
                eprintln!("cosmic-quicklook: {e}");
                std::process::exit(1);
            }
            return Ok(());
        }
    }

    // Demon. Gdy startuje go klient (`--toggle` przy nieżyjącym demonie),
    // klient sam ponawia wysyłkę, aż gniazdo zacznie nasłuchiwać — nie ma
    // po co przekazywać pierwszego polecenia inną drogą.
    let settings = cosmic::app::Settings::default()
        .no_main_window(true)
        .client_decorations(false)
        .transparent(true);
    cosmic::app::run::<app::QuickLook>(settings, config)
}
