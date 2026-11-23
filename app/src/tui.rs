use std::fmt::Display;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use thiserror::Error;
use which::which;

use crate::app::CopyField;
use crate::form::{Form, FormOutcome, FormRequest};
use crate::rbw::{ItemKind, Rbw, RbwError, VaultItem};
use crate::text_input::TextInput;

#[derive(Debug, Error)]
pub enum TuiError {
    #[error("Could not find `fzf` in PATH")]
    MissingFzf,

    #[error("The selected item no longer exists")]
    SelectedItemMissing,

    #[error("Could not parse item index from fzf output")]
    FzfParse,

    #[error("Failed to run `{command}`")]
    SpawnCommand {
        command: String,
        #[source]
        source: io::Error,
    },

    #[error("{command} failed: {stderr}")]
    CommandFailed { command: String, stderr: String },

    #[error("{command} failed ({status})")]
    CommandFailedStatus { command: String, status: String },

    #[error("{command} returned non-UTF8 output")]
    CommandUtf8 {
        command: String,
        #[source]
        source: std::string::FromUtf8Error,
    },

    #[error("Terminal I/O error")]
    Io(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, TuiError>;

const FZF_COMMAND: &str = "fzf --filter";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionVerb {
    Copy,
    Print,
}

impl ActionVerb {
    const fn descriptive(self) -> &'static str {
        match self {
            Self::Copy => "copies",
            Self::Print => "prints",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PickerConfig {
    pub initial_query: String,
    pub fixed_action: Option<CopyField>,
    pub initial_status: Option<String>,
    pub verb: ActionVerb,
    /// Sync each profile before its items are first loaded.
    pub sync: bool,
}

pub enum PickerOutcome {
    Selected { value: String },
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingAction {
    index: usize,
    action: CopyField,
}

enum KeyOutcome {
    Continue,
    Cancel,
    Confirm(PendingAction),
    SwitchProfile(isize),
    Reload,
    Edit(usize),
    SaveForm,
}

/// Loads `vaults[0]` right away; the other profiles are loaded on first switch.
pub fn run_picker<F, E>(
    vaults: &[Rbw],
    fzf: FzfFilter,
    config: PickerConfig,
    mut fetch: F,
) -> Result<PickerOutcome>
where
    F: FnMut(&Rbw, &VaultItem, CopyField) -> std::result::Result<String, E>,
    E: Display + From<RbwError>,
{
    let sync = config.sync;
    let initial_status = config.initial_status.clone();
    let profile_names = vaults
        .iter()
        .map(|vault| vault.profile_name().to_owned())
        .collect();
    let mut terminal = TerminalGuard::enter()?;
    let mut app = PickerApp::new(profile_names, fzf, config);

    if load_profile(&mut terminal, &mut app, vaults, 0, sync)?
        && let Some(status) = initial_status
    {
        app.set_error(status);
    }

    loop {
        terminal.draw(|frame| app.draw(frame))?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };

        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            continue;
        }

        match app.handle_key(key)? {
            KeyOutcome::Continue => {},
            KeyOutcome::Cancel => return Ok(PickerOutcome::Cancelled),
            KeyOutcome::Confirm(pending) => {
                let vault = &vaults[app.active];
                let Some(item) = app.items().get(pending.index).cloned() else {
                    app.set_error(TuiError::SelectedItemMissing.to_string());
                    continue;
                };

                match with_unlocked_terminal(&mut terminal, vault, || {
                    fetch(vault, &item, pending.action)
                })? {
                    Ok(value) => {
                        return Ok(PickerOutcome::Selected { value });
                    },
                    Err(error) => app.set_error(error.to_string()),
                }
            },
            KeyOutcome::SwitchProfile(delta) => {
                let next = (app.active.cast_signed() + delta)
                    .rem_euclid(vaults.len().cast_signed())
                    .cast_unsigned();
                if app.is_loaded(next) {
                    app.activate(next)?;
                } else {
                    load_profile(&mut terminal, &mut app, vaults, next, sync)?;
                }
            },
            KeyOutcome::Reload => {
                let active = app.active;
                load_profile(&mut terminal, &mut app, vaults, active, true)?;
            },
            KeyOutcome::Edit(index) => {
                let vault = &vaults[app.active];
                let Some(item) = app.items().get(index).cloned() else {
                    app.set_error(TuiError::SelectedItemMissing.to_string());
                    continue;
                };

                match with_unlocked_terminal(&mut terminal, vault, || vault.get_entry(&item.id))? {
                    Ok(entry) => app.form = Some(Form::edit(&item, &entry)),
                    Err(error) => app.set_error(error.to_string()),
                }
            },
            KeyOutcome::SaveForm => {
                let vault = &vaults[app.active];
                save_form(&mut terminal, &mut app, vault)?;
            },
        }
    }
}

/// Saves the open form through rbw and reloads the active profile on success.
/// Failures stay in the form so nothing typed is lost.
fn save_form(terminal: &mut TerminalGuard, app: &mut PickerApp, vault: &Rbw) -> Result<()> {
    let Some(form) = app.form.as_mut() else {
        return Ok(());
    };
    let Some(request) = form.request() else {
        form.set_error("A name is required".to_owned());
        return Ok(());
    };

    form.set_info("Saving...".to_owned());
    terminal.draw(|frame| app.draw(frame))?;

    let saved = with_unlocked_terminal(terminal, vault, || match &request {
        FormRequest::Add(login) => vault.add(login),
        FormRequest::EditLogin {
            id,
            password,
            notes,
        } => vault.edit_login(id, password, notes),
        FormRequest::EditNote { id, notes } => vault.edit_note(id, notes),
    })?;

    if let Err(error) = saved {
        if let Some(form) = app.form.as_mut() {
            form.set_error(error.to_string());
        }
        return Ok(());
    }

    app.form = None;
    let active = app.active;
    match vault.load(false) {
        Ok(items) => {
            app.store(active, items);
            app.activate(active)?;
            app.set_status("Saved".to_owned());
        },
        Err(error) => app.set_error(error.to_string()),
    }

    Ok(())
}

pub struct FzfFilter {
    program: PathBuf,
}

impl FzfFilter {
    pub fn new() -> Result<Self> {
        let program = which("fzf").map_err(|_| TuiError::MissingFzf)?;

        Ok(Self { program })
    }

    fn filter(&self, items: &[VaultItem], query: &str) -> Result<Vec<usize>> {
        let query = query.trim();

        if query.is_empty() {
            return Ok((0..items.len()).collect());
        }

        let input = items
            .iter()
            .enumerate()
            .map(|(index, item)| format!("{index}\t{}", searchable_text(item)))
            .collect::<Vec<_>>()
            .join("\n");

        let mut child = Command::new(&self.program)
            .arg("--filter")
            .arg(query)
            .arg("--delimiter")
            .arg("\t")
            .arg("--nth")
            .arg("2..")
            .env_remove("FZF_DEFAULT_OPTS")
            .env_remove("FZF_DEFAULT_OPTS_FILE")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| TuiError::SpawnCommand {
                command: FZF_COMMAND.to_owned(),
                source,
            })?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(input.as_bytes())?;
        }

        let output = child.wait_with_output()?;
        if !output.status.success() {
            if output.status.code() == Some(1) {
                return Ok(Vec::new());
            }

            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            if stderr.is_empty() {
                return Err(TuiError::CommandFailedStatus {
                    command: FZF_COMMAND.to_owned(),
                    status: output.status.to_string(),
                });
            }
            return Err(TuiError::CommandFailed {
                command: FZF_COMMAND.to_owned(),
                stderr,
            });
        }

        String::from_utf8(output.stdout)
            .map_err(|source| TuiError::CommandUtf8 {
                command: FZF_COMMAND.to_owned(),
                source,
            })?
            .lines()
            .map(|line| {
                line.split_once('\t')
                    .ok_or(TuiError::FzfParse)?
                    .0
                    .parse::<usize>()
                    .map_err(|_| TuiError::FzfParse)
            })
            .collect()
    }
}

// The UI is drawn on the terminal itself so stdout stays free for `--print` in
// pipes.
#[cfg(unix)]
type TerminalOutput = std::fs::File;
#[cfg(not(unix))]
type TerminalOutput = io::Stderr;

#[cfg(unix)]
fn open_terminal_output() -> io::Result<TerminalOutput> {
    std::fs::OpenOptions::new().write(true).open("/dev/tty")
}

#[cfg(not(unix))]
fn open_terminal_output() -> io::Result<TerminalOutput> {
    Ok(io::stderr())
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<TerminalOutput>>,
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        let mut terminal = Terminal::new(CrosstermBackend::new(open_terminal_output()?))?;
        enable_raw_mode()?;

        if let Err(error) = execute!(terminal.backend_mut(), EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }

        Ok(Self { terminal })
    }

    fn draw<F>(&mut self, render: F) -> Result<()>
    where
        F: FnOnce(&mut ratatui::Frame<'_>),
    {
        self.terminal.draw(render)?;
        Ok(())
    }

    fn suspend(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        enable_raw_mode()?;
        execute!(self.terminal.backend_mut(), EnterAlternateScreen)?;
        self.terminal.clear()?;
        Ok(())
    }
}

/// Loads `vaults[index]` and makes it active. Failures are shown in the status
/// line and leave the current profile active; returns whether loading
/// succeeded.
fn load_profile(
    terminal: &mut TerminalGuard,
    app: &mut PickerApp,
    vaults: &[Rbw],
    index: usize,
    sync: bool,
) -> Result<bool> {
    let vault = &vaults[index];
    app.set_status(format!("Loading profile {}...", vault.profile_name()));
    terminal.draw(|frame| app.draw(frame))?;

    match with_unlocked_terminal(terminal, vault, || vault.load(sync))? {
        Ok(items) => {
            app.store(index, items);
            app.activate(index)?;
            Ok(true)
        },
        Err(error) => {
            app.set_error(error.to_string());
            Ok(false)
        },
    }
}

/// Runs `op` against `vault`. A locked vault is unlocked first with the
/// terminal handed back, since pinentry prompts on it.
fn with_unlocked_terminal<T, E>(
    terminal: &mut TerminalGuard,
    vault: &Rbw,
    op: impl FnOnce() -> std::result::Result<T, E>,
) -> Result<std::result::Result<T, E>>
where
    E: From<RbwError>,
{
    if !vault.is_unlocked() {
        terminal.suspend()?;
        let unlocked = vault.ensure_unlocked();
        terminal.resume()?;

        if let Err(error) = unlocked {
            return Ok(Err(error.into()));
        }
    }

    Ok(op())
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

struct PickerApp {
    profile_names: Vec<String>,
    loaded: Vec<Option<Vec<VaultItem>>>,
    active: usize,
    input: TextInput,
    fixed_action: Option<CopyField>,
    verb: ActionVerb,
    fzf: FzfFilter,
    filtered: Vec<usize>,
    selected: usize,
    list_state: ListState,
    status: String,
    status_is_error: bool,
    search_area: Option<Rect>,
    form: Option<Form>,
}

impl PickerApp {
    fn new(profile_names: Vec<String>, fzf: FzfFilter, config: PickerConfig) -> Self {
        let loaded = profile_names.iter().map(|_| None).collect();

        Self {
            profile_names,
            loaded,
            active: 0,
            input: TextInput::new(config.initial_query),
            fixed_action: config.fixed_action,
            verb: config.verb,
            fzf,
            filtered: Vec::new(),
            selected: 0,
            list_state: ListState::default(),
            status: String::new(),
            status_is_error: false,
            search_area: None,
            form: None,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<KeyOutcome> {
        if let Some(form) = self.form.as_mut() {
            return Ok(match form.handle_key(key) {
                FormOutcome::Continue => KeyOutcome::Continue,
                FormOutcome::Cancel => {
                    self.form = None;
                    KeyOutcome::Continue
                },
                FormOutcome::Submit => KeyOutcome::SaveForm,
            });
        }

        if key.modifiers.contains(KeyModifiers::ALT) {
            match key.code {
                KeyCode::Char('n') => {
                    self.form = Some(Form::new_login());
                    return Ok(KeyOutcome::Continue);
                },
                KeyCode::Char('e') => {
                    return Ok(self.filtered.get(self.selected).copied().map_or_else(
                        || {
                            self.set_error("No item selected to edit".to_owned());
                            KeyOutcome::Continue
                        },
                        KeyOutcome::Edit,
                    ));
                },
                _ => {},
            }
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.handle_control_key(key);
        }

        match key.code {
            KeyCode::Esc => {
                if !self.input.clear() {
                    return Ok(KeyOutcome::Cancel);
                }
                self.refresh()?;
            },
            KeyCode::Tab if self.profile_names.len() > 1 => {
                return Ok(KeyOutcome::SwitchProfile(1));
            },
            KeyCode::BackTab if self.profile_names.len() > 1 => {
                return Ok(KeyOutcome::SwitchProfile(-1));
            },
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::Home => self.selected = 0,
            KeyCode::End if !self.filtered.is_empty() => {
                self.selected = self.filtered.len() - 1;
            },
            KeyCode::Left => self.input.move_left(),
            KeyCode::Right => self.input.move_right(),
            KeyCode::Enter => {
                let action = default_action(self.current_item(), self.fixed_action);
                return Ok(self.confirm(action));
            },
            KeyCode::Backspace if key.modifiers.contains(KeyModifiers::ALT) => {
                if self.input.delete_word_before() {
                    self.refresh()?;
                }
            },
            KeyCode::Backspace => {
                if self.input.delete_before() {
                    self.refresh()?;
                }
            },
            KeyCode::Delete => {
                if self.input.delete_at() {
                    self.refresh()?;
                }
            },
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::ALT) => {
                self.input.insert(ch);
                self.refresh()?;
            },
            _ => {},
        }

        Ok(KeyOutcome::Continue)
    }

    fn handle_control_key(&mut self, key: KeyEvent) -> Result<KeyOutcome> {
        match key.code {
            KeyCode::Char('c') => return Ok(KeyOutcome::Cancel),
            KeyCode::Char('l') => {
                if self.input.clear() {
                    self.refresh()?;
                }
                return Ok(KeyOutcome::Continue);
            },
            KeyCode::Char('a') => {
                self.input.move_start();
                return Ok(KeyOutcome::Continue);
            },
            KeyCode::Char('e') => {
                self.input.move_end();
                return Ok(KeyOutcome::Continue);
            },
            KeyCode::Char('r') => return Ok(KeyOutcome::Reload),
            KeyCode::Char('k') => {
                if self.input.kill_to_end() {
                    self.refresh()?;
                }
                return Ok(KeyOutcome::Continue);
            },
            _ => {},
        }

        let action = match key.code {
            KeyCode::Char('p') => Some(CopyField::Password),
            KeyCode::Char('u') => Some(CopyField::Username),
            KeyCode::Char('t') => Some(CopyField::Totp),
            KeyCode::Char('o') => Some(CopyField::Uri),
            KeyCode::Char('n') => Some(CopyField::Notes),
            _ => None,
        };

        Ok(action.map_or(KeyOutcome::Continue, |action| self.confirm(action)))
    }

    fn confirm(&mut self, action: CopyField) -> KeyOutcome {
        self.filtered.get(self.selected).copied().map_or_else(
            || {
                self.set_status("No matching item to select".to_owned());
                KeyOutcome::Continue
            },
            |index| KeyOutcome::Confirm(PendingAction { index, action }),
        )
    }

    fn move_selection(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            self.selected = 0;
            return;
        }

        let last = self.filtered.len().saturating_sub(1).cast_signed();
        let next = (self.selected.cast_signed() + delta).clamp(0, last);
        self.selected = next.cast_unsigned();
    }

    fn items(&self) -> &[VaultItem] {
        self.loaded[self.active].as_deref().unwrap_or_default()
    }

    fn is_loaded(&self, profile: usize) -> bool {
        self.loaded[profile].is_some()
    }

    fn store(&mut self, profile: usize, items: Vec<VaultItem>) {
        self.loaded[profile] = Some(items);
    }

    fn activate(&mut self, profile: usize) -> Result<()> {
        self.active = profile;
        self.selected = 0;
        self.refresh()
    }

    fn refresh(&mut self) -> Result<()> {
        let query = self.input.text().trim();
        let filtered = self.fzf.filter(self.items(), query)?;
        self.filtered = filtered;
        self.selected = 0;
        *self.list_state.offset_mut() = 0;
        let status = default_status(&self.filtered, query);
        self.set_status(status);

        Ok(())
    }

    fn set_status(&mut self, message: String) {
        self.status = message;
        self.status_is_error = false;
    }

    fn set_error(&mut self, message: String) {
        self.status = message;
        self.status_is_error = true;
    }

    fn current_item(&self) -> Option<&VaultItem> {
        self.filtered
            .get(self.selected)
            .and_then(|index| self.items().get(*index))
    }

    fn draw_header(&self, frame: &mut ratatui::Frame<'_>, area: Rect, action_label: &str) {
        let header = Paragraph::new(vec![
            Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    "bitpick",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled("profile ", Style::default().fg(Color::Gray)),
                Span::styled(
                    self.profile_names[self.active].clone(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    if self.profile_names.len() > 1 {
                        format!(
                            "  ({}/{}, Tab to switch)",
                            self.active + 1,
                            self.profile_names.len()
                        )
                    } else {
                        String::new()
                    },
                    Style::default().fg(Color::Gray),
                ),
            ]),
            Line::from(vec![
                shortcut_chip("Enter", action_label),
                Span::raw(" "),
                shortcut_chip("Ctrl-P", "pass"),
                Span::raw(" "),
                shortcut_chip("Ctrl-U", "user"),
                Span::raw(" "),
                shortcut_chip("Ctrl-T", "totp"),
                Span::raw(" "),
                shortcut_chip("Ctrl-O", "uri"),
                Span::raw(" "),
                shortcut_chip("Ctrl-N", "notes"),
            ]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(border::ROUNDED)
                .border_style(Style::default().fg(Color::Blue))
                .title(" Overview "),
        );
        frame.render_widget(header, area);
    }

    fn draw_list(&mut self, frame: &mut ratatui::Frame<'_>, area: Rect, action_label: &str) {
        let items = if self.filtered.is_empty() {
            vec![ListItem::new(Line::from("No results"))]
        } else {
            self.filtered
                .iter()
                .map(|index| render_list_item(&self.items()[*index]))
                .collect::<Vec<_>>()
        };

        self.list_state
            .select((!self.filtered.is_empty()).then_some(self.selected));

        let list_title = format!(
            " Credentials · Enter {} {} ",
            self.verb.descriptive(),
            action_label
        );

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(border::ROUNDED)
                    .border_style(Style::default().fg(Color::Blue))
                    .title(list_title),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::LightBlue)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("❯ ");
        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn draw(&mut self, frame: &mut ratatui::Frame<'_>) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(1),
            ])
            .split(frame.area());

        let action_label = default_action(self.current_item(), self.fixed_action).label();

        self.draw_header(frame, outer[0], action_label);

        let search_area = outer[1];
        self.search_area = Some(search_area);

        let cursor_column = self.input.cursor_column();
        let scroll = cursor_column.saturating_sub(search_area.width.saturating_sub(3));

        let search = Paragraph::new(self.input.text()).scroll((0, scroll)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(border::ROUNDED)
                .border_style(Style::default().fg(Color::Magenta))
                .title(" Search ")
                .title_bottom(Line::from("Esc clears, then quits")),
        );
        frame.render_widget(search, search_area);

        let cursor_x = search_area.x + 1 + cursor_column - scroll;
        let cursor_y = search_area.y + 1;
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));

        let middle = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
            .split(outer[2]);

        self.draw_list(frame, middle[0], action_label);

        let help_lines = render_help(action_label, self.verb, self.profile_names.len() > 1);
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),
                Constraint::Length(u16::try_from(help_lines.len()).unwrap_or(u16::MAX) + 2),
            ])
            .split(middle[1]);

        let details = Paragraph::new(render_details(
            self.current_item(),
            self.fixed_action,
            self.verb,
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(border::ROUNDED)
                .border_style(Style::default().fg(Color::Green))
                .title(" Details "),
        )
        .wrap(Wrap { trim: true });
        frame.render_widget(details, right[0]);

        let help = Paragraph::new(help_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(border::ROUNDED)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title(" Actions "),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(help, right[1]);

        let footer = Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                self.status.as_str(),
                Style::default().fg(if self.status_is_error {
                    Color::Red
                } else {
                    Color::Cyan
                }),
            ),
        ]));
        frame.render_widget(footer, outer[3]);

        if let Some(form) = &self.form {
            form.draw(frame);
        }
    }
}

fn default_status(filtered: &[usize], query: &str) -> String {
    if filtered.is_empty() {
        format!("No matches for \"{query}\"")
    } else if query.is_empty() {
        format!("{} item(s) loaded", filtered.len())
    } else {
        format!("{} match(es) for \"{query}\"", filtered.len())
    }
}

fn searchable_text(item: &VaultItem) -> String {
    format!(
        "{} {} {} {}",
        sanitize_inline(&item.name),
        sanitize_inline(item.username().unwrap_or_default()),
        sanitize_inline(item.first_uri().unwrap_or_default()),
        sanitize_inline(item.folder().unwrap_or_default()),
    )
}

/// What Enter copies: `--field` if given, otherwise the password, or the text
/// of a secure note.
fn default_action(item: Option<&VaultItem>, fixed_action: Option<CopyField>) -> CopyField {
    match (fixed_action, item.map(|item| item.kind)) {
        (Some(action), _) => action,
        (None, Some(ItemKind::Note)) => CopyField::Notes,
        (None, _) => CopyField::Password,
    }
}

fn render_list_item(item: &VaultItem) -> ListItem<'static> {
    let mut top = vec![Span::styled(
        item.name.clone(),
        Style::default().add_modifier(Modifier::BOLD),
    )];
    if item.kind == ItemKind::Note {
        top.push(Span::raw("  "));
        top.push(Span::styled("[note]", Style::default().fg(Color::Yellow)));
    }
    if let Some(folder) = item.folder() {
        top.push(Span::raw("  "));
        top.push(Span::styled(
            format!("[{folder}]"),
            Style::default().fg(Color::Magenta),
        ));
    }

    let secondary = format!(
        "{}{}{}",
        item.username().unwrap_or("—"),
        if item.first_uri().is_some() {
            "  •  "
        } else {
            ""
        },
        item.first_uri().unwrap_or(""),
    );

    ListItem::new(vec![
        Line::from(top),
        Line::from(Span::styled(secondary, Style::default().fg(Color::Gray))),
    ])
}

fn render_details(
    item: Option<&VaultItem>,
    fixed_action: Option<CopyField>,
    verb: ActionVerb,
) -> Vec<Line<'static>> {
    let Some(item) = item else {
        return vec![Line::from("No item selected")];
    };

    let default_action = default_action(Some(item), fixed_action);

    vec![
        Line::from(vec![
            Span::styled(
                "Name   ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(item.name.clone()),
        ]),
        Line::from(vec![
            Span::styled(
                "User   ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(item.username().unwrap_or("—").to_owned()),
        ]),
        Line::from(vec![
            Span::styled(
                "URI    ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(item.first_uri().unwrap_or("—").to_owned()),
        ]),
        Line::from(vec![
            Span::styled(
                "Folder ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(item.folder().unwrap_or("—").to_owned()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Default action ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{} {}", verb.descriptive(), default_action.label())),
        ]),
    ]
}

// Lines stay within 33 columns so the pane does not wrap on an 80-column
// terminal.
fn render_help(
    action_label: &str,
    verb: ActionVerb,
    multiple_profiles: bool,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from("↑/↓ PgUp/PgDn Home/End move"),
        Line::from("Ctrl-A/E start/end of query"),
        Line::from("Alt-Backspace delete word"),
        Line::from("Ctrl-K kill to end • Ctrl-L clear"),
        Line::from("Ctrl-R sync and reload"),
        Line::from("Alt-N new login • Alt-E edit"),
        Line::from("Esc clear query, then quit"),
        Line::from(format!("Enter {} {action_label}", verb.descriptive())),
        Line::from("Ctrl-P password • Ctrl-U user"),
        Line::from("Ctrl-T TOTP • Ctrl-O URI"),
        Line::from("Ctrl-N notes"),
    ];
    if multiple_profiles {
        lines.push(Line::from("Tab/Shift-Tab switch profile"));
    }

    lines
}

fn shortcut_chip<'a>(key: &'a str, description: &'a str) -> Span<'a> {
    Span::styled(
        format!(" {key} {description} "),
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )
}

fn sanitize_inline(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\n' | '\r' | '\t' => ' ',
            _ => ch,
        })
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::{FzfFilter, searchable_text};
    use crate::rbw::{ItemKind, VaultItem};

    fn item(id: &str, name: &str, username: &str, uri: &str, folder: Option<&str>) -> VaultItem {
        VaultItem {
            id: id.to_owned(),
            kind: ItemKind::Login,
            name: name.to_owned(),
            username: Some(username.to_owned()),
            folder: folder.map(ToOwned::to_owned),
            uris: vec![uri.to_owned()],
        }
    }

    #[test]
    fn searchable_text_includes_main_fields() {
        let text = searchable_text(&item(
            "1",
            "GitHub",
            "octocat",
            "https://github.com",
            Some("personal"),
        ));
        assert!(text.contains("GitHub"));
        assert!(text.contains("octocat"));
        assert!(text.contains("https://github.com"));
        assert!(text.contains("personal"));
    }

    #[test]
    fn fzf_filter_matches_expected_items_when_available() {
        let Ok(filter) = FzfFilter::new() else {
            return;
        };

        let items = vec![
            item("1", "GitHub", "octocat", "https://github.com", None),
            item("2", "GitLab", "tanuki", "https://gitlab.com", None),
        ];

        let matches = filter.filter(&items, "hub").expect("fzf --filter runs");
        assert_eq!(matches, vec![0]);
    }
}
