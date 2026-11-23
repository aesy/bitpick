# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0]

### Added

- `bitpick` CLI: a full-screen terminal picker for the logins and secure notes in a Bitwarden vault, read through `rbw`
  and filtered with `fzf`.
- Copies a login's password, username, TOTP code, URI or notes, or a secure note's text, to the clipboard, or prints it
  with `--print`.
- Switching between `rbw` profiles with `Tab` when `--profile` is passed more than once.
- Adding logins with `Alt-N` and editing the selected entry with `Alt-E`.
- Unlocking a locked vault in the terminal, and syncing with `--sync` or `Ctrl-R`.

[Unreleased]: https://github.com/aesy/bitpick/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/aesy/bitpick/releases/tag/v0.1.0
