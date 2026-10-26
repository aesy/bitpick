use std::fmt::Display;
use std::io::{self, Stdout, Write};
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
use crate::bw::VaultItem;

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

    #[error("{command} failed with status {status}")]
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
    fn descriptive(self) -> &'static str {
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
}

pub enum PickerOutcome {
    Selected { value: String, action: CopyField },
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
}

pub fn run_picker<F, E>(
    items: &[VaultItem],
    config: PickerConfig,
    mut fetch: F,
) -> Result<PickerOutcome>
where
    F: FnMut(&VaultItem, CopyField) -> std::result::Result<String, E>,
    E: Display,
{
    let mut terminal = TerminalGuard::enter()?;
    let mut app = PickerApp::new(items, config)?;

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
            KeyOutcome::Continue => {}
            KeyOutcome::Cancel => return Ok(PickerOutcome::Cancelled),
            KeyOutcome::Confirm(pending) => match items.get(pending.index) {
                Some(item) => match fetch(item, pending.action) {
                    Ok(value) => {
                        return Ok(PickerOutcome::Selected {
                            value,
                            action: pending.action,
                        });
                    }
                    Err(error) => app.set_status(error.to_string()),
                },
                None => app.set_status(TuiError::SelectedItemMissing.to_string()),
            },
        }
    }
}

struct FzfFilter {
    program: PathBuf,
}

impl FzfFilter {
    fn new() -> Result<Self> {
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

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    fn draw<F>(&mut self, render: F) -> Result<()>
    where
        F: FnOnce(&mut ratatui::Frame<'_>),
    {
        self.terminal.draw(render)?;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[derive(Debug, Clone)]
struct SearchInput {
    text: String,
    cursor: usize,
}

impl SearchInput {
    fn new(initial: impl Into<String>) -> Self {
        let text = initial.into();
        let cursor = text.len();
        Self { text, cursor }
    }

    fn text(&self) -> &str {
        &self.text
    }

    fn cursor_column(&self) -> u16 {
        self.text[..self.cursor].chars().count() as u16
    }

    fn insert(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    fn delete_before(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let prev = self.prev_boundary(self.cursor);
        self.text.replace_range(prev..self.cursor, "");
        self.cursor = prev;
        true
    }

    fn delete_at(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        let next = self.next_boundary(self.cursor);
        self.text.replace_range(self.cursor..next, "");
        true
    }

    fn delete_word_before(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let chars: Vec<(usize, char)> = self.text[..self.cursor].char_indices().collect();
        let mut start = self.cursor;
        let mut i = chars.len();
        while i > 0 && chars[i - 1].1.is_whitespace() {
            start = chars[i - 1].0;
            i -= 1;
        }
        while i > 0 && !chars[i - 1].1.is_whitespace() {
            start = chars[i - 1].0;
            i -= 1;
        }
        if start == self.cursor {
            return false;
        }
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
        true
    }

    fn kill_to_end(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        self.text.truncate(self.cursor);
        true
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.prev_boundary(self.cursor);
        }
    }

    fn move_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor = self.next_boundary(self.cursor);
        }
    }

    fn move_start(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.text.len();
    }

    fn clear(&mut self) -> bool {
        if self.text.is_empty() && self.cursor == 0 {
            return false;
        }
        self.text.clear();
        self.cursor = 0;
        true
    }

    fn prev_boundary(&self, idx: usize) -> usize {
        self.text[..idx]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    fn next_boundary(&self, idx: usize) -> usize {
        self.text[idx..]
            .chars()
            .next()
            .map(|c| idx + c.len_utf8())
            .unwrap_or_else(|| self.text.len())
    }
}

struct PickerApp<'a> {
    items: &'a [VaultItem],
    input: SearchInput,
    fixed_action: Option<CopyField>,
    verb: ActionVerb,
    fzf: FzfFilter,
    filtered: Vec<usize>,
    selected: usize,
    status: String,
    search_area: Option<Rect>,
}

impl<'a> PickerApp<'a> {
    fn new(items: &'a [VaultItem], config: PickerConfig) -> Result<Self> {
        let fzf = FzfFilter::new()?;
        let input = SearchInput::new(config.initial_query);
        let filtered = fzf.filter(items, input.text().trim())?;
        let status = config
            .initial_status
            .unwrap_or_else(|| default_status(&filtered, input.text().trim()));

        Ok(Self {
            items,
            input,
            fixed_action: config.fixed_action,
            verb: config.verb,
            fzf,
            filtered,
            selected: 0,
            status,
            search_area: None,
        })
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<KeyOutcome> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.handle_control_key(key);
        }

        match key.code {
            KeyCode::Esc => return Ok(KeyOutcome::Cancel),
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::Home => self.selected = 0,
            KeyCode::End if !self.filtered.is_empty() => {
                self.selected = self.filtered.len() - 1;
            }
            KeyCode::Left => self.input.move_left(),
            KeyCode::Right => self.input.move_right(),
            KeyCode::Enter => {
                return Ok(self.confirm(self.fixed_action.unwrap_or(CopyField::Password)));
            }
            KeyCode::Backspace => {
                if self.input.delete_before() {
                    self.refresh()?;
                }
            }
            KeyCode::Delete => {
                if self.input.delete_at() {
                    self.refresh()?;
                }
            }
            KeyCode::Char(ch) => {
                self.input.insert(ch);
                self.refresh()?;
            }
            _ => {}
        }

        Ok(KeyOutcome::Continue)
    }

    fn handle_control_key(&mut self, key: KeyEvent) -> Result<KeyOutcome> {
        match key.code {
            KeyCode::Char('c') => return Ok(KeyOutcome::Cancel),
            KeyCode::Char('l') | KeyCode::Char('\u{0c}') => {
                if self.input.clear() {
                    self.refresh()?;
                }
                return Ok(KeyOutcome::Continue);
            }
            KeyCode::Char('a') => {
                self.input.move_start();
                return Ok(KeyOutcome::Continue);
            }
            KeyCode::Char('e') => {
                self.input.move_end();
                return Ok(KeyOutcome::Continue);
            }
            KeyCode::Char('w') => {
                if self.input.delete_word_before() {
                    self.refresh()?;
                }
                return Ok(KeyOutcome::Continue);
            }
            KeyCode::Char('k') => {
                if self.input.kill_to_end() {
                    self.refresh()?;
                }
                return Ok(KeyOutcome::Continue);
            }
            _ => {}
        }

        let action = match key.code {
            KeyCode::Char('p') => Some(CopyField::Password),
            KeyCode::Char('u') => Some(CopyField::Username),
            KeyCode::Char('t') => Some(CopyField::Totp),
            KeyCode::Char('o') => Some(CopyField::Uri),
            KeyCode::Char('n') => Some(CopyField::Notes),
            _ => None,
        };

        match action {
            Some(action) => Ok(self.confirm(action)),
            None => Ok(KeyOutcome::Continue),
        }
    }

    fn confirm(&mut self, action: CopyField) -> KeyOutcome {
        match self.filtered.get(self.selected).copied() {
            Some(index) => KeyOutcome::Confirm(PendingAction { index, action }),
            None => {
                self.set_status("No matching item to select".to_owned());
                KeyOutcome::Continue
            }
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            self.selected = 0;
            return;
        }

        let last = self.filtered.len().saturating_sub(1) as isize;
        let next = (self.selected as isize + delta).clamp(0, last);
        self.selected = next as usize;
    }

    fn refresh(&mut self) -> Result<()> {
        let query = self.input.text().trim();
        self.filtered = self.fzf.filter(self.items, query)?;

        if self.filtered.is_empty() {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(self.filtered.len() - 1);
        }
        self.status = default_status(&self.filtered, query);

        Ok(())
    }

    fn set_status(&mut self, message: String) {
        self.status = message;
    }

    fn current_item(&self) -> Option<&'a VaultItem> {
        self.filtered
            .get(self.selected)
            .and_then(|index| self.items.get(*index))
    }

    fn draw(&mut self, frame: &mut ratatui::Frame<'_>) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(8),
                Constraint::Length(2),
            ])
            .split(frame.area());

        let header_chip_label = self.fixed_action.map(|f| f.label()).unwrap_or("password");

        let header = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(" 󰌾 ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "Bitwarden TUI",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled("fast vault access", Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                shortcut_chip("Enter", header_chip_label),
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
        frame.render_widget(header, outer[0]);

        let search_area = outer[1];
        self.search_area = Some(search_area);

        let search = Paragraph::new(self.input.text()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(border::ROUNDED)
                .border_style(Style::default().fg(Color::Magenta))
                .title(" Search with fzf --filter ")
                .title_bottom(Line::from("Ctrl-L clear • Ctrl-W delete word • Esc quit")),
        );
        frame.render_widget(search, search_area);

        let cursor_x = search_area.x + 1 + self.input.cursor_column();
        let cursor_y = search_area.y + 1;
        frame.set_cursor_position(Position::new(cursor_x, cursor_y));

        let middle = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
            .split(outer[2]);

        let items = if self.filtered.is_empty() {
            vec![ListItem::new(Line::from("No results"))]
        } else {
            self.filtered
                .iter()
                .map(|index| render_list_item(&self.items[*index]))
                .collect::<Vec<_>>()
        };

        let mut state = ListState::default();
        if !self.filtered.is_empty() {
            state.select(Some(self.selected));
        }

        let action_label = self.fixed_action.map(|f| f.label()).unwrap_or("password");
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
        frame.render_stateful_widget(list, middle[0], &mut state);

        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(8), Constraint::Length(8)])
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

        let help = Paragraph::new(render_help(self.fixed_action, self.verb))
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
            Span::styled("Status ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("• "),
            Span::styled(self.status.as_str(), Style::default().fg(Color::Cyan)),
            Span::raw("   "),
            Span::styled(
                "Search backend ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("• "),
            Span::styled("fzf --filter", Style::default().fg(Color::Magenta)),
        ]))
        .style(Style::default().fg(Color::Gray));
        frame.render_widget(footer, outer[3]);
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
        sanitize_inline(item.notes.as_deref().unwrap_or_default()),
    )
}

fn render_list_item(item: &VaultItem) -> ListItem<'static> {
    let mut top = Vec::new();
    if item.favorite {
        top.push(Span::styled("★ ", Style::default().fg(Color::Yellow)));
    }
    top.push(Span::styled(
        item.name.clone(),
        Style::default().add_modifier(Modifier::BOLD),
    ));

    let mut flags = Vec::new();
    if item.has_totp() {
        flags.push("totp");
    }
    if item.has_notes() {
        flags.push("notes");
    }
    if !flags.is_empty() {
        top.push(Span::raw("  "));
        top.push(Span::styled(
            format!("[{}]", flags.join(", ")),
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

    let notes = item
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("—");

    let default_action = fixed_action.unwrap_or(CopyField::Password);

    vec![
        Line::from(vec![
            Span::styled(
                "Name  ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(item.name.clone()),
        ]),
        Line::from(vec![
            Span::styled(
                "User  ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(item.username().unwrap_or("—").to_owned()),
        ]),
        Line::from(vec![
            Span::styled(
                "URI   ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(item.first_uri().unwrap_or("—").to_owned()),
        ]),
        Line::from(vec![
            Span::styled(
                "Pass  ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(if item.password().is_some() {
                "••••••••"
            } else {
                "—"
            }),
        ]),
        Line::from(vec![
            Span::styled(
                "TOTP  ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(if item.has_totp() { "available" } else { "—" }),
        ]),
        Line::from(vec![
            Span::styled(
                "Notes ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(notes.to_owned()),
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

fn render_help(fixed_action: Option<CopyField>, verb: ActionVerb) -> Vec<Line<'static>> {
    let verb_str = verb.descriptive();
    let mut lines = vec![
        Line::from("↑/↓ move through results"),
        Line::from("Ctrl-A/E line start/end • Ctrl-W delete word • Ctrl-K kill to end"),
        Line::from("Backspace/Delete edit • Ctrl-L clear • Esc quits"),
    ];

    match fixed_action {
        Some(action) => lines.push(Line::from(format!(
            "Enter {} {}.",
            verb_str,
            action.label()
        ))),
        None => lines.extend([
            Line::from(format!("Enter {} password", verb_str)),
            Line::from(
                "Ctrl-U username • Ctrl-T TOTP • Ctrl-O URI • Ctrl-N notes • Ctrl-P password",
            ),
        ]),
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
    use super::{FzfFilter, SearchInput, searchable_text};
    use crate::bw::{Login, LoginUri, VaultItem};

    #[test]
    fn searchable_text_includes_main_fields() {
        let item = VaultItem {
            id: "1".to_owned(),
            name: "GitHub".to_owned(),
            notes: Some("personal".to_owned()),
            favorite: false,
            login: Some(Login {
                username: Some("octocat".to_owned()),
                password: Some("secret".to_owned()),
                totp: None,
                uris: vec![LoginUri {
                    uri: Some("https://github.com".to_owned()),
                }],
            }),
        };

        let text = searchable_text(&item);
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
            VaultItem {
                id: "1".to_owned(),
                name: "GitHub".to_owned(),
                notes: Some("code hosting".to_owned()),
                favorite: false,
                login: Some(Login {
                    username: Some("octocat".to_owned()),
                    password: Some("secret".to_owned()),
                    totp: None,
                    uris: vec![LoginUri {
                        uri: Some("https://github.com".to_owned()),
                    }],
                }),
            },
            VaultItem {
                id: "2".to_owned(),
                name: "GitLab".to_owned(),
                notes: Some("another forge".to_owned()),
                favorite: false,
                login: Some(Login {
                    username: Some("tanuki".to_owned()),
                    password: Some("secret".to_owned()),
                    totp: None,
                    uris: vec![LoginUri {
                        uri: Some("https://gitlab.com".to_owned()),
                    }],
                }),
            },
        ];

        let matches = match filter.filter(&items, "hub") {
            Ok(matches) => matches,
            Err(_) => return,
        };
        assert_eq!(matches, vec![0]);
    }

    #[test]
    fn search_input_insert_appends_and_advances_cursor() {
        let mut input = SearchInput::new("");
        input.insert('h');
        input.insert('i');
        assert_eq!(input.text(), "hi");
        assert_eq!(input.cursor_column(), 2);
    }

    #[test]
    fn search_input_initial_cursor_is_at_end() {
        let input = SearchInput::new("abc");
        assert_eq!(input.cursor_column(), 3);
    }

    #[test]
    fn search_input_backspace_removes_char_before_cursor() {
        let mut input = SearchInput::new("hi");
        assert!(input.delete_before());
        assert_eq!(input.text(), "h");
        assert_eq!(input.cursor_column(), 1);
    }

    #[test]
    fn search_input_delete_removes_char_at_cursor() {
        let mut input = SearchInput::new("hi");
        input.move_start();
        assert!(input.delete_at());
        assert_eq!(input.text(), "i");
        assert_eq!(input.cursor_column(), 0);
    }

    #[test]
    fn search_input_word_delete_removes_last_word_and_trailing_space() {
        let mut input = SearchInput::new("hello world ");
        assert!(input.delete_word_before());
        assert_eq!(input.text(), "hello ");
    }

    #[test]
    fn search_input_word_delete_at_start_is_noop() {
        let mut input = SearchInput::new("hello");
        input.move_start();
        assert!(!input.delete_word_before());
        assert_eq!(input.text(), "hello");
    }

    #[test]
    fn search_input_kill_to_end_truncates_from_cursor() {
        let mut input = SearchInput::new("hello world");
        input.move_start();
        for _ in 0..5 {
            input.move_right();
        }
        assert!(input.kill_to_end());
        assert_eq!(input.text(), "hello");
    }

    #[test]
    fn search_input_move_start_and_end_bracket_the_text() {
        let mut input = SearchInput::new("abc");
        input.move_start();
        assert_eq!(input.cursor_column(), 0);
        input.move_end();
        assert_eq!(input.cursor_column(), 3);
    }

    #[test]
    fn search_input_clear_resets_state() {
        let mut input = SearchInput::new("abc");
        assert!(input.clear());
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor_column(), 0);
    }

    #[test]
    fn search_input_handles_multibyte_chars() {
        let mut input = SearchInput::new("");
        input.insert('é');
        input.insert('a');
        assert_eq!(input.text(), "éa");
        assert_eq!(input.cursor_column(), 2);
        input.move_left();
        assert_eq!(input.cursor_column(), 1);
        assert!(input.delete_before());
        assert_eq!(input.text(), "a");
    }
}
