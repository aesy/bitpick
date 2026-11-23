use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::rbw::{Entry, ItemKind, NewLogin, VaultItem};
use crate::text_input::TextInput;

const LABEL_WIDTH: u16 = 10;
const NOTES_HEIGHT: u16 = 6;

pub enum FormOutcome {
    Continue,
    Cancel,
    Submit,
}

pub enum FormRequest {
    Add(NewLogin),
    EditLogin {
        id: String,
        password: String,
        notes: String,
    },
    EditNote {
        id: String,
        notes: String,
    },
}

enum Target {
    New,
    Login { id: String },
    Note { id: String },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldStyle {
    Text,
    Secret,
    Multiline,
}

struct Field {
    label: &'static str,
    style: FieldStyle,
    input: TextInput,
}

impl Field {
    fn new(label: &'static str, style: FieldStyle, value: &str) -> Self {
        Self {
            label,
            style,
            input: TextInput::new(value),
        }
    }
}

pub struct Form {
    title: String,
    target: Target,
    fields: Vec<Field>,
    focus: usize,
    reveal: bool,
    message: Option<(String, bool)>,
}

impl Form {
    pub fn new_login() -> Self {
        Self::with_fields(
            " New login ".to_owned(),
            Target::New,
            vec![
                Field::new("Name", FieldStyle::Text, ""),
                Field::new("Username", FieldStyle::Text, ""),
                Field::new("URI", FieldStyle::Text, ""),
                Field::new("Folder", FieldStyle::Text, ""),
                Field::new("Password", FieldStyle::Secret, ""),
                Field::new("Notes", FieldStyle::Multiline, ""),
            ],
        )
    }

    /// rbw can only change the password and notes of an existing entry.
    pub fn edit(item: &VaultItem, entry: &Entry) -> Self {
        let notes = entry.notes().unwrap_or_default();
        let title = format!(" Edit {} ", item.name);

        match item.kind {
            ItemKind::Login => Self::with_fields(
                title,
                Target::Login {
                    id: item.id.clone(),
                },
                vec![
                    Field::new(
                        "Password",
                        FieldStyle::Secret,
                        entry.password().unwrap_or_default(),
                    ),
                    Field::new("Notes", FieldStyle::Multiline, notes),
                ],
            ),
            ItemKind::Note => Self::with_fields(
                title,
                Target::Note {
                    id: item.id.clone(),
                },
                vec![Field::new("Notes", FieldStyle::Multiline, notes)],
            ),
        }
    }

    const fn with_fields(title: String, target: Target, fields: Vec<Field>) -> Self {
        Self {
            title,
            target,
            fields,
            focus: 0,
            reveal: false,
            message: None,
        }
    }

    pub fn set_error(&mut self, message: String) {
        self.message = Some((message, true));
    }

    pub fn set_info(&mut self, message: String) {
        self.message = Some((message, false));
    }

    pub fn request(&self) -> Option<FormRequest> {
        let value = |label: &str| {
            self.fields
                .iter()
                .find(|field| field.label == label)
                .map_or_default(|field| field.input.text().to_owned())
        };

        match &self.target {
            Target::New => {
                let name = value("Name").trim().to_owned();
                if name.is_empty() {
                    return None;
                }

                Some(FormRequest::Add(NewLogin {
                    name,
                    username: value("Username").trim().to_owned(),
                    uri: value("URI").trim().to_owned(),
                    folder: value("Folder").trim().to_owned(),
                    password: value("Password"),
                    notes: value("Notes"),
                }))
            },
            Target::Login { id } => Some(FormRequest::EditLogin {
                id: id.clone(),
                password: value("Password"),
                notes: value("Notes"),
            }),
            Target::Note { id } => Some(FormRequest::EditNote {
                id: id.clone(),
                notes: value("Notes"),
            }),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> FormOutcome {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let multiline = self.fields[self.focus].style == FieldStyle::Multiline;

        match key.code {
            KeyCode::Esc => return FormOutcome::Cancel,
            KeyCode::Char('s') if control => return FormOutcome::Submit,
            KeyCode::Char('v') if alt => self.reveal = !self.reveal,
            KeyCode::Tab | KeyCode::Down => self.move_focus(1),
            KeyCode::BackTab | KeyCode::Up => self.move_focus(-1),
            KeyCode::Enter if !multiline => self.move_focus(1),
            _ => self.edit_focused(key.code, control, alt),
        }

        FormOutcome::Continue
    }

    fn edit_focused(&mut self, code: KeyCode, control: bool, alt: bool) {
        let input = &mut self.fields[self.focus].input;

        match code {
            KeyCode::Enter => input.insert('\n'),
            KeyCode::Backspace if alt => {
                input.delete_word_before();
            },
            KeyCode::Backspace => {
                input.delete_before();
            },
            KeyCode::Delete => {
                input.delete_at();
            },
            KeyCode::Left => input.move_left(),
            KeyCode::Right => input.move_right(),
            KeyCode::Home => input.move_start(),
            KeyCode::End => input.move_end(),
            KeyCode::Char('a') if control => input.move_start(),
            KeyCode::Char('e') if control => input.move_end(),
            KeyCode::Char('k') if control => {
                input.kill_to_end();
            },
            KeyCode::Char(ch) if !control && !alt => input.insert(ch),
            _ => {},
        }
    }

    fn move_focus(&mut self, delta: isize) {
        let last = self.fields.len().cast_signed() - 1;
        self.focus = (self.focus.cast_signed() + delta)
            .clamp(0, last)
            .cast_unsigned();
    }

    fn area(&self, screen: Rect) -> Rect {
        let single_rows = u16::try_from(
            self.fields
                .iter()
                .filter(|field| field.style != FieldStyle::Multiline)
                .count(),
        )
        .unwrap_or(u16::MAX);
        let has_notes = self
            .fields
            .iter()
            .any(|field| field.style == FieldStyle::Multiline);
        let notes_rows = if has_notes { NOTES_HEIGHT + 1 } else { 0 };
        // Fields, a blank row, the message row and the key hints, plus the
        // border.
        let height = single_rows + notes_rows + 3 + 2;
        let width = screen.width.saturating_sub(4).min(72);

        Rect {
            x: screen.x + (screen.width.saturating_sub(width)) / 2,
            y: screen.y + (screen.height.saturating_sub(height)) / 2,
            width,
            height: height.min(screen.height),
        }
    }

    pub fn draw(&self, frame: &mut Frame<'_>) {
        let area = self.area(frame.area());

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Yellow))
            .title(self.title.clone());
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        let mut y = inner.y;
        let mut cursor = None;
        for (index, field) in self.fields.iter().enumerate() {
            let focused = index == self.focus;
            let label_style = if focused {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };

            if field.style == FieldStyle::Multiline {
                frame.render_widget(
                    Paragraph::new(Span::styled(field.label, label_style)),
                    Rect::new(inner.x, y, inner.width, 1).intersection(inner),
                );
                let notes_area =
                    Rect::new(inner.x, y + 1, inner.width, NOTES_HEIGHT).intersection(inner);
                let row = field.input.cursor_row();
                let scroll = row.saturating_sub(NOTES_HEIGHT - 1);
                frame.render_widget(
                    Paragraph::new(field.input.text())
                        .scroll((scroll, 0))
                        .style(Style::default().bg(Color::Black)),
                    notes_area,
                );
                if focused {
                    let column = field
                        .input
                        .cursor_column()
                        .min(inner.width.saturating_sub(1));
                    cursor = Some(Position::new(
                        notes_area.x + column,
                        notes_area.y + row - scroll,
                    ));
                }
                y += NOTES_HEIGHT + 1;
                continue;
            }

            let value_width = inner.width.saturating_sub(LABEL_WIDTH);
            let column = field.input.cursor_column();
            let scroll = column.saturating_sub(value_width.saturating_sub(1));
            let value = if field.style == FieldStyle::Secret && !self.reveal {
                "•".repeat(field.input.text().chars().count())
            } else {
                field.input.text().to_owned()
            };

            frame.render_widget(
                Paragraph::new(Line::from(vec![Span::styled(
                    format!("{:width$}", field.label, width = LABEL_WIDTH as usize),
                    label_style,
                )])),
                Rect::new(inner.x, y, LABEL_WIDTH.min(inner.width), 1).intersection(inner),
            );
            frame.render_widget(
                Paragraph::new(value).scroll((0, scroll)),
                Rect::new(inner.x + LABEL_WIDTH, y, value_width, 1).intersection(inner),
            );
            if focused {
                cursor = Some(Position::new(inner.x + LABEL_WIDTH + column - scroll, y));
            }
            y += 1;
        }

        y += 1;
        if let Some((message, is_error)) = &self.message {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    message.as_str(),
                    Style::default().fg(if *is_error { Color::Red } else { Color::Cyan }),
                )),
                Rect::new(inner.x, y, inner.width, 1).intersection(inner),
            );
        }
        y += 1;

        frame.render_widget(
            Paragraph::new("Tab next • Ctrl-S save • Esc cancel • Alt-V show password")
                .style(Style::default().fg(Color::Gray)),
            Rect::new(inner.x, y, inner.width, 1).intersection(inner),
        );

        if let Some(cursor) = cursor
            && inner.contains(cursor)
        {
            frame.set_cursor_position(cursor);
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{Form, FormRequest};

    fn type_text(form: &mut Form, text: &str) {
        for ch in text.chars() {
            let code = if ch == '\n' {
                KeyCode::Enter
            } else {
                KeyCode::Char(ch)
            };
            form.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
        }
    }

    #[test]
    fn new_login_requires_a_name() {
        assert!(Form::new_login().request().is_none());
    }

    #[test]
    fn new_login_collects_fields_in_order() {
        let mut form = Form::new_login();
        type_text(
            &mut form,
            "GitHub\noctocat\nhttps://github.com\nWork\nsecret\nline 1\nline 2",
        );

        let Some(FormRequest::Add(login)) = form.request() else {
            panic!("expected an add request");
        };
        assert_eq!(login.name, "GitHub");
        assert_eq!(login.username, "octocat");
        assert_eq!(login.uri, "https://github.com");
        assert_eq!(login.folder, "Work");
        assert_eq!(login.password, "secret");
        assert_eq!(login.notes, "line 1\nline 2");
    }
}
