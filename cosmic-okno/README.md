# cosmic-okno — show/hide windows with a shortcut

🇵🇱 [Polska wersja](README.pl.md)

A small CLI tool for keyboard shortcuts: toggles an application's window (show/minimise/launch) and hides windows from a list instead of closing them.

## Features
- `cosmic-okno lista` — list windows
- `cosmic-okno przelacz <app_id> [command]` — toggle a window (launch `command` if it isn't running)
- `cosmic-okno zamknij-lub-schowaj <app_id>[,…]` — close the window, or hide it if its app is on the list
- `cosmic-okno schowaj-gdy-sie-pojawi <app_id> [seconds]` — hide the window as soon as it appears

## Requirements
COSMIC (`cosmic-toplevel-*` protocols), Rust.

## Building
```sh
cd cosmic-okno
cargo build --release
cp target/release/cosmic-okno ~/.local/bin/
```

## Configuration
No config file — bind the commands to shortcuts in COSMIC Settings.

---

**Disclaimer:** Private hobby project, built for learning and personal use. Not affiliated with System76 or the COSMIC project. Built with open-source tools and public data sources. Provided “as is”, without warranty of any kind.
