# bitwarden-tui

Fast terminal picker for Bitwarden credentials using `rbw` and `fzf`.

## Implementation notes

- The interface is built with `ratatui` + `crossterm`.
- Searching is delegated to `fzf --filter`.
- Clipboard on Linux/BSD uses `wl-copy` / `xclip` / `xsel` first and falls back to `arboard`, which then keeps a small background process alive to serve the copied value. On macOS and Windows `arboard` comes first, with `pbcopy` / `clip.exe` as fallback.
- Vault access uses [`rbw`](https://github.com/doy/rbw), an unofficial Bitwarden client whose background agent keeps the vault unlocked, so loading takes milliseconds instead of seconds with the official `bw` CLI. Secrets are only fetched for the item you pick.

## What it does

- Loads login items from your Bitwarden vault
- Filters them in a full-screen TUI
- Copies the selected field to the clipboard (or prints it with `--print`)
- Supports passwords, usernames, TOTP codes, URIs, and notes
- Clears the clipboard automatically after a configurable delay

## Requirements

- `rbw` in your `PATH` (e.g. `pacman -S rbw`), configured and logged in
- `fzf` in your `PATH`
- A clipboard backend (any of: `arboard` builds in support for X11/Wayland/macOS/Windows; alternatively one of `wl-copy`, `xclip`, `xsel`, `pbcopy`, `clip.exe` on `PATH`). Not needed if you only use `--print`.

## Install

```bash
cargo install --path .
```

Or from a checkout, run without installing:

```bash
cargo run --release -- --sync
```

## Usage

Set up `rbw` once, then run:

```bash
rbw config set email you@example.com
rbw login
bitwarden-tui
```

If the vault is locked at startup, `rbw unlock` runs first and asks for the master password through the configured `pinentry` (`rbw config set pinentry pinentry-gnome3`, `pinentry-curses`, ...). How long the vault stays unlocked is set with `rbw config set lock_timeout <SECS>`. If it locks while the picker is open, the picker steps aside for the prompt and comes back afterwards.

### Keybindings in the picker

**Actions**

- `Enter`: copy password, or the field set by `--field` (the Ctrl shortcuts below still work)
- `Ctrl-P`: copy password
- `Ctrl-U`: copy username
- `Ctrl-T`: copy TOTP
- `Ctrl-O`: copy URI
- `Ctrl-N`: copy notes

**Search field editing**

- `Left` / `Right`: move cursor
- `Ctrl-A` / `Ctrl-E`: cursor to start / end of the query
- `Backspace`: delete the character before the cursor
- `Delete`: delete the character at the cursor
- `Ctrl-W`: delete the word before the cursor
- `Ctrl-K`: delete from cursor to end of the query
- `Ctrl-L`: clear the whole query

**Result navigation**

- `Up` / `Down`: move through results
- `PageUp` / `PageDown`: jump 10 results
- `Home` / `End`: first / last result

**Exit**

- `Esc` / `Ctrl-C`: quit

The picker draws on the terminal (`/dev/tty`), so `--print` output can be piped or captured, e.g. `pass=$(bitwarden-tui --print)`.

### Helpful flags

```bash
bitwarden-tui --query github
bitwarden-tui --field username
bitwarden-tui --field totp --print
bitwarden-tui --sync --print
bitwarden-tui --clipboard-clear 30
bitwarden-tui --clipboard-clear 0   # disable auto-clear
```

## Security

- The clipboard is cleared automatically 45 seconds after a copy, unless another program has overwritten it in the meantime. Adjust with `--clipboard-clear <SECS>` or disable with `--clipboard-clear 0`.
- No session token or vault data is written to disk by this tool; keys stay in the `rbw` agent.
- Passwords, notes and TOTP secrets are not loaded for the list; they are fetched only for the selected item.

## Notes

- Use `--print` when clipboard access is unavailable.
- If a field such as notes or TOTP is missing, the TUI shows a status message instead of exiting.
