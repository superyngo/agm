use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub const MAX_CONTENT_LINES: usize = 5000;

pub struct ScrollablePopup {
    pub title: String,
    /// Optional styled title that overrides `title` when set. Used by panels
    /// that need richer title rendering (e.g., the tab-bar on the Help/About
    /// panel showing both tabs at once).
    pub title_line: Option<Line<'static>>,
    pub lines: Vec<Line<'static>>,
    pub scroll_offset: usize,
    visible_height: usize,
    /// Number of rows the content occupies after word-wrapping at the current
    /// width. Defaults to the logical line count and is refined on each render.
    content_height: usize,
    close_hint: String, // e.g., "Esc:close" or "l:close"
}

impl ScrollablePopup {
    pub fn new(title: impl Into<String>, mut lines: Vec<Line<'static>>) -> Self {
        // Truncate lines if they exceed MAX_CONTENT_LINES
        if lines.len() > MAX_CONTENT_LINES {
            lines.truncate(MAX_CONTENT_LINES);
            lines.push(Line::from("[truncated — content too large]"));
        }

        let content_height = lines.len();
        Self {
            title: title.into(),
            title_line: None,
            lines,
            scroll_offset: 0,
            visible_height: 1, // Will be updated on first render
            content_height,    // Refined to the wrapped row count on first render
            close_hint: "Esc:close".to_string(),
        }
    }

    pub fn with_close_hint(mut self, hint: impl Into<String>) -> Self {
        self.close_hint = hint.into();
        self
    }

    /// Override the plain string title with a styled `Line`. Use this for
    /// titles that mix styles (e.g., a tab bar with active/inactive segments).
    pub fn with_title_line(mut self, line: Line<'static>) -> Self {
        self.title_line = Some(line);
        self
    }

    /// Returns PopupAction indicating what the caller should do.
    pub fn handle_key(&mut self, code: KeyCode) -> PopupAction {
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_up(1);
                PopupAction::Consumed
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down(1);
                PopupAction::Consumed
            }
            KeyCode::PageUp => {
                self.scroll_up(self.visible_height.max(1));
                PopupAction::Consumed
            }
            KeyCode::PageDown => {
                self.scroll_down(self.visible_height.max(1));
                PopupAction::Consumed
            }
            KeyCode::Home => {
                self.scroll_offset = 0;
                PopupAction::Consumed
            }
            KeyCode::End => {
                self.scroll_to_end();
                PopupAction::Consumed
            }
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => PopupAction::Close,
            _ => PopupAction::Ignored,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        // Use super::popup_area(area) to calculate centered rect
        let popup_rect = super::popup_area(area);

        // Render Clear widget first (clears background)
        frame.render_widget(Clear, popup_rect);

        // Create block with title and close hint
        let title_text = format!(" {} ", self.title);
        let close_hint_text = format!(" {} ", self.close_hint);

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        // Prefer the styled title line when one is set; it usually encodes
        // mixed styles (e.g., a tab bar). Surround it with single spaces so
        // it lines up with the plain-text variant.
        block = if let Some(line) = self.title_line.clone() {
            let padded = Line::from({
                let mut spans = Vec::with_capacity(line.spans.len() + 2);
                spans.push(Span::raw(" "));
                spans.extend(line.spans);
                spans.push(Span::raw(" "));
                spans
            });
            block.title(padded)
        } else {
            block.title(title_text)
        };
        block = block.title_bottom(close_hint_text.as_str());

        let inner_area = block.inner(popup_rect);

        // Update visible height from inner area
        self.visible_height = inner_area.height as usize;

        // Render the block
        frame.render_widget(block, popup_rect);

        // Create paragraph with content
        let paragraph = Paragraph::new(self.lines.clone()).wrap(Wrap { trim: true });

        // Measure how many rows the content occupies once word-wrapped at the
        // current width. Scroll offset is applied to wrapped rows, so this is
        // what the scroll bounds and page indicator must be based on.
        self.content_height = paragraph.line_count(inner_area.width);

        // Re-clamp in case a resize shrank the reachable range.
        self.scroll_offset = self.scroll_offset.min(self.max_scroll());

        frame.render_widget(paragraph.scroll((self.scroll_offset as u16, 0)), inner_area);

        // Render page indicator at bottom-right
        if !self.lines.is_empty() {
            let position_text = format!("[{}/{}]", self.current_page(), self.total_pages());
            let position_span = Span::styled(position_text, Style::default().fg(Color::Gray));

            // Calculate position for bottom-right alignment
            let indicator_width = position_span.content.len() as u16;
            if inner_area.width >= indicator_width && inner_area.height > 0 {
                let x = inner_area.x + inner_area.width - indicator_width;
                let y = inner_area.y + inner_area.height - 1;
                let indicator_area = Rect::new(x, y, indicator_width, 1);

                let indicator_paragraph = Paragraph::new(Line::from(vec![position_span]));
                frame.render_widget(indicator_paragraph, indicator_area);
            }
        }
    }

    pub fn current_page(&self) -> usize {
        if self.visible_height == 0 || self.lines.is_empty() {
            return 1;
        }
        // At the bottom the scroll offset is content_height - visible_height,
        // which rarely lands on a clean page boundary. Snap to the last page so
        // reaching the end reads N/N rather than (N-1)/N.
        if self.scroll_offset >= self.max_scroll() {
            return self.total_pages();
        }
        let page = self.scroll_offset / self.visible_height + 1;
        page.min(self.total_pages())
    }

    pub fn total_pages(&self) -> usize {
        if self.visible_height == 0 || self.lines.is_empty() {
            return 1;
        }
        self.content_height.div_ceil(self.visible_height).max(1)
    }

    fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: usize) {
        let max = self.max_scroll();
        self.scroll_offset = (self.scroll_offset + amount).min(max);
    }

    fn scroll_to_end(&mut self) {
        self.scroll_offset = self.max_scroll();
    }

    fn max_scroll(&self) -> usize {
        self.content_height
            .saturating_sub(self.visible_height.max(1))
    }
}

#[derive(Debug, PartialEq)]
pub enum PopupAction {
    Consumed, // key was handled, popup stays open
    Close,    // popup should close
    Ignored,  // key not relevant to popup
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Line;

    #[test]
    fn test_scroll_up_down() {
        let lines = vec![
            Line::from("line 1"),
            Line::from("line 2"),
            Line::from("line 3"),
            Line::from("line 4"),
            Line::from("line 5"),
        ];
        let mut popup = ScrollablePopup::new("Test", lines);
        popup.visible_height = 3; // Simulate having visible height

        // Test scroll down
        popup.scroll_down(2);
        assert_eq!(popup.scroll_offset, 2);

        // Test scroll up
        popup.scroll_up(1);
        assert_eq!(popup.scroll_offset, 1);

        // Test scroll up beyond beginning
        popup.scroll_up(5);
        assert_eq!(popup.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_clamp() {
        let lines = vec![
            Line::from("line 1"),
            Line::from("line 2"),
            Line::from("line 3"),
        ];
        let mut popup = ScrollablePopup::new("Test", lines);
        popup.visible_height = 3;

        // Try to scroll past the end
        popup.scroll_down(10);
        let max_scroll = popup.max_scroll();
        assert_eq!(popup.scroll_offset, max_scroll);

        // For 3 lines with visible_height 3, max_scroll should be 0
        assert_eq!(max_scroll, 0);
    }

    #[test]
    fn test_page_up_down() {
        let lines = (1..=20)
            .map(|i| Line::from(format!("line {}", i)))
            .collect();
        let mut popup = ScrollablePopup::new("Test", lines);
        popup.visible_height = 5;

        // Test page down
        assert_eq!(popup.handle_key(KeyCode::PageDown), PopupAction::Consumed);
        assert_eq!(popup.scroll_offset, 5);

        // Test page up
        assert_eq!(popup.handle_key(KeyCode::PageUp), PopupAction::Consumed);
        assert_eq!(popup.scroll_offset, 0);
    }

    #[test]
    fn test_home_end() {
        let lines = (1..=10)
            .map(|i| Line::from(format!("line {}", i)))
            .collect();
        let mut popup = ScrollablePopup::new("Test", lines);
        popup.visible_height = 3;

        // Scroll to middle
        popup.scroll_down(3);
        assert!(popup.scroll_offset > 0);

        // Test home
        assert_eq!(popup.handle_key(KeyCode::Home), PopupAction::Consumed);
        assert_eq!(popup.scroll_offset, 0);

        // Test end
        assert_eq!(popup.handle_key(KeyCode::End), PopupAction::Consumed);
        assert_eq!(popup.scroll_offset, popup.max_scroll());
    }

    #[test]
    fn test_handle_key_esc() {
        let lines = vec![Line::from("test line")];
        let mut popup = ScrollablePopup::new("Test", lines);

        assert_eq!(popup.handle_key(KeyCode::Esc), PopupAction::Close);
    }

    #[test]
    fn test_handle_key_enter_close() {
        let lines = vec![Line::from("test line")];
        let mut popup = ScrollablePopup::new("Test", lines);

        assert_eq!(popup.handle_key(KeyCode::Enter), PopupAction::Close);
    }

    #[test]
    fn test_handle_key_space_close() {
        let lines = vec![Line::from("test line")];
        let mut popup = ScrollablePopup::new("Test", lines);

        assert_eq!(popup.handle_key(KeyCode::Char(' ')), PopupAction::Close);
    }

    #[test]
    fn test_handle_key_unknown() {
        let lines = vec![Line::from("test line")];
        let mut popup = ScrollablePopup::new("Test", lines);

        assert_eq!(popup.handle_key(KeyCode::Char('x')), PopupAction::Ignored);
        assert_eq!(popup.handle_key(KeyCode::Tab), PopupAction::Ignored);
    }

    #[test]
    fn test_truncation() {
        let lines: Vec<Line<'static>> = (1..=6000)
            .map(|i| Line::from(format!("line {}", i)))
            .collect();

        let popup = ScrollablePopup::new("Test", lines);

        // Should be truncated to MAX_CONTENT_LINES + 1 (for truncation message)
        assert_eq!(popup.lines.len(), MAX_CONTENT_LINES + 1);

        // Check truncation message
        let last_line = &popup.lines[MAX_CONTENT_LINES];
        assert_eq!(last_line.to_string(), "[truncated — content too large]");
    }

    #[test]
    fn test_new_defaults() {
        let lines = vec![Line::from("test line")];
        let popup = ScrollablePopup::new("Test Title", lines);

        assert_eq!(popup.title, "Test Title");
        assert_eq!(popup.scroll_offset, 0);
        assert_eq!(popup.close_hint, "Esc:close");
        assert_eq!(popup.visible_height, 1);
    }

    #[test]
    fn test_with_close_hint() {
        let lines = vec![Line::from("test line")];
        let popup = ScrollablePopup::new("Test", lines).with_close_hint("q:quit");

        assert_eq!(popup.close_hint, "q:quit");
    }

    #[test]
    fn test_vi_keys() {
        let lines = (1..=10)
            .map(|i| Line::from(format!("line {}", i)))
            .collect();
        let mut popup = ScrollablePopup::new("Test", lines);
        popup.visible_height = 3;

        // Test 'j' (down)
        assert_eq!(popup.handle_key(KeyCode::Char('j')), PopupAction::Consumed);
        assert_eq!(popup.scroll_offset, 1);

        // Test 'k' (up)
        assert_eq!(popup.handle_key(KeyCode::Char('k')), PopupAction::Consumed);
        assert_eq!(popup.scroll_offset, 0);
    }

    #[test]
    fn test_page_indicator_values() {
        let lines = (1..=20)
            .map(|i| Line::from(format!("line {}", i)))
            .collect();
        let mut popup = ScrollablePopup::new("Test", lines);
        popup.visible_height = 5;

        assert_eq!(popup.current_page(), 1);
        assert_eq!(popup.total_pages(), 4);

        popup.scroll_offset = 5;
        assert_eq!(popup.current_page(), 2);

        popup.scroll_offset = 15;
        assert_eq!(popup.current_page(), 4);

        popup.scroll_offset = 18;
        assert_eq!(popup.current_page(), 4);
    }

    #[test]
    fn test_page_indicator_single_page() {
        let lines = vec![Line::from("line 1"), Line::from("line 2")];
        let mut popup = ScrollablePopup::new("Test", lines);
        popup.visible_height = 10;

        assert_eq!(popup.current_page(), 1);
        assert_eq!(popup.total_pages(), 1);
    }

    #[test]
    fn test_scroll_end_reaches_wrapped_bottom() {
        // 10 logical lines that each wrap into 3 rows => 30 wrapped rows.
        let lines = (1..=10)
            .map(|i| Line::from(format!("line {}", i)))
            .collect();
        let mut popup = ScrollablePopup::new("Test", lines);
        popup.visible_height = 5;
        popup.content_height = 30; // what render() would measure post-wrap

        // End must reach the true wrapped bottom, not lines.len()-visible_height.
        popup.handle_key(KeyCode::End);
        assert_eq!(popup.scroll_offset, 25);
        assert_eq!(popup.total_pages(), 6);
        // At the end the indicator must read the last page (6/6), even though
        // max_scroll (25) is not a multiple of visible_height (5).
        assert_eq!(popup.current_page(), 6);
    }

    #[test]
    fn test_page_indicator_snaps_to_last_at_end() {
        // content_height not a multiple of visible_height => bottom offset is
        // off the page grid; the indicator must still reach total_pages.
        let lines = (1..=10)
            .map(|i| Line::from(format!("line {}", i)))
            .collect();
        let mut popup = ScrollablePopup::new("Test", lines);
        popup.visible_height = 4;
        popup.content_height = 23; // ceil(23/4) = 6 pages, max_scroll = 19

        assert_eq!(popup.total_pages(), 6);
        popup.handle_key(KeyCode::End);
        assert_eq!(popup.scroll_offset, 19);
        assert_eq!(popup.current_page(), 6);

        // One line up from the bottom drops back below the last page.
        popup.handle_key(KeyCode::Up);
        assert!(popup.current_page() < 6);
    }

    #[test]
    fn test_page_indicator_empty() {
        let popup = ScrollablePopup::new("Test", vec![]);
        assert_eq!(popup.current_page(), 1);
        assert_eq!(popup.total_pages(), 1);
    }

    /// Regression: with word-wrapped content, pressing End must reveal the
    /// final logical line. Before the fix, scroll bounds used the pre-wrap
    /// line count, so the wrapped tail was unreachable. Drives the real
    /// render() path (which measures wrapped height via Paragraph::line_count).
    #[test]
    fn test_render_end_reveals_last_line_with_wrapping() {
        use ratatui::{backend::TestBackend, Terminal};

        // Several long lines that each wrap into multiple rows, then a short
        // unique marker as the very last line.
        let mut lines: Vec<Line<'static>> = (0..6).map(|_| Line::from("A".repeat(120))).collect();
        lines.push(Line::from("ZZZ_BOTTOM_MARKER"));

        let mut popup = ScrollablePopup::new("Info", lines);

        let backend = TestBackend::new(60, 16);
        let mut terminal = Terminal::new(backend).unwrap();

        // First render populates visible_height and the wrapped content_height.
        terminal.draw(|f| popup.render(f, f.area())).unwrap();

        // Marker is below the fold initially.
        let before = buffer_to_string(terminal.backend().buffer());
        assert!(!before.contains("ZZZ_BOTTOM_MARKER"));

        // Scroll to the end and re-render.
        popup.handle_key(KeyCode::End);
        terminal.draw(|f| popup.render(f, f.area())).unwrap();

        let after = buffer_to_string(terminal.backend().buffer());
        assert!(
            after.contains("ZZZ_BOTTOM_MARKER"),
            "End should reveal the last line; buffer was:\n{after}"
        );
    }

    fn buffer_to_string(buf: &ratatui::buffer::Buffer) -> String {
        buf.content.iter().map(|c| c.symbol()).collect()
    }
}
