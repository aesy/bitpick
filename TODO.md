* Ctrl-L does not work
* ESC / Ctrl-C should not cause error
* Start is a bit slow
* The UI is not accurate when using --field option
* --print option does not work, and does not update UI descriptions

## Bugs / correctness

* Header shortcut chip always renders "Enter password" regardless of --field (tui.rs:391) — sub-issue of #4
* Enter with zero filtered results exits with error instead of showing a status (tui.rs:325)
* Likely cause of the Ctrl-L bug: some terminals emit form-feed (\x0C) as a plain char, not Char('l') + CONTROL — match both
* Dead AppError variants: SelectionCancelled and SelectedItemMissing are never constructed (error.rs:21, 27)
* Dead branch: "No items loaded" in tui.rs:504-506 is unreachable — app.rs:67-69 errors out earlier on empty vaults

## Startup performance (breakdown of #3)

* Session validation probe runs `bw list items --search ...` on every launch (bw.rs:198-203) — consider `bw status` or a session-age heuristic
* No local cache of vault items — `bw list items` runs every launch; a short-TTL cache keyed by vault revision would make repeat launches near-instant

## UX

* Search field has no cursor / no readline editing (Ctrl-W word delete, Ctrl-A/E, arrow-key cursor)

## Security

* No clipboard auto-clear timer — standard in `keepassxc`, `bw`, `pass`; would slot in after Clipboard::copy in app.rs:101

## Docs / project hygiene

* README claims clipboard uses shell tools, but code prefers arboard on non-Linux (clipboard.rs:44-53)
* Cargo.toml missing description, license, repository, keywords — needed for `cargo publish`
* No LICENSE file, no CI, no `cargo install` docs in README
* .idea/ and bitwarden-tui.iml tracked in git — usually gitignored
