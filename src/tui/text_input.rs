use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

/// Single-line text input with a char-indexed cursor.
///
/// Shared by all TUI inline input boxes (add, rename, …). Does NOT handle
/// Esc/Enter — those carry mode-specific semantics and stay with the caller.
#[derive(Debug, Default, Clone)]
pub struct TextInput {
    buffer: String,
    cursor: usize, // char index, 0..=char_len
}

impl TextInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_text(s: impl Into<String>) -> Self {
        let buffer = s.into();
        let cursor = buffer.chars().count();
        Self { buffer, cursor }
    }

    pub fn text(&self) -> &str {
        &self.buffer
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
    }

    fn char_len(&self) -> usize {
        self.buffer.chars().count()
    }

    /// Handle one key event. Returns true if consumed.
    /// Esc and Enter are intentionally not consumed.
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        match code {
            KeyCode::Home => {
                self.cursor = 0;
                true
            }
            KeyCode::End => {
                self.cursor = self.char_len();
                true
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                true
            }
            KeyCode::Right => {
                if self.cursor < self.char_len() {
                    self.cursor += 1;
                }
                true
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    let mut chars: Vec<char> = self.buffer.chars().collect();
                    chars.remove(self.cursor - 1);
                    self.buffer = chars.into_iter().collect();
                    self.cursor -= 1;
                }
                true
            }
            KeyCode::Delete => {
                if self.cursor < self.char_len() {
                    let mut chars: Vec<char> = self.buffer.chars().collect();
                    chars.remove(self.cursor);
                    self.buffer = chars.into_iter().collect();
                }
                true
            }
            KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                let mut chars: Vec<char> = self.buffer.chars().collect();
                chars.insert(self.cursor, c);
                self.buffer = chars.into_iter().collect();
                self.cursor += 1;
                true
            }
            _ => false,
        }
    }

    /// Render `prefix` + text-with-inline-cursor as a single `Line`.
    pub fn render_line<'a>(
        &'a self,
        prefix: &'a str,
        prefix_style: Style,
        text_style: Style,
    ) -> Line<'a> {
        let chars: Vec<char> = self.buffer.chars().collect();
        let before: String = chars[..self.cursor].iter().collect();
        let cursor_char: String = chars
            .get(self.cursor)
            .map(|c| c.to_string())
            .unwrap_or_else(|| " ".to_string());
        let after_start = if chars.get(self.cursor).is_some() {
            self.cursor + 1
        } else {
            self.cursor
        };
        let after: String = chars[after_start..].iter().collect();
        Line::from(vec![
            Span::styled(prefix.to_string(), prefix_style),
            Span::styled(before, text_style),
            Span::styled(
                cursor_char,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::empty()),
            ),
            Span::styled(after, text_style),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode) -> bool {
        let mut i = TextInput::with_text("abc");
        i.handle_key(code, KeyModifiers::NONE)
    }

    #[test]
    fn with_text_places_cursor_at_end() {
        let i = TextInput::with_text("héllo");
        assert_eq!(i.cursor(), 5);
        assert_eq!(i.text(), "héllo");
    }

    #[test]
    fn home_end_left_right() {
        let mut i = TextInput::with_text("abc");
        assert!(i.handle_key(KeyCode::Home, KeyModifiers::NONE));
        assert_eq!(i.cursor(), 0);
        assert!(i.handle_key(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(i.cursor(), 1);
        assert!(i.handle_key(KeyCode::End, KeyModifiers::NONE));
        assert_eq!(i.cursor(), 3);
        assert!(i.handle_key(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(i.cursor(), 2);
    }

    #[test]
    fn left_at_start_stays() {
        let mut i = TextInput::new();
        assert!(i.handle_key(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(i.cursor(), 0);
    }

    #[test]
    fn right_at_end_stays() {
        assert!(k(KeyCode::Right));
    }

    #[test]
    fn insert_char_moves_cursor() {
        let mut i = TextInput::new();
        i.handle_key(KeyCode::Char('x'), KeyModifiers::NONE);
        i.handle_key(KeyCode::Char('y'), KeyModifiers::NONE);
        assert_eq!(i.text(), "xy");
        assert_eq!(i.cursor(), 2);
    }

    #[test]
    fn insert_at_middle() {
        let mut i = TextInput::with_text("ac");
        i.handle_key(KeyCode::Left, KeyModifiers::NONE);
        i.handle_key(KeyCode::Char('b'), KeyModifiers::NONE);
        assert_eq!(i.text(), "abc");
        assert_eq!(i.cursor(), 2);
    }

    #[test]
    fn backspace_at_start_noop() {
        let mut i = TextInput::with_text("a");
        i.handle_key(KeyCode::Home, KeyModifiers::NONE);
        i.handle_key(KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(i.text(), "a");
        assert_eq!(i.cursor(), 0);
    }

    #[test]
    fn backspace_removes_prev_char() {
        let mut i = TextInput::with_text("abc");
        i.handle_key(KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(i.text(), "ab");
        assert_eq!(i.cursor(), 2);
    }

    #[test]
    fn delete_at_end_noop() {
        let mut i = TextInput::with_text("abc");
        i.handle_key(KeyCode::Delete, KeyModifiers::NONE);
        assert_eq!(i.text(), "abc");
        assert_eq!(i.cursor(), 3);
    }

    #[test]
    fn delete_removes_char_under_cursor() {
        let mut i = TextInput::with_text("abc");
        i.handle_key(KeyCode::Home, KeyModifiers::NONE);
        i.handle_key(KeyCode::Delete, KeyModifiers::NONE);
        assert_eq!(i.text(), "bc");
        assert_eq!(i.cursor(), 0);
    }

    #[test]
    fn ctrl_char_not_consumed() {
        let mut i = TextInput::new();
        assert!(!i.handle_key(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(i.text(), "");
    }

    #[test]
    fn clear_resets_state() {
        let mut i = TextInput::with_text("hello");
        i.clear();
        assert_eq!(i.text(), "");
        assert_eq!(i.cursor(), 0);
    }

    #[test]
    fn handles_multibyte_chars() {
        let mut i = TextInput::with_text("中文");
        assert_eq!(i.cursor(), 2);
        i.handle_key(KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(i.text(), "中");
        assert_eq!(i.cursor(), 1);
        i.handle_key(KeyCode::Home, KeyModifiers::NONE);
        i.handle_key(KeyCode::Char('日'), KeyModifiers::NONE);
        assert_eq!(i.text(), "日中");
        assert_eq!(i.cursor(), 1);
    }
}
