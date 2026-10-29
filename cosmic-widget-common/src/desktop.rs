//! Trzymanie widgetu na jednym pulpicie wirtualnym (KDE Plasma / KWin).
//!
//! Powierzchnia `zwlr_layer_shell_v1` nie zna pojęcia pulpitu wirtualnego —
//! należy do *ekranu*, więc KWin rysuje ją na wszystkich pulpitach naraz
//! i żadna reguła okien tego nie złapie (reguły działają tylko na zwykłe
//! okna). Jedyne wyjście: widget sam pyta KWin przez D-Bus, który pulpit
//! jest aktywny, i po zejściu ze swojego niszczy własną powierzchnię
//! (odtwarza ją po powrocie) — po stronie widgetu obsługuje to wiadomość
//! `DesktopVisible` w jego `app.rs`.
//!
//! Nie próbować zamiast tego `Layer::Background` (sprawdzone 2026-08-11):
//! wygląda jak rozwiązanie, ale klik w tapetę podnosi okno pulpitu
//! plasmashella nad całą tę warstwę i wszystkie widgety znikają z ekranu.
//!
//! Na kompozytorze bez `org.kde.KWin` (np. cosmic-comp na Pop!_OS)
//! subskrypcja po prostu nie znajduje usługi i widget zostaje widoczny —
//! zachowanie jak przed zmianą.

use std::time::Duration;

use cosmic::iced::futures::channel::mpsc::Sender;
use cosmic::iced::futures::{SinkExt, StreamExt};
use cosmic::iced::Subscription;

#[zbus::proxy(
    interface = "org.kde.KWin.VirtualDesktopManager",
    default_service = "org.kde.KWin",
    default_path = "/VirtualDesktopManager"
)]
trait VirtualDesktopManager {
    // KWin nazywa właściwości z małej litery; bez jawnego `name` zbus
    // wygenerowałby "Current"/"Desktops" i dostałby UnknownProperty.
    /// UUID aktywnego pulpitu.
    #[zbus(property, name = "current")]
    fn current(&self) -> zbus::Result<String>;

    /// Lista pulpitów: (pozycja liczona od 0, UUID, nazwa).
    #[zbus(property, name = "desktops")]
    fn desktops(&self) -> zbus::Result<Vec<(u32, String, String)>>;
}

/// Emituje `true`, gdy aktywny jest pulpit `desktop` (numerowany od 1 —
/// tak jak w ustawieniach Plasmy), `false` przy każdym innym.
pub fn subscription(desktop: u32) -> Subscription<bool> {
    // `run_with` bierze wskaźnik na funkcję, nie domknięcie — numer pulpitu
    // musi więc przyjechać przez `data`, a nie zostać przechwycony.
    Subscription::run_with(desktop, |desktop| {
        let desktop = *desktop;
        cosmic::iced::stream::channel(4, move |mut output| async move {
            // Bez KWin (cosmic-comp) każda próba kończy się od razu — stały
            // retry co 5 s budził widget i spamował journal (2026-10-06,
            // ~430 wpisów/h). Dlatego odstęp rośnie aż do 10 min, a wraca
            // do 5 s dopiero po połączeniu, które realnie pożyło.
            let mut delay = Duration::from_secs(5);
            let mut announced = false;
            loop {
                let started = std::time::Instant::now();
                if let Err(e) = watch(desktop, &mut output).await
                    && delay == Duration::from_secs(5)
                {
                    eprintln!("nasłuch pulpitów KWin przerwany: {e}");
                }
                // Połączenie, które coś zdążyło wysłać (np. `false` = schowaj),
                // wymaga ponownego „pokaż”; natychmiastowa porażka — nie.
                if started.elapsed() > Duration::from_secs(1) {
                    announced = false;
                }
                if started.elapsed() > Duration::from_secs(60) {
                    delay = Duration::from_secs(5);
                }
                // Zerwana sesja D-Bus / restart KWin — nie zostawiaj widgetu
                // schowanego na zawsze, pokaż go i spróbuj podłączyć się znów.
                if !announced {
                    let _ = output.send(true).await;
                    announced = true;
                }
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_secs(600));
            }
        })
    })
}

/// Jeden cykl życia połączenia: ustal UUID docelowego pulpitu, wyślij stan
/// początkowy, potem śledź zmiany aż do zerwania połączenia.
async fn watch(desktop: u32, output: &mut Sender<bool>) -> zbus::Result<()> {
    let conn = zbus::Connection::session().await?;
    let proxy = VirtualDesktopManagerProxy::new(&conn).await?;

    let target = proxy
        .desktops()
        .await?
        .into_iter()
        .find(|(position, _, _)| position + 1 == desktop)
        .map(|(_, uuid, _)| uuid);

    let Some(target) = target else {
        eprintln!(
            "nie ma pulpitu wirtualnego nr {desktop} — \
             widget zostaje widoczny na wszystkich"
        );
        let _ = output.send(true).await;
        // Nie kręć pętli ponownych prób w kółko — czekaj bez końca.
        std::future::pending::<()>().await;
        return Ok(());
    };

    let mut changes = proxy.receive_current_changed().await;
    let _ = output.send(proxy.current().await? == target).await;

    while let Some(change) = changes.next().await {
        if let Ok(current) = change.get().await {
            let _ = output.send(current == target).await;
        }
    }

    Ok(())
}
