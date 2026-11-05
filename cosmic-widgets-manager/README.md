# cosmic-widgets-manager

A small GUI for the widgets in this repo — no more editing TOML by hand.

- List of all widgets with **Start / Stop** and an **Autostart** toggle (writes `~/.config/autostart/<widget>.desktop`).
- **Obsidian vault picker**: choose your vault folder once (system file chooser via XDG portal). The folder is validated (`.obsidian/` must exist) and `vault_path` / `vault_name` are written into the configs of every vault-based widget (calendar, tasks, note, drawer). Comments and other keys in those files are kept.
- Widgets are looked up in `~/.local/bin`, then in `PATH`.

## Build

```sh
cd cosmic-widgets-manager
cargo build --release
cp target/release/cosmic-widgets-manager ~/.local/bin/
```

Restart running widgets after changing the vault so they pick up the new path.

Status: early version. UI strings are in Polish.
