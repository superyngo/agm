use std::collections::VecDeque;
use chrono::Local;
use ratatui::text::{Line, Span};
use ratatui::style::{Color, Style};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogLevel { Info, Success, Warning, Error }

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,     // "HH:MM:SS"
    pub message: String,
    pub level: LogLevel,
}

pub struct LogBuffer {
    entries: VecDeque<LogEntry>,
    max_entries: usize,
    pub auto_scroll: bool,
}

impl LogBuffer {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
            auto_scroll: true,
        }
    }

    pub fn push(&mut self, level: LogLevel, message: impl Into<String>) {
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        let entry = LogEntry {
            timestamp,
            message: message.into(),
            level,
        };
        self.entries.push_back(entry);
        if self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    /// Convert entries to styled ratatui Lines for rendering
    pub fn to_lines(&self) -> Vec<Line<'static>> {
        self.entries.iter().map(|e| {
            let color = match e.level {
                LogLevel::Info => Color::White,
                LogLevel::Success => Color::Green,
                LogLevel::Warning => Color::Yellow,
                LogLevel::Error => Color::Red,
            };
            Line::from(vec![
                Span::styled(format!("[{}] ", e.timestamp), Style::default().fg(Color::DarkGray)),
                Span::styled(e.message.clone(), Style::default().fg(color)),
            ])
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_len() {
        let mut buffer = LogBuffer::new(10);
        assert_eq!(buffer.len(), 0);
        
        buffer.push(LogLevel::Info, "First message");
        assert_eq!(buffer.len(), 1);
        
        buffer.push(LogLevel::Success, "Second message");
        assert_eq!(buffer.len(), 2);
        
        buffer.push(LogLevel::Warning, "Third message");
        assert_eq!(buffer.len(), 3);
    }

    #[test]
    fn test_max_entries_eviction() {
        let mut buffer = LogBuffer::new(2);
        
        buffer.push(LogLevel::Info, "First message");
        buffer.push(LogLevel::Success, "Second message");
        assert_eq!(buffer.len(), 2);
        
        // This should evict the first entry
        buffer.push(LogLevel::Warning, "Third message");
        assert_eq!(buffer.len(), 2);
        
        // Check that the first entry is now "Second message"
        assert_eq!(buffer.entries[0].message, "Second message");
        assert_eq!(buffer.entries[1].message, "Third message");
    }

    #[test]
    fn test_to_lines_colors() {
        let mut buffer = LogBuffer::new(10);
        buffer.push(LogLevel::Info, "Info message");
        buffer.push(LogLevel::Success, "Success message");
        buffer.push(LogLevel::Warning, "Warning message");
        buffer.push(LogLevel::Error, "Error message");
        
        let lines = buffer.to_lines();
        assert_eq!(lines.len(), 4);
        
        // Check color for each level
        // Info message (line 0, span 1)
        if let Some(style) = lines[0].spans[1].style.fg {
            assert_eq!(style, Color::White);
        }
        
        // Success message (line 1, span 1) 
        if let Some(style) = lines[1].spans[1].style.fg {
            assert_eq!(style, Color::Green);
        }
        
        // Warning message (line 2, span 1)
        if let Some(style) = lines[2].spans[1].style.fg {
            assert_eq!(style, Color::Yellow);
        }
        
        // Error message (line 3, span 1)
        if let Some(style) = lines[3].spans[1].style.fg {
            assert_eq!(style, Color::Red);
        }
    }

    #[test]
    fn test_auto_scroll_default_true() {
        let buffer = LogBuffer::new(10);
        assert_eq!(buffer.auto_scroll, true);
    }

    #[test]
    fn test_is_empty() {
        let mut buffer = LogBuffer::new(10);
        assert_eq!(buffer.is_empty(), true);
        
        buffer.push(LogLevel::Info, "Test message");
        assert_eq!(buffer.is_empty(), false);
    }
}