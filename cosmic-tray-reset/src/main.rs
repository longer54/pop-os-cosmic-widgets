//! Minimalna ikonka w zasobniku systemowym (StatusNotifierItem) — kliknięcie
//! restartuje autostartowe usługi systemd widgetów pulpitu COSMIC (kalendarz,
//! zadania [zawiera też sekcję "Do zapamiętania"], agenda, panel statusu,
//! pogoda, kursy, notatka), bo czasem tracą łączność z portalem
//! `xdg-desktop-portal-cosmic` i przestają reagować na kliknięcia (notatka
//! po autostarcie potrafi też nie przyjmować pisania — restart to czyści).

use ksni::menu::StandardItem;
use ksni::{Icon, MenuItem, ToolTip, Tray, TrayMethods};

const UNITS: &[&str] = &[
    r"app-cosmic\x2dcalendar\x2dwidget@autostart.service",
    r"app-cosmic\x2dwidget\x2dtasks@autostart.service",
    r"app-cosmic\x2dwidget\x2dagenda@autostart.service",
    r"app-cosmic\x2dwidget\x2dpanel@autostart.service",
    r"app-cosmic\x2dwidget\x2dweather@autostart.service",
    r"app-cosmic\x2dwidget\x2dmarkets@autostart.service",
    r"app-cosmic\x2dwidget\x2dnote@autostart.service",
];

fn reset_widgets() {
    // Najpierw ubij WSZYSTKIE instancje, także odpalone ręcznie poza
    // systemd — restart samych unitów ich nie dotyka i widgety się
    // dublowały (objaw: drugi komplet okien / nakładający się tekst).
    if let Ok(home) = std::env::var("HOME") {
        let pattern = format!(
            "^{home}/.local/bin/cosmic-(widget-(tasks|agenda|panel|weather|markets|note)|calendar-widget)$"
        );
        let _ = std::process::Command::new("pkill")
            .arg("-f")
            .arg(pattern)
            .status();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    // Restart jednostek pojedynczo z odstępem, a nie wszystkich naraz —
    // 6 warstwowych okien (layer-shell) otwierających/zamykających się w
    // tej samej klatce potrafiło wywalić cosmic-comp (błąd protokołu
    // cosmic_corner_radius_toplevel_v1 w aplecie listy okien, 2026-07-24),
    // co gasiło górny pasek i dock.
    for unit in UNITS {
        let status = std::process::Command::new("systemctl")
            .arg("--user")
            .arg("restart")
            .arg(unit)
            .status();
        if let Err(e) = status {
            eprintln!("cosmic-tray-reset: nie mogę uruchomić systemctl dla {unit}: {e}");
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
}

struct ResetTray;

impl Tray for ResetTray {
    fn id(&self) -> String {
        "cosmic-tray-reset".into()
    }

    fn icon_name(&self) -> String {
        "view-refresh-symbolic".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        Vec::new()
    }

    fn title(&self) -> String {
        "Reset widgetów".into()
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Reset widgetów COSMIC".into(),
            description:
                "Kliknij, aby zrestartować widgety pulpitu (kalendarz, zadania, agenda, panel, pogoda, kursy, notatka)"
                    .into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        reset_widgets();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![StandardItem {
            label: "Zresetuj widgety".into(),
            activate: Box::new(|_| reset_widgets()),
            ..Default::default()
        }
        .into()]
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Przy autostarcie po zalogowaniu StatusNotifierWatcher (zasobnik) bywa
    // jeszcze niegotowy na DBus — ponawiamy zamiast panikować od razu.
    let handle = loop {
        match ResetTray.spawn().await {
            Ok(h) => break h,
            Err(e) => {
                eprintln!("cosmic-tray-reset: rejestracja ikony nieudana ({e}), ponawiam za 2s");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
    };
    std::future::pending::<()>().await;
    drop(handle);
}
