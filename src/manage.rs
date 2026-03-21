use std::io::{self, stdout};
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};

use crate::config::Config;
use crate::editor;
use crate::paths::{contract_tilde, expand_tilde};
use crate::skills::{
    self, SkillInstallStatus, SourceGroup, SourceKind,
};

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum ListRow {
    SourceHeader { group_index: usize },
    Skill { group_index: usize, skill_index: usize },
}

#[derive(Clone)]
enum ConfirmState {
    /// Simple y/N confirmation for non-migrated sources
    Normal { group_index: usize },
    /// Type "delete" confirmation for migrated sources
    Migrated { group_index: usize, typed: String },
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct App {
    config: Config,
    groups: Vec<SourceGroup>,
    rows: Vec<ListRow>,
    cursor: usize,
    scroll_offset: usize,
    status_message: Option<(String, Instant)>,
    search_mode: bool,
    search_query: String,
    filtered_rows: Option<Vec<usize>>,
    should_quit: bool,
    source_filter: Option<String>,
    skills_dir: PathBuf,
    source_dir: PathBuf,
    confirm_state: Option<ConfirmState>,
}

impl App {
    fn new(config: Config, groups: Vec<SourceGroup>, source_filter: Option<String>, skills_dir: PathBuf, source_dir: PathBuf) -> Self {
        let rows = build_rows(&groups);
        Self {
            config,
            groups,
            rows,
            cursor: 0,
            scroll_offset: 0,
            status_message: None,
            search_mode: false,
            search_query: String::new(),
            filtered_rows: None,
            should_quit: false,
            source_filter,
            skills_dir,
            source_dir,
            confirm_state: None,
        }
    }

    fn visible_rows(&self) -> Vec<usize> {
        match &self.filtered_rows {
            Some(indices) => indices.clone(),
            None => (0..self.rows.len()).collect(),
        }
    }

    fn refresh(&mut self) {
        let _ = skills::prune_broken_skills(&self.skills_dir);
        self.groups = skills::scan_all_sources(
            &self.source_dir,
            &self.skills_dir,
            &self.config.central.skill_repos,
        );
        if let Some(ref filter) = self.source_filter {
            if filter != "all" {
                self.groups.retain(|g| g.name == *filter);
            }
        }
        self.rows = build_rows(&self.groups);
        self.apply_search_filter();
        let vis = self.visible_rows();
        if vis.is_empty() {
            self.cursor = 0;
        } else if self.cursor >= vis.len() {
            self.cursor = vis.len() - 1;
        }
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

    fn current_row(&self) -> Option<&ListRow> {
        let vis = self.visible_rows();
        vis.get(self.cursor).and_then(|&i| self.rows.get(i))
    }

    fn move_cursor(&mut self, delta: isize) {
        let vis = self.visible_rows();
        if vis.is_empty() {
            return;
        }
        let new = (self.cursor as isize + delta).clamp(0, vis.len() as isize - 1);
        self.cursor = new as usize;
    }

    fn page_size(&self, area_height: u16) -> usize {
        // Subtract borders + footer
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

    fn apply_search_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_rows = None;
            return;
        }
        let query = self.search_query.to_lowercase();
        let mut matching_groups = std::collections::HashSet::new();
        let mut visible = Vec::new();

        // First pass: find skills matching query
        for (i, row) in self.rows.iter().enumerate() {
            if let ListRow::Skill { group_index, skill_index } = row {
                let skill = &self.groups[*group_index].skills[*skill_index];
                if skill.name.to_lowercase().contains(&query) {
                    matching_groups.insert(*group_index);
                    visible.push(i);
                }
            }
        }

        // Second pass: include source headers for matching groups
        let mut result = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            match row {
                ListRow::SourceHeader { group_index } if matching_groups.contains(group_index) => {
                    result.push(i);
                }
                ListRow::Skill { .. } if visible.contains(&i) => {
                    result.push(i);
                }
                _ => {}
            }
        }
        self.filtered_rows = Some(result);
    }

    fn toggle_skill(&mut self) {
        let row = match self.current_row() {
            Some(r) => r.clone(),
            None => return,
        };
        match row {
            ListRow::Skill { group_index, skill_index } => {
                let skill = &self.groups[group_index].skills[skill_index];
                let name = skill.name.clone();
                let source_path = skill.source_path.clone();
                match skill.install_status {
                    SkillInstallStatus::Installed => {
                        match skills::uninstall_skill(&name, &self.skills_dir) {
                            Ok(()) => {
                                self.groups[group_index].skills[skill_index].install_status =
                                    SkillInstallStatus::NotInstalled;
                                self.set_status(format!("Uninstalled {name}"));
                            }
                            Err(e) => self.set_status(format!("Error: {e}")),
                        }
                    }
                    SkillInstallStatus::NotInstalled => {
                        match skills::install_skill(&name, &source_path, &self.skills_dir) {
                            Ok(()) => {
                                self.groups[group_index].skills[skill_index].install_status =
                                    SkillInstallStatus::Installed;
                                self.set_status(format!("Installed {name}"));
                            }
                            Err(e) => self.set_status(format!("Error: {e}")),
                        }
                    }
                    SkillInstallStatus::Conflict => {
                        self.set_status(format!("Conflict: {name} installed from another source"));
                    }
                }
            }
            ListRow::SourceHeader { group_index } => {
                let all_installed = self.groups[group_index]
                    .skills
                    .iter()
                    .all(|s| s.install_status == SkillInstallStatus::Installed);
                let group_name = self.groups[group_index].name.clone();
                if all_installed {
                    for i in 0..self.groups[group_index].skills.len() {
                        let name = self.groups[group_index].skills[i].name.clone();
                        if self.groups[group_index].skills[i].install_status == SkillInstallStatus::Installed {
                            let _ = skills::uninstall_skill(&name, &self.skills_dir);
                            self.groups[group_index].skills[i].install_status =
                                SkillInstallStatus::NotInstalled;
                        }
                    }
                    self.set_status(format!("Uninstalled all from {group_name}"));
                } else {
                    for i in 0..self.groups[group_index].skills.len() {
                        let name = self.groups[group_index].skills[i].name.clone();
                        let source = self.groups[group_index].skills[i].source_path.clone();
                        if self.groups[group_index].skills[i].install_status == SkillInstallStatus::NotInstalled {
                            let _ = skills::install_skill(&name, &source, &self.skills_dir);
                            self.groups[group_index].skills[i].install_status =
                                SkillInstallStatus::Installed;
                        }
                    }
                    self.set_status(format!("Installed all from {group_name}"));
                }
            }
        }
    }

    fn start_delete(&mut self) {
        let row = match self.current_row() {
            Some(r) => r.clone(),
            None => return,
        };
        if let ListRow::SourceHeader { group_index } = row {
            let group = &self.groups[group_index];
            match &group.kind {
                SourceKind::Migrated { .. } => {
                    self.confirm_state = Some(ConfirmState::Migrated {
                        group_index,
                        typed: String::new(),
                    });
                }
                _ => {
                    self.confirm_state = Some(ConfirmState::Normal { group_index });
                }
            }
        }
    }

    fn execute_delete(&mut self, group_index: usize) {
        let group = self.groups[group_index].clone();
        match skills::delete_source(&group, &self.skills_dir) {
            Ok(()) => {
                // If it was a repo, remove from config
                if let SourceKind::Repo { url: Some(ref url) } = group.kind {
                    self.config.remove_skill_repo(url);
                    let _ = self.config.save();
                }
                self.set_status(format!("Deleted source: {}", group.name));
                self.refresh();
            }
            Err(e) => self.set_status(format!("Error deleting: {e}")),
        }
        self.confirm_state = None;
    }

    fn open_editor(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
        let row = match self.current_row() {
            Some(r) => r.clone(),
            None => return,
        };
        if let ListRow::Skill { group_index, skill_index } = row {
            let skill = &self.groups[group_index].skills[skill_index];
            let skill_md = skill.source_path.join("SKILL.md");
            if !skill_md.exists() {
                self.set_status("SKILL.md not found");
                return;
            }

            // Leave TUI
            let _ = disable_raw_mode();
            let _ = stdout().execute(LeaveAlternateScreen);

            let ed = editor::get_editor(&self.config);
            let _ = editor::open_files(&ed, &[skill_md.as_path()]);

            // Re-enter TUI
            let _ = stdout().execute(EnterAlternateScreen);
            let _ = enable_raw_mode();
            let _ = terminal.clear();
        }
    }

    fn show_info(&mut self) {
        let row = match self.current_row() {
            Some(r) => r.clone(),
            None => return,
        };
        match row {
            ListRow::Skill { group_index, skill_index } => {
                let skill = &self.groups[group_index].skills[skill_index];
                self.set_status(format!("Path: {}", contract_tilde(&skill.source_path)));
            }
            ListRow::SourceHeader { group_index } => {
                let group = &self.groups[group_index];
                let info = match &group.kind {
                    SourceKind::Repo { url: Some(url) } => format!("Repo: {url}"),
                    SourceKind::Repo { url: None } => {
                        format!("Repo: {}", contract_tilde(&group.path))
                    }
                    SourceKind::Local => format!("Local: {}", contract_tilde(&group.path)),
                    SourceKind::Migrated { tool } => {
                        format!("Migrated from {tool}: {}", contract_tilde(&group.path))
                    }
                };
                self.set_status(info);
            }
        }
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, area_height: u16) {
        // Confirmation mode intercepts all keys
        if let Some(state) = self.confirm_state.clone() {
            match state {
                ConfirmState::Normal { group_index } => match code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => self.execute_delete(group_index),
                    _ => {
                        self.confirm_state = None;
                        self.set_status("Delete cancelled");
                    }
                },
                ConfirmState::Migrated { group_index, mut typed } => match code {
                    KeyCode::Char(c) => {
                        typed.push(c);
                        if typed == "delete" {
                            self.execute_delete(group_index);
                        } else if !"delete".starts_with(&typed) {
                            self.confirm_state = None;
                            self.set_status("Delete cancelled");
                        } else {
                            self.confirm_state = Some(ConfirmState::Migrated { group_index, typed });
                        }
                    }
                    KeyCode::Backspace => {
                        typed.pop();
                        self.confirm_state = Some(ConfirmState::Migrated { group_index, typed });
                    }
                    KeyCode::Esc => {
                        self.confirm_state = None;
                        self.set_status("Delete cancelled");
                    }
                    _ => {
                        self.confirm_state = None;
                        self.set_status("Delete cancelled");
                    }
                },
            }
            return;
        }

        // Search mode
        if self.search_mode {
            match code {
                KeyCode::Esc => {
                    self.search_mode = false;
                    self.search_query.clear();
                    self.filtered_rows = None;
                    self.cursor = 0;
                }
                KeyCode::Enter => {
                    self.search_mode = false;
                    // Keep filter active
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.apply_search_filter();
                    self.cursor = 0;
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.apply_search_filter();
                    self.cursor = 0;
                }
                _ => {}
            }
            return;
        }

        // Normal mode
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
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
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => {
                let vis = self.visible_rows();
                if !vis.is_empty() {
                    self.cursor = vis.len() - 1;
                }
            }
            KeyCode::Char(' ') => self.toggle_skill(),
            KeyCode::Char('e') => self.open_editor(terminal),
            KeyCode::Delete | KeyCode::Char('d') => self.start_delete(),
            KeyCode::Char('i') => self.show_info(),
            KeyCode::Char('r') => {
                self.refresh();
                self.set_status("Refreshed");
            }
            KeyCode::Char('/') => {
                self.search_mode = true;
                self.search_query.clear();
                self.filtered_rows = None;
            }
            KeyCode::Esc => {
                if self.filtered_rows.is_some() {
                    self.search_query.clear();
                    self.filtered_rows = None;
                    self.cursor = 0;
                }
                self.status_message = None;
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_rows(groups: &[SourceGroup]) -> Vec<ListRow> {
    let mut rows = Vec::new();
    for (gi, group) in groups.iter().enumerate() {
        rows.push(ListRow::SourceHeader { group_index: gi });
        for si in 0..group.skills.len() {
            rows.push(ListRow::Skill { group_index: gi, skill_index: si });
        }
    }
    rows
}

fn kind_label(kind: &SourceKind) -> &'static str {
    match kind {
        SourceKind::Repo { .. } => "repo",
        SourceKind::Local => "local",
        SourceKind::Migrated { .. } => "migrated",
    }
}

fn kind_icon(kind: &SourceKind) -> &'static str {
    match kind {
        SourceKind::Repo { .. } => "📦",
        SourceKind::Local => "📁",
        SourceKind::Migrated { .. } => "🔀",
    }
}

fn status_icon(status: &SkillInstallStatus) -> &'static str {
    match status {
        SkillInstallStatus::Installed => "✓",
        SkillInstallStatus::NotInstalled => "✗",
        SkillInstallStatus::Conflict => "⚡",
    }
}

fn status_color(status: &SkillInstallStatus) -> Color {
    match status {
        SkillInstallStatus::Installed => Color::Green,
        SkillInstallStatus::NotInstalled => Color::DarkGray,
        SkillInstallStatus::Conflict => Color::Yellow,
    }
}

fn status_label(status: &SkillInstallStatus) -> &'static str {
    match status {
        SkillInstallStatus::Installed => "installed",
        SkillInstallStatus::NotInstalled => "not installed",
        SkillInstallStatus::Conflict => "conflict",
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();

    // Split: main list area + footer (2 lines)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(area);

    render_list(app, frame, chunks[0]);
    render_footer(app, frame, chunks[1]);
}

fn render_list(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" AGM Skills Manager ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = app.visible_rows();
    if visible.is_empty() {
        let msg = if app.search_query.is_empty() {
            "No skills found."
        } else {
            "No matching skills."
        };
        let p = Paragraph::new(msg).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    }

    let height = inner.height as usize;
    let start = app.scroll_offset;
    let end = (start + height).min(visible.len());

    let mut lines: Vec<Line> = Vec::new();
    for vis_idx in start..end {
        let row_idx = visible[vis_idx];
        let is_cursor = vis_idx == app.cursor;
        let row = &app.rows[row_idx];

        let line = match row {
            ListRow::SourceHeader { group_index } => {
                let group = &app.groups[*group_index];
                let count = group.skills.len();
                let plural = if count == 1 { "skill" } else { "skills" };
                let icon = kind_icon(&group.kind);
                let label = kind_label(&group.kind);
                let text = format!("{icon} {} ({label})  [{count} {plural}]", group.name);

                let style = if is_cursor {
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                };
                Line::from(Span::styled(text, style))
            }
            ListRow::Skill { group_index, skill_index } => {
                let skill = &app.groups[*group_index].skills[*skill_index];
                let icon = status_icon(&skill.install_status);
                let color = status_color(&skill.install_status);
                let label = status_label(&skill.install_status);

                let prefix = if is_cursor { " > " } else { "   " };

                let mut spans = vec![
                    Span::styled(prefix, if is_cursor {
                        Style::default().fg(Color::Yellow).bg(Color::DarkGray)
                    } else {
                        Style::default()
                    }),
                    Span::styled(
                        format!("{icon} "),
                        if is_cursor {
                            Style::default().fg(color).bg(Color::DarkGray)
                        } else {
                            Style::default().fg(color)
                        },
                    ),
                ];

                // Skill name with padding to right-align status
                let name_style = if is_cursor {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::White)
                };
                let label_style = if is_cursor {
                    Style::default().fg(color).bg(Color::DarkGray)
                } else {
                    Style::default().fg(color)
                };
                spans.push(Span::styled(format!("{:<30}", skill.name), name_style));
                spans.push(Span::styled(label.to_string(), label_style));

                Line::from(spans)
            }
        };
        lines.push(line);
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Line 1: keybindings or confirmation prompt
    if let Some(ref state) = app.confirm_state {
        let prompt = match state {
            ConfirmState::Normal { group_index } => {
                let g = &app.groups[*group_index];
                format!(
                    "Delete \"{}\" and {} skill(s)? [y/N]",
                    g.name,
                    g.skills.len()
                )
            }
            ConfirmState::Migrated { group_index, typed } => {
                let g = &app.groups[*group_index];
                format!(
                    "⚠ PERMANENT: Delete \"{}\" ({} skills)? Type 'delete': {typed}",
                    g.name,
                    g.skills.len()
                )
            }
        };
        let p = Paragraph::new(Line::from(Span::styled(
            prompt,
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        frame.render_widget(p, inner);
    } else if app.search_mode {
        let prompt = format!("/{}", app.search_query);
        let p = Paragraph::new(Line::from(Span::styled(
            prompt,
            Style::default().fg(Color::Yellow),
        )));
        frame.render_widget(p, inner);
    } else {
        // Compose two lines: hints + status
        let hints = Line::from(vec![
            Span::styled("␣", Style::default().fg(Color::Yellow)),
            Span::raw(" toggle  "),
            Span::styled("e", Style::default().fg(Color::Yellow)),
            Span::raw(" edit  "),
            Span::styled("Del", Style::default().fg(Color::Yellow)),
            Span::raw(" remove source  "),
            Span::styled("i", Style::default().fg(Color::Yellow)),
            Span::raw(" info  "),
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::raw(" refresh  "),
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::raw(" search  "),
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::raw(" quit"),
        ]);

        let status_line = if let Some((ref msg, _)) = app.status_message {
            Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Green)))
        } else {
            Line::default()
        };

        // If inner has room for 1 line, show hints. If 2+ show both.
        if inner.height >= 2 {
            let sub = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Length(1)])
                .split(inner);
            frame.render_widget(Paragraph::new(hints), sub[0]);
            frame.render_widget(Paragraph::new(status_line), sub[1]);
        } else {
            // Prefer status if active, else hints
            if app.status_message.is_some() {
                frame.render_widget(Paragraph::new(status_line), inner);
            } else {
                frame.render_widget(Paragraph::new(hints), inner);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Interactive TUI for managing skills.
pub fn run(config: &mut Config, source_filter: Option<&str>) -> Result<()> {
    let skills_dir = expand_tilde(&config.central.skills_source);
    let source_dir = expand_tilde(&config.central.source_dir);

    // Prune broken symlinks first
    let _ = skills::prune_broken_skills(&skills_dir);

    // Load groups
    let mut groups = skills::scan_all_sources(&source_dir, &skills_dir, &config.central.skill_repos);

    if groups.is_empty() {
        println!("No skill sources found. Use `agm skills add` to add skill sources.");
        return Ok(());
    }

    // Source selection
    let chosen_filter: Option<String> = match source_filter {
        Some(f) => Some(f.to_string()),
        None if groups.len() > 1 => {
            let mut items: Vec<String> = vec!["all".to_string()];
            items.extend(groups.iter().map(|g| g.name.clone()));
            let selection = dialoguer::Select::new()
                .with_prompt("Select source to manage")
                .items(&items)
                .default(0)
                .interact_opt()?;
            match selection {
                Some(0) => Some("all".to_string()),
                Some(i) => Some(items[i].clone()),
                None => return Ok(()), // user cancelled
            }
        }
        None => Some("all".to_string()), // single group → show all
    };

    // Filter groups if not "all"
    if let Some(ref f) = chosen_filter {
        if f != "all" {
            groups.retain(|g| g.name == *f);
        }
    }

    if groups.is_empty() {
        println!("No matching source found.");
        return Ok(());
    }

    // Install panic hook for terminal safety
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // Enter alternate screen + raw mode
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new(
        config.clone(),
        groups,
        chosen_filter,
        skills_dir,
        source_dir,
    );

    // Event loop
    loop {
        let area_height = terminal.size()?.height;
        app.ensure_visible(area_height);
        terminal.draw(|frame| render(&app, frame))?;

        app.clear_expired_status();

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code, key.modifiers, &mut terminal, area_height);
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    // Restore default panic hook
    let _ = std::panic::take_hook();

    // Write back any config changes
    *config = app.config;

    Ok(())
}