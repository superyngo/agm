use std::collections::HashSet;
use std::io::stdout;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame, Terminal,
};

use crate::config::Config;
use crate::editor;
use crate::linker::{self, LinkStatus};
use crate::paths::{contract_tilde, expand_tilde};
use crate::skills;

/// Which central config field a row represents
#[derive(Debug, Clone, PartialEq)]
pub enum CentralField {
    Config,
    Prompt,
    Skills,
    Agents,
    Source,
}

/// Which tool-specific field a row represents
#[derive(Debug, Clone, PartialEq)]
pub enum ToolField {
    Prompt,
    Skills,
    Agents,
    Settings,
    Auth,
    Mcp,
}

/// A single row in the tool TUI list
#[derive(Debug, Clone)]
pub enum ToolRow {
    CentralHeader,
    CentralItem(CentralField),
    ToolHeader {
        key: String,
        name: String,
        installed: bool,
    },
    ToolItem {
        tool_key: String,
        field: ToolField,
    },
}

/// Build the flat list of rows from config state and expanded sections.
/// `expanded` contains keys of sections that are currently open ("central", tool keys).
pub fn build_rows(config: &Config, expanded: &HashSet<String>) -> Vec<ToolRow> {
    let mut rows = Vec::new();

    // Central section
    rows.push(ToolRow::CentralHeader);
    if expanded.contains("central") {
        rows.push(ToolRow::CentralItem(CentralField::Config));
        rows.push(ToolRow::CentralItem(CentralField::Prompt));
        rows.push(ToolRow::CentralItem(CentralField::Skills));
        rows.push(ToolRow::CentralItem(CentralField::Agents));
        rows.push(ToolRow::CentralItem(CentralField::Source));
    }

    // Tool sections — BTreeMap gives alphabetical order
    for (key, tool) in &config.tools {
        let installed = tool.is_installed();
        rows.push(ToolRow::ToolHeader {
            key: key.clone(),
            name: tool.name.clone(),
            installed,
        });
        if expanded.contains(key) {
            rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Prompt });
            rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Skills });
            rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Agents });
            if !tool.settings.is_empty() {
                rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Settings });
            }
            if !tool.auth.is_empty() {
                rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Auth });
            }
            if !tool.mcp.is_empty() {
                rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Mcp });
            }
        }
    }

    rows
}

// ---------------------------------------------------------------------------
// Popup state
// ---------------------------------------------------------------------------

pub enum PopupState {
    Log(super::popup::ScrollablePopup),
    FilePicker {
        title: String,
        files: Vec<(String, PathBuf, bool)>, // (display, resolved_path, exists)
        cursor: usize,
    },
    PathEditor {
        field: CentralField,
        value: String,
        cursor_pos: usize,
    },
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub struct ToolApp {
    config: Config,
    config_path: Option<PathBuf>,
    rows: Vec<ToolRow>,
    cursor: usize,
    scroll_offset: usize,
    expanded: HashSet<String>,
    log: super::log::LogBuffer,
    status_message: Option<(String, Instant)>,
    popup: Option<PopupState>,
    should_quit: bool,
}

impl ToolApp {
    fn new(config: Config, config_path: Option<PathBuf>) -> Self {
        let mut expanded = HashSet::new();
        expanded.insert("central".to_string());
        let rows = build_rows(&config, &expanded);
        Self {
            config,
            config_path,
            rows,
            cursor: 0,
            scroll_offset: 0,
            expanded,
            log: super::log::LogBuffer::new(500),
            status_message: None,
            popup: None,
            should_quit: false,
        }
    }

    fn rebuild_rows(&mut self) {
        let old_len = self.rows.len();
        self.rows = build_rows(&self.config, &self.expanded);
        if self.cursor >= self.rows.len() {
            self.cursor = self.rows.len().saturating_sub(1);
        }
    }

    fn current_row(&self) -> Option<&ToolRow> {
        self.rows.get(self.cursor)
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    fn clear_expired_status(&mut self) {
        if let Some((_, when)) = &self.status_message {
            if when.elapsed().as_secs() >= 3 {
                self.status_message = None;
            }
        }
    }

    fn move_cursor(&mut self, delta: isize) {
        if self.rows.is_empty() { return; }
        let new = (self.cursor as isize + delta).clamp(0, self.rows.len() as isize - 1);
        self.cursor = new as usize;
    }

    fn page_size(&self, area_height: u16) -> usize {
        (area_height.saturating_sub(5)) as usize
    }

    fn ensure_visible(&mut self, area_height: u16) {
        let page = self.page_size(area_height).max(1);
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.cursor >= self.scroll_offset + page {
            self.scroll_offset = self.cursor - page + 1;
        }
    }

    fn toggle_expanded(&mut self, key: &str) {
        if self.expanded.contains(key) {
            self.expanded.remove(key);
        } else {
            self.expanded.insert(key.to_string());
        }
        self.rebuild_rows();
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        _terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        area_height: u16,
    ) {
        // Popup intercepts all keys
        if self.popup.is_some() {
            self.handle_popup_key(code);
            return;
        }

        match code {
            // Navigation
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::PageUp => {
                let page = self.page_size(area_height) as isize;
                self.move_cursor(-page);
            }
            KeyCode::PageDown => {
                let page = self.page_size(area_height) as isize;
                self.move_cursor(page);
            }
            KeyCode::Home => self.move_cursor(-(self.rows.len() as isize)),
            KeyCode::End => self.move_cursor(self.rows.len() as isize),

            // Expand/collapse all
            KeyCode::Char('0') => {
                self.expanded.clear();
                self.rebuild_rows();
            }
            KeyCode::Char('9') => {
                self.expanded.insert("central".to_string());
                for key in self.config.tools.keys() {
                    self.expanded.insert(key.clone());
                }
                self.rebuild_rows();
            }

            // Toggle expand/collapse or link (Task 4.3 will fill in link toggling)
            KeyCode::Char(' ') | KeyCode::Enter => {
                if let Some(row) = self.current_row().cloned() {
                    match &row {
                        ToolRow::CentralHeader => self.toggle_expanded("central"),
                        ToolRow::ToolHeader { key, .. } => self.toggle_expanded(key),
                        // Link toggle will be added in Task 4.3
                        // Path editor will be added in Task 4.6
                        _ => {}
                    }
                }
            }

            // Edit — stub, will be filled in Task 4.4
            KeyCode::Char('e') => {}

            // Log popup
            KeyCode::Char('l') => {
                let lines = self.log.to_lines();
                self.popup = Some(PopupState::Log(
                    super::popup::ScrollablePopup::new("Log", lines)
                        .with_close_hint("l:close"),
                ));
            }

            // Quit
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,

            _ => {}
        }
    }

    fn handle_popup_key(&mut self, code: KeyCode) {
        match &mut self.popup {
            Some(PopupState::Log(ref mut popup)) => {
                match code {
                    KeyCode::Char('l') | KeyCode::Esc => self.popup = None,
                    _ => { popup.handle_key(code); }
                }
            }
            Some(PopupState::FilePicker { ref files, ref mut cursor, .. }) => {
                let len = files.len();
                match code {
                    KeyCode::Up | KeyCode::Char('k') => *cursor = cursor.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => *cursor = (*cursor + 1).min(len.saturating_sub(1)),
                    KeyCode::Esc => self.popup = None,
                    // Enter handling will be in Task 4.5
                    _ => {}
                }
            }
            Some(PopupState::PathEditor { ref mut value, ref mut cursor_pos, .. }) => {
                match code {
                    KeyCode::Char(c) => { value.insert(*cursor_pos, c); *cursor_pos += 1; }
                    KeyCode::Backspace => {
                        if *cursor_pos > 0 { value.remove(*cursor_pos - 1); *cursor_pos -= 1; }
                    }
                    KeyCode::Delete => {
                        if *cursor_pos < value.len() { value.remove(*cursor_pos); }
                    }
                    KeyCode::Left => *cursor_pos = cursor_pos.saturating_sub(1),
                    KeyCode::Right => *cursor_pos = (*cursor_pos + 1).min(value.len()),
                    KeyCode::Home => *cursor_pos = 0,
                    KeyCode::End => *cursor_pos = value.len(),
                    KeyCode::Esc => self.popup = None,
                    // Enter handling will be in Task 4.6
                    _ => {}
                }
            }
            None => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(app: &mut ToolApp, frame: &mut Frame) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    render_list(app, frame, chunks[0]);
    render_footer(app, frame, chunks[1]);

    // Popup overlay
    match &mut app.popup {
        Some(PopupState::Log(ref mut popup)) => popup.render(frame, frame.area()),
        Some(PopupState::FilePicker { .. }) => render_file_picker(app, frame, frame.area()),
        Some(PopupState::PathEditor { .. }) => render_path_editor(app, frame, frame.area()),
        None => {}
    }
}

fn render_list(app: &ToolApp, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" AGM Tool Manager ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.rows.is_empty() {
        let p = Paragraph::new("No tools configured.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    }

    let height = inner.height as usize;
    let start = app.scroll_offset;
    let end = (start + height).min(app.rows.len());

    let mut lines: Vec<Line> = Vec::new();
    for idx in start..end {
        let is_cursor = idx == app.cursor;
        let row = &app.rows[idx];
        let line = render_row(row, is_cursor, &app.config, &app.expanded);
        lines.push(line);
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn render_row(row: &ToolRow, is_cursor: bool, config: &Config, expanded: &HashSet<String>) -> Line<'static> {
    let cursor_prefix = if is_cursor { "▸ " } else { "  " };

    match row {
        ToolRow::CentralHeader => {
            let arrow = if expanded.contains("central") { "▼" } else { "▶" };
            let style = if is_cursor {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            };
            Line::from(Span::styled(format!("{}{} central", cursor_prefix, arrow), style))
        }

        ToolRow::CentralItem(field) => {
            let (label, value) = match field {
                CentralField::Config => {
                    ("config".to_string(), "~/.config/agm/config.toml".to_string())
                }
                CentralField::Prompt => {
                    ("prompt".to_string(), contract_tilde(&expand_tilde(&config.central.prompt_source)))
                }
                CentralField::Skills => {
                    ("skills".to_string(), contract_tilde(&expand_tilde(&config.central.skills_source)))
                }
                CentralField::Agents => {
                    ("agents".to_string(), contract_tilde(&expand_tilde(&config.central.agents_source)))
                }
                CentralField::Source => {
                    ("source".to_string(), contract_tilde(&expand_tilde(&config.central.source_dir)))
                }
            };
            let style = if is_cursor {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(vec![
                Span::raw(format!("{}    ", cursor_prefix)),
                Span::styled(format!("{:<8}", label), Style::default().fg(Color::DarkGray)),
                Span::styled(value, style),
            ])
        }

        ToolRow::ToolHeader { key, name, installed } => {
            let arrow = if expanded.contains(key) { "▼" } else { "▶" };
            let status = if *installed { "" } else { " (not installed)" };
            let style = if is_cursor {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if !installed {
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            };
            Line::from(Span::styled(
                format!("{}{} {} ({}){}", cursor_prefix, arrow, key, name, status),
                style,
            ))
        }

        ToolRow::ToolItem { tool_key, field } => {
            let tool = match config.tools.get(tool_key) {
                Some(t) => t,
                None => return Line::from(""),
            };
            let (label, value_spans) = render_tool_field(tool, field, config);
            let mut spans = vec![
                Span::raw(format!("{}    ", cursor_prefix)),
                Span::styled(format!("{:<8} ", label), Style::default().fg(Color::DarkGray)),
            ];
            spans.extend(value_spans);
            if is_cursor {
                // Highlight the entire line
                Line::from(spans).style(Style::default().fg(Color::Yellow))
            } else {
                Line::from(spans)
            }
        }
    }
}

fn render_tool_field(tool: &crate::config::ToolConfig, field: &ToolField, config: &Config) -> (String, Vec<Span<'static>>) {
    let config_dir = expand_tilde(&tool.config_dir);

    match field {
        ToolField::Prompt => {
            let link_path = config_dir.join(&tool.prompt_filename);
            let target = expand_tilde(&config.central.prompt_source);
            let status = linker::check_link(&link_path, &target, false);
            let spans = link_status_spans(&status, &link_path, &target);
            ("prompt".to_string(), spans)
        }
        ToolField::Skills => {
            let link_path = config_dir.join(&tool.skills_dir);
            let target = expand_tilde(&config.central.skills_source);
            let status = linker::check_link(&link_path, &target, true);
            let spans = link_status_spans(&status, &link_path, &target);
            ("skills".to_string(), spans)
        }
        ToolField::Agents => {
            let link_path = config_dir.join(&tool.agents_dir);
            let target = expand_tilde(&config.central.agents_source);
            let status = linker::check_link(&link_path, &target, true);
            let spans = link_status_spans(&status, &link_path, &target);
            ("agents".to_string(), spans)
        }
        ToolField::Settings => {
            let paths: Vec<String> = tool.settings.iter()
                .map(|s| resolve_display(tool, s))
                .collect();
            ("settings".to_string(), vec![Span::raw(paths.join(", "))])
        }
        ToolField::Auth => {
            let paths: Vec<String> = tool.auth.iter()
                .map(|s| resolve_display(tool, s))
                .collect();
            ("auth".to_string(), vec![Span::raw(paths.join(", "))])
        }
        ToolField::Mcp => {
            let paths: Vec<String> = tool.mcp.iter()
                .map(|s| resolve_display(tool, s))
                .collect();
            ("mcp".to_string(), vec![Span::raw(paths.join(", "))])
        }
    }
}

fn resolve_display(tool: &crate::config::ToolConfig, path: &str) -> String {
    if std::path::Path::new(path).is_absolute() || path.starts_with('~') {
        contract_tilde(&expand_tilde(path))
    } else {
        // Relative to config_dir
        let full = expand_tilde(&tool.config_dir).join(path);
        contract_tilde(&full)
    }
}

fn link_status_spans(status: &LinkStatus, link_path: &std::path::Path, target: &std::path::Path) -> Vec<Span<'static>> {
    match status {
        LinkStatus::Linked => vec![
            Span::styled("✓ linked", Style::default().fg(Color::Green)),
            Span::raw(format!(" → {}", contract_tilde(target))),
        ],
        LinkStatus::Missing => vec![
            Span::styled("✗ not linked", Style::default().fg(Color::Yellow)),
        ],
        LinkStatus::Broken => vec![
            Span::styled("✗ broken", Style::default().fg(Color::Red)),
            Span::raw(format!(" → {}", contract_tilde(link_path))),
        ],
        LinkStatus::Wrong(actual) => vec![
            Span::styled("✗ wrong target", Style::default().fg(Color::Red)),
            Span::raw(format!(" → {}", actual)),
        ],
        LinkStatus::Blocked => vec![
            Span::styled("⚠ blocked", Style::default().fg(Color::Red)),
            Span::raw(" (exists, not a link)"),
        ],
    }
}

fn render_footer(app: &ToolApp, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let hints = Line::from(vec![
        Span::styled("␣", Style::default().fg(Color::Yellow)),
        Span::raw("/"),
        Span::styled("⏎", Style::default().fg(Color::Yellow)),
        Span::raw(" toggle  "),
        Span::styled("e", Style::default().fg(Color::Yellow)),
        Span::raw(" edit  "),
        Span::styled("0", Style::default().fg(Color::Yellow)),
        Span::raw("/"),
        Span::styled("9", Style::default().fg(Color::Yellow)),
        Span::raw(" fold/unfold  "),
        Span::styled("l", Style::default().fg(Color::Yellow)),
        Span::raw(" log  "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit"),
    ]);

    let status_line = if let Some((ref msg, _)) = app.status_message {
        Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Green)))
    } else {
        Line::default()
    };

    if inner.height >= 2 {
        let sub = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(inner);
        frame.render_widget(Paragraph::new(hints), sub[0]);
        frame.render_widget(Paragraph::new(status_line), sub[1]);
    } else if app.status_message.is_some() {
        frame.render_widget(Paragraph::new(status_line), inner);
    } else {
        frame.render_widget(Paragraph::new(hints), inner);
    }
}

fn render_file_picker(app: &ToolApp, frame: &mut Frame, area: Rect) {
    if let Some(PopupState::FilePicker { ref title, ref files, cursor }) = app.popup {
        let popup_area = super::dialog_area(area, files.len() as u16);
        frame.render_widget(Clear, popup_area);
        let block = Block::default()
            .title(format!(" {} ", title))
            .title_bottom(" ⏎:select  Esc:cancel ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let lines: Vec<Line> = files.iter().enumerate().map(|(i, (display, _, exists))| {
            let prefix = if i == cursor { "▸ " } else { "  " };
            let style = if !exists {
                Style::default().fg(Color::DarkGray)
            } else if i == cursor {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            let warning = if !exists { " ⚠ not found" } else { "" };
            Line::from(Span::styled(format!("{}{}{}", prefix, display, warning), style))
        }).collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

fn render_path_editor(app: &ToolApp, frame: &mut Frame, area: Rect) {
    if let Some(PopupState::PathEditor { ref field, ref value, cursor_pos }) = app.popup {
        let popup_area = super::dialog_area(area, 3);
        frame.render_widget(Clear, popup_area);
        let label = match field {
            CentralField::Skills => "skills",
            CentralField::Agents => "agents",
            CentralField::Source => "source",
            _ => "path",
        };
        let block = Block::default()
            .title(format!(" Edit {} path ", label))
            .title_bottom(" ⏎:save  Esc:cancel ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        // Render value with cursor
        let mut spans = Vec::new();
        for (i, ch) in value.chars().enumerate() {
            if i == cursor_pos {
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(Color::Black).bg(Color::White),
                ));
            } else {
                spans.push(Span::raw(ch.to_string()));
            }
        }
        if cursor_pos >= value.len() {
            spans.push(Span::styled(" ", Style::default().fg(Color::Black).bg(Color::White)));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), inner);
    }
}

/// Entry point for the tool TUI.
pub fn run(config_path: Option<PathBuf>) -> Result<()> {
    let config = Config::load_from(config_path.clone())?;
    let mut app = ToolApp::new(config, config_path);

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        prev_hook(info);
    }));

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        let area_height = terminal.size()?.height;
        app.ensure_visible(area_height);
        terminal.draw(|frame| render(&mut app, frame))?;

        app.clear_expired_status();

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code, &mut terminal, area_height);
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CentralConfig, ToolConfig};
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    fn test_config_with_tools(tools: Vec<(&str, ToolConfig)>) -> Config {
        let mut tools_map = BTreeMap::new();
        for (key, tool) in tools {
            tools_map.insert(key.to_string(), tool);
        }
        Config {
            editor: String::new(),
            central: CentralConfig {
                prompt_source: "~/.local/share/agm/prompts/MASTER.md".to_string(),
                skills_source: "~/.local/share/agm/skills".to_string(),
                agents_source: "~/.local/share/agm/agents".to_string(),
                source_dir: "~/.local/share/agm/source".to_string(),
                source_repos: vec![],
            },
            tools: tools_map,
        }
    }

    fn test_tool_config(name: &str, config_dir: &str, with_optional: bool) -> ToolConfig {
        ToolConfig {
            name: name.to_string(),
            config_dir: config_dir.to_string(),
            settings: if with_optional { vec!["settings.json".to_string()] } else { vec![] },
            auth: if with_optional { vec!["auth.json".to_string()] } else { vec![] },
            prompt_filename: "PROMPT.md".to_string(),
            skills_dir: "skills".to_string(),
            agents_dir: "agents".to_string(),
            mcp: if with_optional { vec!["mcp.json".to_string()] } else { vec![] },
        }
    }

    #[test]
    fn test_build_rows_all_collapsed() {
        let config = test_config_with_tools(vec![
            ("claude", test_tool_config("Claude Code", "/nonexistent/claude", true)),
            ("copilot", test_tool_config("Copilot CLI", "/nonexistent/copilot", true)),
        ]);
        let expanded = HashSet::new();
        let rows = build_rows(&config, &expanded);

        // Should have 1 central header + 2 tool headers
        assert_eq!(rows.len(), 3);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::ToolHeader { ref key, .. } if key == "claude"));
        assert!(matches!(rows[2], ToolRow::ToolHeader { ref key, .. } if key == "copilot"));
    }

    #[test]
    fn test_build_rows_central_expanded() {
        let config = test_config_with_tools(vec![
            ("claude", test_tool_config("Claude Code", "/nonexistent/claude", true)),
        ]);
        let mut expanded = HashSet::new();
        expanded.insert("central".to_string());
        let rows = build_rows(&config, &expanded);

        // Should have central header + 5 central items + 1 tool header
        assert_eq!(rows.len(), 7);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::CentralItem(CentralField::Config)));
        assert!(matches!(rows[2], ToolRow::CentralItem(CentralField::Prompt)));
        assert!(matches!(rows[3], ToolRow::CentralItem(CentralField::Skills)));
        assert!(matches!(rows[4], ToolRow::CentralItem(CentralField::Agents)));
        assert!(matches!(rows[5], ToolRow::CentralItem(CentralField::Source)));
        assert!(matches!(rows[6], ToolRow::ToolHeader { ref key, .. } if key == "claude"));
    }

    #[test]
    fn test_build_rows_tool_expanded() {
        let _tmp = TempDir::new().unwrap();
        let tool_dir = _tmp.path().join("claude");
        std::fs::create_dir_all(&tool_dir).unwrap();
        
        let config = test_config_with_tools(vec![
            ("claude", test_tool_config("Claude Code", &tool_dir.to_string_lossy(), true)),
        ]);
        let mut expanded = HashSet::new();
        expanded.insert("claude".to_string());
        let rows = build_rows(&config, &expanded);

        // Should have central header + tool header + 6 tool items (Prompt, Skills, Agents, Settings, Auth, Mcp)
        assert_eq!(rows.len(), 8);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::ToolHeader { ref key, installed: true, .. } if key == "claude"));
        assert!(matches!(rows[2], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Prompt));
        assert!(matches!(rows[3], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Skills));
        assert!(matches!(rows[4], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Agents));
        assert!(matches!(rows[5], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Settings));
        assert!(matches!(rows[6], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Auth));
        assert!(matches!(rows[7], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Mcp));
    }

    #[test]
    fn test_build_rows_empty_vec_skipped() {
        let _tmp = TempDir::new().unwrap();
        let tool_dir = _tmp.path().join("minimal");
        std::fs::create_dir_all(&tool_dir).unwrap();
        
        let config = test_config_with_tools(vec![
            ("minimal", test_tool_config("Minimal Tool", &tool_dir.to_string_lossy(), false)),
        ]);
        let mut expanded = HashSet::new();
        expanded.insert("minimal".to_string());
        let rows = build_rows(&config, &expanded);

        // Should have central header + tool header + 3 tool items (only Prompt, Skills, Agents)
        assert_eq!(rows.len(), 5);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::ToolHeader { ref key, .. } if key == "minimal"));
        assert!(matches!(rows[2], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Prompt));
        assert!(matches!(rows[3], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Skills));
        assert!(matches!(rows[4], ToolRow::ToolItem { ref field, .. } if *field == ToolField::Agents));

        // Verify no Settings/Auth/Mcp items
        for row in &rows {
            if let ToolRow::ToolItem { field, .. } = row {
                assert!(!matches!(field, ToolField::Settings | ToolField::Auth | ToolField::Mcp));
            }
        }
    }

    #[test]
    fn test_build_rows_alphabetical() {
        let config = test_config_with_tools(vec![
            ("zed", test_tool_config("Zed Editor", "/nonexistent/zed", true)),
            ("alpha", test_tool_config("Alpha Tool", "/nonexistent/alpha", true)),
        ]);
        let expanded = HashSet::new();
        let rows = build_rows(&config, &expanded);

        // Should have central header + alpha tool header + zed tool header (alphabetical order)
        assert_eq!(rows.len(), 3);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::ToolHeader { ref key, .. } if key == "alpha"));
        assert!(matches!(rows[2], ToolRow::ToolHeader { ref key, .. } if key == "zed"));
    }
}