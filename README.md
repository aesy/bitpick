# bitwarden-tui

Fast terminal picker for Bitwarden credentials using `bw` and `fzf`.

## Implementation notes

- The interface is built with `ratatui` + `crossterm`.
- Searching is delegated to `fzf --filter`.
- Clipboard uses `arboard` where available (Linux X11/Wayland, macOS, Windows), with a fallback to `wl-copy` / `xclip` / `xsel` / `pbcopy` / `clip.exe` when the platform prefers a shell tool.
- Vault access uses the standard `bw` CLI.

## What it does

- Loads login items from your Bitwarden vault
- Filters them in a full-screen TUI
- Copies the selected field to the clipboard (or prints it with `--print`)
- Supports passwords, usernames, TOTP codes, URIs, and notes
- Clears the clipboard automatically after a configurable delay

## Requirements

- `bw` in your `PATH`
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

Log in to Bitwarden once, then run:

```bash
bw login
bitwarden-tui --sync
```

### Keybindings in the picker

**Actions**

- `Enter`: copy password, or the field set by `--field`
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
- A cached Bitwarden session token is written to `$XDG_RUNTIME_DIR/bitwarden-tui/bw-session` (falling back to `$XDG_CACHE_HOME` then `~/.cache`) with mode `0600`. The session is refreshed transparently when it expires.

## Notes

- If `BW_SESSION` is not set, the app tries to unlock the Bitwarden vault.
- A cached session token is reused automatically and refreshed on demand — you should not need to type your master password on every launch.
- Use `--print` when clipboard access is unavailable.
- If a field such as notes or TOTP is missing, the TUI shows a status message instead of exiting.
