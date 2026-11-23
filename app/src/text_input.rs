use ratatui::text::Span;

#[derive(Debug, Clone)]
pub struct TextInput {
    text: String,
    cursor: usize,
}

impl TextInput {
    pub fn new(initial: impl Into<String>) -> Self {
        let text = initial.into();
        let cursor = text.len();
        Self { text, cursor }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Display column of the cursor within its line.
    pub fn cursor_column(&self) -> u16 {
        let before = &self.text[..self.cursor];
        let line_start = before.rfind('\n').map_or(0, |index| index + 1);
        u16::try_from(Span::raw(&before[line_start..]).width()).unwrap_or(u16::MAX)
    }

    pub fn cursor_row(&self) -> u16 {
        u16::try_from(self.text[..self.cursor].matches('\n').count()).unwrap_or(u16::MAX)
    }

    pub fn insert(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    pub fn delete_before(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let prev = self.prev_boundary(self.cursor);
        self.text.replace_range(prev..self.cursor, "");
        self.cursor = prev;
        true
    }

    pub fn delete_at(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        let next = self.next_boundary(self.cursor);
        self.text.replace_range(self.cursor..next, "");
        true
    }

    pub fn delete_word_before(&mut self) -> bool {
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

    pub fn kill_to_end(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        self.text.truncate(self.cursor);
        true
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.prev_boundary(self.cursor);
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor = self.next_boundary(self.cursor);
        }
    }

    pub const fn move_start(&mut self) {
        self.cursor = 0;
    }

    pub const fn move_end(&mut self) {
        self.cursor = self.text.len();
    }

    pub fn clear(&mut self) -> bool {
        if self.text.is_empty() && self.cursor == 0 {
            return false;
        }
        self.text.clear();
        self.cursor = 0;
        true
    }

    pub fn prev_boundary(&self, idx: usize) -> usize {
        self.text[..idx]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    pub fn next_boundary(&self, idx: usize) -> usize {
        self.text[idx..]
            .chars()
            .next()
            .map_or_else(|| self.text.len(), |c| idx + c.len_utf8())
    }
}

#[cfg(test)]
mod tests {
    use super::TextInput;

    #[test]
    fn insert_appends_and_advances_cursor() {
        let mut input = TextInput::new("");
        input.insert('h');
        input.insert('i');
        assert_eq!(input.text(), "hi");
        assert_eq!(input.cursor_column(), 2);
    }

    #[test]
    fn initial_cursor_is_at_end() {
        let input = TextInput::new("abc");
        assert_eq!(input.cursor_column(), 3);
    }

    #[test]
    fn backspace_removes_char_before_cursor() {
        let mut input = TextInput::new("hi");
        assert!(input.delete_before());
        assert_eq!(input.text(), "h");
        assert_eq!(input.cursor_column(), 1);
    }

    #[test]
    fn delete_removes_char_at_cursor() {
        let mut input = TextInput::new("hi");
        input.move_start();
        assert!(input.delete_at());
        assert_eq!(input.text(), "i");
        assert_eq!(input.cursor_column(), 0);
    }

    #[test]
    fn word_delete_removes_last_word_and_trailing_space() {
        let mut input = TextInput::new("hello world ");
        assert!(input.delete_word_before());
        assert_eq!(input.text(), "hello ");
    }

    #[test]
    fn word_delete_at_start_is_noop() {
        let mut input = TextInput::new("hello");
        input.move_start();
        assert!(!input.delete_word_before());
        assert_eq!(input.text(), "hello");
    }

    #[test]
    fn kill_to_end_truncates_from_cursor() {
        let mut input = TextInput::new("hello world");
        input.move_start();
        for _ in 0..5 {
            input.move_right();
        }
        assert!(input.kill_to_end());
        assert_eq!(input.text(), "hello");
    }

    #[test]
    fn move_start_and_end_bracket_the_text() {
        let mut input = TextInput::new("abc");
        input.move_start();
        assert_eq!(input.cursor_column(), 0);
        input.move_end();
        assert_eq!(input.cursor_column(), 3);
    }

    #[test]
    fn clear_resets_state() {
        let mut input = TextInput::new("abc");
        assert!(input.clear());
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor_column(), 0);
    }

    #[test]
    fn cursor_position_tracks_lines() {
        let mut input = TextInput::new("ab");
        input.insert('\n');
        input.insert('c');
        assert_eq!(input.cursor_row(), 1);
        assert_eq!(input.cursor_column(), 1);
        input.move_start();
        assert_eq!(input.cursor_row(), 0);
        assert_eq!(input.cursor_column(), 0);
    }

    #[test]
    fn handles_multibyte_chars() {
        let mut input = TextInput::new("");
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
