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
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::config::Config;
use crate::editor;
use crate::linker::{self, LinkStatus};
use crate::paths::{contract_tilde, expand_tilde};
use crate::skills;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// Which central config field a row represents
#[derive(Debug, Clone, PartialEq)]
pub enum CentralField {
    Config,
    Prompt,
    Skills,
    Agents,
    Commands,
    Source,
}

/// Which linkable field a tool row represents
#[derive(Debug, Clone, PartialEq)]
pub enum LinkField {
    Prompt,
    Skills,
    Agents,
    Commands,
}

/// Which file-group type a row represents
#[derive(Debug, Clone, PartialEq)]
pub enum FileGroup {
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
    StatusHeader {
        tool_key: String,
    },
    LinkItem {
        tool_key: String,
        field: LinkField,
    },
    FileGroupHeader {
        tool_key: String,
        group: FileGroup,
    },
    FileItem {
        tool_key: String,
        group: FileGroup,
        index: usize,
    },
}

fn group_key_suffix(group: &FileGroup) -> &'static str {
    match group {
        FileGroup::Settings => "settings",
        FileGroup::Auth => "auth",
        FileGroup::Mcp => "mcp",
    }
}

fn group_label(group: &FileGroup) -> &'static str {
    match group {
        FileGroup::Settings => "settings",
        FileGroup::Auth => "auth",
        FileGroup::Mcp => "mcp",
    }
}

/// Build the flat list of rows from config state and expanded sections.
pub fn build_rows(config: &Config, expanded: &HashSet<String>) -> Vec<ToolRow> {
    let mut rows = Vec::new();

    // Central section
    rows.push(ToolRow::CentralHeader);
    if expanded.contains("central") {
        rows.push(ToolRow::CentralItem(CentralField::Config));
        rows.push(ToolRow::CentralItem(CentralField::Prompt));
        rows.push(ToolRow::CentralItem(CentralField::Skills));
        rows.push(ToolRow::CentralItem(CentralField::Agents));
        rows.push(ToolRow::CentralItem(CentralField::Commands));
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
            // Status sub-group (links)
            rows.push(ToolRow::StatusHeader {
                tool_key: key.clone(),
            });
            let status_key = format!("{}:status", key);
            if expanded.contains(&status_key) {
                rows.push(ToolRow::LinkItem {
                    tool_key: key.clone(),
                    field: LinkField::Prompt,
                });
                rows.push(ToolRow::LinkItem {
                    tool_key: key.clone(),
                    field: LinkField::Skills,
                });
                rows.push(ToolRow::LinkItem {
                    tool_key: key.clone(),
                    field: LinkField::Agents,
                });
                rows.push(ToolRow::LinkItem {
                    tool_key: key.clone(),
                    field: LinkField::Commands,
                });
            }
            // File groups
            for (files, group) in [
                (&tool.settings, FileGroup::Settings),
                (&tool.auth, FileGroup::Auth),
                (&tool.mcp, FileGroup::Mcp),
            ] {
                if !files.is_empty() {
                    rows.push(ToolRow::FileGroupHeader {
                        tool_key: key.clone(),
                        group: group.clone(),
                    });
                    if files.len() > 1 {
                        let gk = format!("{}:{}", key, group_key_suffix(&group));
                        if expanded.contains(&gk) {
                            for i in 0..files.len() {
                                rows.push(ToolRow::FileItem {
                                    tool_key: key.clone(),
                                    group: group.clone(),
                                    index: i,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    rows
}

/// Extract a [tools.{key}] section from raw config text.
/// Returns (section_lines, start_line_index, end_line_index).
fn extract_tool_section(config_text: &str, tool_key: &str) -> Option<(Vec<String>, usize, usize)> {
    let header = format!("[tools.{}]", tool_key);
    let lines: Vec<&str> = config_text.lines().collect();

    let start = lines.iter().position(|l| l.trim() == header)?;

    let sub_prefix = format!("[tools.{}.", tool_key);
    let mut end = lines.len();
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && !trimmed.starts_with(&sub_prefix) {
            end = i;
            break;
        }
    }

    let section_lines: Vec<String> = lines[start..end].iter().map(|l| l.to_string()).collect();
    Some((section_lines, start, end))
}

/// Replace a [tools.{key}] section in raw config text with new content.
fn replace_tool_section(config_text: &str, tool_key: &str, new_section: &str) -> Option<String> {
    let header = format!("[tools.{}]", tool_key);
    let lines: Vec<&str> = config_text.lines().collect();

    let start = lines.iter().position(|l| l.trim() == header)?;

    let sub_prefix = format!("[tools.{}.", tool_key);
    let mut end = lines.len();
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && !trimmed.starts_with(&sub_prefix) {
            end = i;
            break;
        }
    }

    let mut result: Vec<&str> = Vec::new();
    result.extend_from_slice(&lines[..start]);
    for line in new_section.lines() {
        result.push(line);
    }
    result.extend_from_slice(&lines[end..]);

    Some(result.join("\n"))
}

// ---------------------------------------------------------------------------
// Popup state
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) struct LinkContext {
    tool_key: String,
    field: LinkField,
}

pub enum PopupState {
    Log(super::popup::ScrollablePopup),
    Info {
        popup: super::popup::ScrollablePopup,
        editor_path: Option<PathBuf>,
        link_context: Option<LinkContext>,
    },
    PathEditor {
        field: CentralField,
        value: String,
        cursor_pos: usize,
    },
    ConfirmCreate {
        path: PathBuf,
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
    pending_editor_path: Option<PathBuf>,
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
            pending_editor_path: None,
        }
    }

    fn rebuild_rows(&mut self) {
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
        if self.rows.is_empty() {
            return;
        }
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

    // ------------------------------------------------------------------
    // Key handling
    // ------------------------------------------------------------------

    fn handle_key(
        &mut self,
        code: KeyCode,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        area_height: u16,
    ) {
        if self.popup.is_some() {
            self.handle_popup_key(code, terminal);
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
                    self.expanded.insert(format!("{}:status", key));
                    let tool = &self.config.tools[key];
                    if tool.settings.len() > 1 {
                        self.expanded.insert(format!("{}:settings", key));
                    }
                    if tool.auth.len() > 1 {
                        self.expanded.insert(format!("{}:auth", key));
                    }
                    if tool.mcp.len() > 1 {
                        self.expanded.insert(format!("{}:mcp", key));
                    }
                }
                self.rebuild_rows();
            }

            // Primary action: space/enter
            KeyCode::Char(' ') | KeyCode::Enter => {
                if let Some(row) = self.current_row().cloned() {
                    match &row {
                        ToolRow::CentralHeader => self.toggle_expanded("central"),
                        ToolRow::ToolHeader { key, .. } => self.toggle_expanded(key),
                        ToolRow::StatusHeader { tool_key } => {
                            let sk = format!("{}:status", tool_key);
                            self.toggle_expanded(&sk);
                        }
                        ToolRow::CentralItem(
                            ref cf @ (CentralField::Skills
                            | CentralField::Agents
                            | CentralField::Commands
                            | CentralField::Source),
                        ) => {
                            let current_value = match cf {
                                CentralField::Skills => self.config.central.skills_source.clone(),
                                CentralField::Agents => self.config.central.agents_source.clone(),
                                CentralField::Commands => self.config.central.commands_source.clone(),
                                CentralField::Source => self.config.central.source_dir.clone(),
                                _ => unreachable!(),
                            };
                            let len = current_value.len();
                            self.popup = Some(PopupState::PathEditor {
                                field: cf.clone(),
                                value: current_value,
                                cursor_pos: len,
                            });
                        }
                        ToolRow::CentralItem(CentralField::Config) => {
                            self.show_central_info(&CentralField::Config);
                        }
                        ToolRow::CentralItem(CentralField::Prompt) => {
                            self.show_central_info(&CentralField::Prompt);
                        }
                        ToolRow::LinkItem { tool_key, field } => {
                            self.show_link_info(&tool_key.clone(), &field.clone());
                        }
                        ToolRow::FileGroupHeader { tool_key, group } => {
                            let files = self.get_group_files(tool_key, group);
                            if files.len() <= 1 {
                                self.show_file_info(&tool_key.clone(), &group.clone(), 0);
                            } else {
                                let gk = format!("{}:{}", tool_key, group_key_suffix(group));
                                self.toggle_expanded(&gk);
                            }
                        }
                        ToolRow::FileItem {
                            tool_key,
                            group,
                            index,
                        } => {
                            self.show_file_info(&tool_key.clone(), &group.clone(), *index);
                        }
                    }
                }
            }

            // Install/link action: i
            KeyCode::Char('i') => {
                if let Some(row) = self.current_row().cloned() {
                    match &row {
                        ToolRow::StatusHeader { tool_key } => {
                            self.toggle_all_links(&tool_key.clone());
                        }
                        ToolRow::LinkItem { tool_key, field } => {
                            self.toggle_link(&tool_key.clone(), &field.clone());
                        }
                        _ => {}
                    }
                }
            }

            // Edit — open editor
            KeyCode::Char('e') => {
                if let Some(row) = self.current_row().cloned() {
                    self.handle_edit(&row, terminal);
                }
            }

            // Log popup
            KeyCode::Char('l') => {
                let lines = self.log.to_lines();
                let mut popup =
                    super::popup::ScrollablePopup::new("Log", lines).with_close_hint("l:close");
                popup.scroll_offset = popup.lines.len().saturating_sub(1);
                self.popup = Some(PopupState::Log(popup));
            }

            // Quit
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,

            _ => {}
        }
    }

    fn handle_popup_key(
        &mut self,
        code: KeyCode,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) {
        // Determine popup type first
        let is_log = matches!(&self.popup, Some(PopupState::Log(_)));
        let is_info = matches!(&self.popup, Some(PopupState::Info { .. }));
        let is_path = matches!(&self.popup, Some(PopupState::PathEditor { .. }));
        let is_confirm = matches!(&self.popup, Some(PopupState::ConfirmCreate { .. }));

        if is_log {
            if let Some(PopupState::Log(ref mut popup)) = self.popup {
                match code {
                    KeyCode::Char('l') | KeyCode::Esc => self.popup = None,
                    _ => {
                        popup.handle_key(code);
                    }
                }
            }
        } else if is_info {
            self.handle_info_popup_key(code);
        } else if is_path {
            if let Some(PopupState::PathEditor {
                ref field,
                ref mut value,
                ref mut cursor_pos,
            }) = self.popup
            {
                match code {
                    KeyCode::Char(c) => {
                        value.insert(*cursor_pos, c);
                        *cursor_pos += 1;
                    }
                    KeyCode::Backspace => {
                        if *cursor_pos > 0 {
                            value.remove(*cursor_pos - 1);
                            *cursor_pos -= 1;
                        }
                    }
                    KeyCode::Delete => {
                        if *cursor_pos < value.len() {
                            value.remove(*cursor_pos);
                        }
                    }
                    KeyCode::Left => *cursor_pos = cursor_pos.saturating_sub(1),
                    KeyCode::Right => *cursor_pos = (*cursor_pos + 1).min(value.len()),
                    KeyCode::Home => *cursor_pos = 0,
                    KeyCode::End => *cursor_pos = value.len(),
                    KeyCode::Enter => {
                        let field_clone = field.clone();
                        let value_clone = value.clone();
                        self.popup = None;
                        self.save_central_path(field_clone, value_clone);
                    }
                    KeyCode::Esc => self.popup = None,
                    _ => {}
                }
            }
        } else if is_confirm {
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    if let Some(PopupState::ConfirmCreate { ref path }) = self.popup {
                        let path_clone = path.clone();
                        self.popup = None;
                        if let Some(parent) = path_clone.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Err(e) = std::fs::write(&path_clone, "") {
                            self.set_status(format!("Failed to create file: {}", e));
                        } else {
                            self.open_in_editor(terminal, &[path_clone]);
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.popup = None;
                }
                _ => {}
            }
        }
    }

    fn handle_info_popup_key(&mut self, code: KeyCode) {
        // Let ScrollablePopup handle scrolling first
        if let Some(PopupState::Info { ref mut popup, .. }) = self.popup {
            match popup.handle_key(code) {
                super::popup::PopupAction::Close => {
                    self.popup = None;
                    return;
                }
                super::popup::PopupAction::Consumed => return,
                super::popup::PopupAction::Ignored => {} // fall through
            }
        }

        // Extract context for action keys
        let (editor_path, link_ctx) = match &self.popup {
            Some(PopupState::Info {
                editor_path,
                link_context,
                ..
            }) => (editor_path.clone(), link_context.clone()),
            _ => return,
        };

        match code {
            KeyCode::Char('e') => {
                if let Some(path) = editor_path {
                    self.popup = None;
                    self.pending_editor_path = Some(path);
                }
            }
            KeyCode::Char('i') => {
                if let Some(ctx) = link_ctx {
                    let tk = ctx.tool_key.clone();
                    let f = ctx.field.clone();
                    self.popup = None;
                    self.toggle_link(&tk, &f);
                    self.show_link_info(&tk, &f);
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // Link operations
    // ------------------------------------------------------------------

    fn get_link_paths(
        &self,
        tool_key: &str,
        field: &LinkField,
    ) -> Option<(PathBuf, PathBuf, bool, &'static str)> {
        let tool = self.config.tools.get(tool_key)?;
        if !tool.is_installed() {
            return None;
        }
        let config_dir = expand_tilde(&tool.config_dir);
        Some(match field {
            LinkField::Prompt => {
                let link = config_dir.join(&tool.prompt_filename);
                let target = expand_tilde(&self.config.central.prompt_source);
                (link, target, false, "prompt")
            }
            LinkField::Skills => {
                let link = config_dir.join(&tool.skills_dir);
                let target = expand_tilde(&self.config.central.skills_source);
                (link, target, true, "skills")
            }
            LinkField::Agents => {
                let link = config_dir.join(&tool.agents_dir);
                let target = expand_tilde(&self.config.central.agents_source);
                (link, target, true, "agents")
            }
            LinkField::Commands => {
                let link = config_dir.join(&tool.commands_dir);
                let target = expand_tilde(&self.config.central.commands_source);
                (link, target, true, "commands")
            }
        })
    }

    fn toggle_link(&mut self, tool_key: &str, field: &LinkField) {
        use super::log::LogLevel;

        let tool = match self.config.tools.get(tool_key) {
            Some(t) => t,
            None => return,
        };

        if !tool.is_installed() {
            self.set_status(format!("{} is not installed", tool_key));
            return;
        }

        let (link_path, target, is_dir, label) = match self.get_link_paths(tool_key, field) {
            Some(v) => v,
            None => return,
        };

        let status = linker::check_link(&link_path, &target, is_dir);
        match status {
            LinkStatus::Linked => match linker::remove_link_quiet(&link_path, label, is_dir) {
                Ok((true, msg)) => {
                    self.log
                        .push(LogLevel::Success, format!("[{}] {}", tool_key, msg));
                    self.recover_after_unlink(tool_key, field, &link_path);
                    self.set_status(format!("✓ {} {} unlinked", tool_key, label));
                }
                Ok((false, msg)) => self.set_status(msg),
                Err(e) => {
                    self.log.push(
                        LogLevel::Error,
                        format!("[{}] Unlink {} failed: {}", tool_key, label, e),
                    );
                    self.set_status(format!("✗ {}", e));
                }
            },
            LinkStatus::Missing | LinkStatus::Broken => {
                match linker::create_link_quiet(&link_path, &target, label, is_dir) {
                    Ok((true, msg)) => {
                        self.log
                            .push(LogLevel::Success, format!("[{}] {}", tool_key, msg));
                        self.set_status(format!("✓ {} {} linked", tool_key, label));
                    }
                    Ok((false, msg)) => self.set_status(msg),
                    Err(e) => {
                        self.log.push(
                            LogLevel::Error,
                            format!("[{}] Link {} failed: {}", tool_key, label, e),
                        );
                        self.set_status(format!("✗ {}", e));
                    }
                }
            }
            LinkStatus::Wrong(_) => {
                if let Err(e) = crate::platform::remove_link(&link_path) {
                    self.log.push(
                        LogLevel::Error,
                        format!("[{}] Failed to remove wrong link: {}", tool_key, e),
                    );
                    self.set_status(format!("✗ Failed to remove wrong link: {}", e));
                    return;
                }
                match linker::create_link_quiet(&link_path, &target, label, is_dir) {
                    Ok((true, msg)) => {
                        self.log.push(
                            LogLevel::Success,
                            format!("[{}] Repaired: {}", tool_key, msg),
                        );
                        self.set_status(format!("✓ {} {} repaired", tool_key, label));
                    }
                    Ok((false, msg)) => self.set_status(msg),
                    Err(e) => {
                        self.log.push(
                            LogLevel::Error,
                            format!("[{}] Repair failed: {}", tool_key, e),
                        );
                        self.set_status(format!("✗ {}", e));
                    }
                }
            }
            LinkStatus::Blocked => {
                self.handle_blocked_link(tool_key, field, &link_path, &target, is_dir, label);
            }
        }
    }

    fn handle_blocked_link(
        &mut self,
        tool_key: &str,
        field: &LinkField,
        link_path: &std::path::Path,
        target: &std::path::Path,
        is_dir: bool,
        label: &str,
    ) {
        use super::log::LogLevel;
        use chrono::Local;

        if is_dir {
            let source_dir = expand_tilde(&self.config.central.source_dir);
            let tool_target = source_dir.join("agm_tools").join(tool_key);
            let central_dir = target;

            let result = match field {
                LinkField::Agents => {
                    let agents_target = tool_target.join("agents");
                    skills::migrate_agents_dir_quiet(
                        link_path,
                        &agents_target,
                        central_dir,
                        tool_key,
                    )
                }
                LinkField::Commands => {
                    let commands_target = tool_target.join("commands");
                    skills::migrate_commands_dir_quiet(
                        link_path,
                        &commands_target,
                        central_dir,
                        tool_key,
                    )
                }
                _ => skills::migrate_tool_dir_quiet(link_path, &tool_target, central_dir, tool_key),
            };

            match result {
                Ok((count, msgs)) => {
                    for m in &msgs {
                        self.log
                            .push(LogLevel::Info, format!("[{}] {}", tool_key, m.trim()));
                    }
                    match linker::create_link_quiet(link_path, target, label, true) {
                        Ok((true, msg)) => {
                            self.log.push(
                                LogLevel::Success,
                                format!("[{}] Migrated {} items, {}", tool_key, count, msg),
                            );
                            self.set_status(format!(
                                "✓ {} {} migrated and linked",
                                tool_key, label
                            ));
                        }
                        Ok((false, msg)) => self.set_status(msg),
                        Err(e) => {
                            self.log.push(
                                LogLevel::Error,
                                format!(
                                    "[{}] Migration succeeded but link failed: {}",
                                    tool_key, e
                                ),
                            );
                            self.set_status(format!("✗ Link failed after migration: {}", e));
                        }
                    }
                }
                Err(e) => {
                    self.log.push(
                        LogLevel::Error,
                        format!("[{}] Migration failed: {}", tool_key, e),
                    );
                    self.set_status(format!("✗ Migration failed: {}", e));
                }
            }
        } else {
            let timestamp = Local::now().format("%Y%m%d_%H%M%S");
            let backup = link_path.with_extension(format!("{}.bak", timestamp));
            match std::fs::rename(link_path, &backup) {
                Ok(()) => {
                    self.log.push(
                        LogLevel::Info,
                        format!(
                            "[{}] Backed up {} to {}",
                            tool_key,
                            label,
                            contract_tilde(&backup)
                        ),
                    );
                    match linker::create_link_quiet(link_path, target, label, false) {
                        Ok((true, msg)) => {
                            self.log
                                .push(LogLevel::Success, format!("[{}] {}", tool_key, msg));
                            self.set_status(format!(
                                "✓ {} {} backed up and linked",
                                tool_key, label
                            ));
                        }
                        Ok((false, msg)) => self.set_status(msg),
                        Err(e) => {
                            let _ = std::fs::rename(&backup, link_path);
                            self.log.push(
                                LogLevel::Error,
                                format!("[{}] Link failed, restored backup: {}", tool_key, e),
                            );
                            self.set_status(format!("✗ Link failed: {}", e));
                        }
                    }
                }
                Err(e) => {
                    self.log.push(
                        LogLevel::Error,
                        format!("[{}] Backup failed: {}", tool_key, e),
                    );
                    self.set_status(format!("✗ Backup failed: {}", e));
                }
            }
        }
    }

    fn cleanup_empty_tool_store(&self, tool_store: &std::path::Path) {
        if !tool_store.exists() {
            return;
        }
        if let Ok(mut entries) = std::fs::read_dir(tool_store) {
            if entries.next().is_none() {
                let _ = std::fs::remove_dir(tool_store);
            }
        }
    }

    fn recover_after_unlink(
        &mut self,
        tool_key: &str,
        field: &LinkField,
        link_path: &std::path::Path,
    ) {
        use super::log::LogLevel;
        let source_dir = expand_tilde(&self.config.central.source_dir);
        let tool_store = source_dir.join("agm_tools").join(tool_key);

        match field {
            LinkField::Skills => {
                if !tool_store.exists() {
                    let _ = std::fs::create_dir_all(link_path);
                    return;
                }
                // Create the tool's skills directory
                if let Err(e) = std::fs::create_dir_all(link_path) {
                    self.log.push(
                        LogLevel::Warning,
                        format!("[{}] Failed to create skills dir: {}", tool_key, e),
                    );
                    return;
                }
                // Move skill directories: any dir in tool_store that
                // is not "agents" and contains SKILL.md
                let mut count = 0usize;
                if let Ok(entries) = std::fs::read_dir(&tool_store) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name == "agents" {
                            continue;
                        }
                        let path = entry.path();
                        if !path.is_dir() {
                            continue;
                        }
                        if !path.join("SKILL.md").exists() {
                            continue;
                        }
                        let dest = link_path.join(&name);
                        if dest.exists() {
                            continue;
                        }
                        if std::fs::rename(&path, &dest).is_ok() {
                            count += 1;
                        }
                    }
                }
                if count > 0 {
                    self.log.push(
                        LogLevel::Info,
                        format!("[{}] Restored {} skill(s)", tool_key, count),
                    );
                }
                self.cleanup_empty_tool_store(&tool_store);
            }
            LinkField::Agents => {
                let agents_store = tool_store.join("agents");
                if agents_store.exists() {
                    // Restore from agm_tools store
                    match std::fs::rename(&agents_store, link_path) {
                        Ok(()) => {
                            self.log.push(
                                LogLevel::Info,
                                format!("[{}] Restored agents directory", tool_key),
                            );
                        }
                        Err(e) => {
                            self.log.push(
                                LogLevel::Warning,
                                format!("[{}] Failed to restore agents: {}", tool_key, e),
                            );
                        }
                    }
                    self.cleanup_empty_tool_store(&tool_store);
                } else {
                    // No store — create empty dir so tool can use it individually
                    match std::fs::create_dir_all(link_path) {
                        Ok(()) => {
                            self.log.push(
                                LogLevel::Info,
                                format!("[{}] Created empty agents directory", tool_key),
                            );
                        }
                        Err(e) => {
                            self.log.push(
                                LogLevel::Warning,
                                format!("[{}] Failed to create agents directory: {}", tool_key, e),
                            );
                        }
                    }
                }
            }
            LinkField::Commands => {
                let commands_store = tool_store.join("commands");
                if commands_store.exists() {
                    match std::fs::rename(&commands_store, link_path) {
                        Ok(()) => {
                            self.log.push(
                                LogLevel::Info,
                                format!("[{}] Restored commands directory", tool_key),
                            );
                        }
                        Err(e) => {
                            self.log.push(
                                LogLevel::Warning,
                                format!("[{}] Failed to restore commands: {}", tool_key, e),
                            );
                        }
                    }
                    self.cleanup_empty_tool_store(&tool_store);
                } else {
                    match std::fs::create_dir_all(link_path) {
                        Ok(()) => {
                            self.log.push(
                                LogLevel::Info,
                                format!("[{}] Created empty commands directory", tool_key),
                            );
                        }
                        Err(e) => {
                            self.log.push(
                                LogLevel::Warning,
                                format!("[{}] Failed to create commands directory: {}", tool_key, e),
                            );
                        }
                    }
                }
            }
            LinkField::Prompt => {
                // Find and restore the most recent .bak backup.
                // Backup naming: link_path.with_extension("{timestamp}.bak")
                // e.g., AGENTS.md → AGENTS.20240101_120000.bak
                if let Some(parent) = link_path.parent() {
                    if let Some(stem) = link_path.file_stem().and_then(|f| f.to_str()) {
                        let prefix = format!("{}.", stem);
                        let mut backups: Vec<_> = std::fs::read_dir(parent)
                            .into_iter()
                            .flatten()
                            .flatten()
                            .filter(|e| {
                                let n = e.file_name().to_string_lossy().to_string();
                                n.starts_with(&prefix)
                                    && n.ends_with(".bak")
                                    && e.path() != *link_path
                            })
                            .collect();
                        // Sort by name descending → most recent timestamp first
                        backups.sort_by_key(|b| std::cmp::Reverse(b.file_name()));
                        if let Some(latest) = backups.first() {
                            let bak_path = latest.path();
                            match std::fs::rename(&bak_path, link_path) {
                                Ok(()) => {
                                    self.log.push(
                                        LogLevel::Info,
                                        format!("[{}] Restored prompt from backup", tool_key),
                                    );
                                }
                                Err(e) => {
                                    self.log.push(
                                        LogLevel::Warning,
                                        format!("[{}] Failed to restore prompt: {}", tool_key, e),
                                    );
                                }
                            }
                        } else {
                            self.log.push(
                                LogLevel::Info,
                                format!("[{}] No prompt backup found to restore", tool_key),
                            );
                        }
                    }
                }
            }
        }
    }

    fn toggle_all_links(&mut self, tool_key: &str) {
        let tool = match self.config.tools.get(tool_key) {
            Some(t) => t,
            None => return,
        };
        if !tool.is_installed() {
            self.set_status(format!("{} is not installed", tool_key));
            return;
        }

        // Check current state of all 3 links
        let fields = [LinkField::Prompt, LinkField::Skills, LinkField::Agents, LinkField::Commands];
        let mut linked_count = 0;
        for f in &fields {
            if let Some((link_path, target, is_dir, _)) = self.get_link_paths(tool_key, f) {
                if matches!(
                    linker::check_link(&link_path, &target, is_dir),
                    LinkStatus::Linked
                ) {
                    linked_count += 1;
                }
            }
        }

        let tk = tool_key.to_string();
        if linked_count == 4 {
            // All linked → unlink all
            for f in &fields {
                if let Some((link_path, target, is_dir, _)) = self.get_link_paths(&tk, f) {
                    if matches!(
                        linker::check_link(&link_path, &target, is_dir),
                        LinkStatus::Linked
                    ) {
                        self.toggle_link(&tk, f);
                    }
                }
            }
        } else {
            // Partially or none linked → link all
            for f in &fields {
                if let Some((link_path, target, is_dir, _)) = self.get_link_paths(&tk, f) {
                    if !matches!(
                        linker::check_link(&link_path, &target, is_dir),
                        LinkStatus::Linked
                    ) {
                        self.toggle_link(&tk, f);
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Info popups
    // ------------------------------------------------------------------

    fn show_central_info(&mut self, field: &CentralField) {
        let (title, path) = match field {
            CentralField::Config => {
                let p = self
                    .config_path
                    .clone()
                    .unwrap_or_else(|| expand_tilde("~/.config/agm/config.toml"));
                ("Config".to_string(), p)
            }
            CentralField::Prompt => {
                let p = expand_tilde(&self.config.central.prompt_source);
                ("Prompt".to_string(), p)
            }
            _ => return,
        };

        let mut content_lines = vec![
            Line::from(vec![
                Span::styled("Path: ", Style::default().fg(Color::DarkGray)),
                Span::raw(contract_tilde(&path)),
            ]),
            Line::from(""),
        ];

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    for line in content.lines().take(super::popup::MAX_CONTENT_LINES) {
                        content_lines.push(Line::from(line.to_string()));
                    }
                }
                Err(e) => {
                    content_lines.push(Line::from(Span::styled(
                        format!("Error reading file: {}", e),
                        Style::default().fg(Color::Red),
                    )));
                }
            }
        } else {
            content_lines.push(Line::from(Span::styled(
                "File not found",
                Style::default().fg(Color::Yellow),
            )));
        }

        let editor_path = if path.exists() { Some(path) } else { None };
        let hint = if editor_path.is_some() {
            "Esc:close  e:edit"
        } else {
            "Esc:close"
        };
        self.popup = Some(PopupState::Info {
            popup: super::popup::ScrollablePopup::new(&title, content_lines).with_close_hint(hint),
            editor_path,
            link_context: None,
        });
    }

    fn show_link_info(&mut self, tool_key: &str, field: &LinkField) {
        let tool = match self.config.tools.get(tool_key) {
            Some(t) => t,
            None => return,
        };
        let _config_dir = expand_tilde(&tool.config_dir);
        let (link_path, target, is_dir, label) = match self.get_link_paths(tool_key, field) {
            Some(v) => v,
            None => return,
        };

        let status = linker::check_link(&link_path, &target, is_dir);
        let status_text = match &status {
            LinkStatus::Linked => "✓ Linked",
            LinkStatus::Missing => "✗ Not linked",
            LinkStatus::Broken => "✗ Broken link",
            LinkStatus::Wrong(_) => "✗ Wrong target",
            LinkStatus::Blocked => "✗ Not linked",
        };
        let status_color = match &status {
            LinkStatus::Linked => Color::Green,
            LinkStatus::Missing | LinkStatus::Blocked => Color::Yellow,
            _ => Color::Red,
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Field:  ", Style::default().fg(Color::DarkGray)),
                Span::raw(label),
            ]),
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled(status_text.to_string(), Style::default().fg(status_color)),
            ]),
            Line::from(vec![
                Span::styled("Link:   ", Style::default().fg(Color::DarkGray)),
                Span::raw(contract_tilde(&link_path)),
            ]),
            Line::from(vec![
                Span::styled("Target: ", Style::default().fg(Color::DarkGray)),
                Span::raw(contract_tilde(&target)),
            ]),
            Line::from(""),
        ];

        if is_dir {
            // Directory: show stats
            let dir_path = if matches!(status, LinkStatus::Linked) {
                &target
            } else {
                &link_path
            };
            if dir_path.exists() {
                let mut entries = Vec::new();
                if let Ok(rd) = std::fs::read_dir(dir_path) {
                    for entry in rd.flatten() {
                        entries.push(entry.file_name().to_string_lossy().to_string());
                    }
                }
                entries.sort();
                lines.push(Line::from(Span::styled(
                    format!("Contents: {} item(s)", entries.len()),
                    Style::default().fg(Color::DarkGray),
                )));
                for e in &entries {
                    lines.push(Line::from(format!("  {}", e)));
                }
            } else {
                lines.push(Line::from(Span::styled(
                    "Directory not found",
                    Style::default().fg(Color::Yellow),
                )));
            }
        } else {
            // File: show content
            let file_path = if matches!(status, LinkStatus::Linked) {
                &target
            } else {
                &link_path
            };
            if file_path.exists() {
                match std::fs::read_to_string(file_path) {
                    Ok(content) => {
                        lines.push(Line::from(Span::styled(
                            "Content:",
                            Style::default().fg(Color::DarkGray),
                        )));
                        for line in content.lines().take(super::popup::MAX_CONTENT_LINES) {
                            lines.push(Line::from(line.to_string()));
                        }
                    }
                    Err(e) => {
                        lines.push(Line::from(Span::styled(
                            format!("Error reading: {}", e),
                            Style::default().fg(Color::Red),
                        )));
                    }
                }
            } else {
                lines.push(Line::from(Span::styled(
                    "File not found",
                    Style::default().fg(Color::Yellow),
                )));
            }
        }

        let title = format!("{} — {}", tool_key, label);
        self.popup = Some(PopupState::Info {
            popup: super::popup::ScrollablePopup::new(&title, lines)
                .with_close_hint("Esc:close  i:toggle link"),
            editor_path: None,
            link_context: Some(LinkContext {
                tool_key: tool_key.to_string(),
                field: field.clone(),
            }),
        });
    }

    fn show_file_info(&mut self, tool_key: &str, group: &FileGroup, index: usize) {
        let files = self.get_group_files(tool_key, group);
        let path = match files.get(index) {
            Some(p) => p.clone(),
            None => return,
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Path: ", Style::default().fg(Color::DarkGray)),
                Span::raw(contract_tilde(&path)),
            ]),
            Line::from(""),
        ];

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    for line in content.lines().take(super::popup::MAX_CONTENT_LINES) {
                        lines.push(Line::from(line.to_string()));
                    }
                }
                Err(e) => {
                    lines.push(Line::from(Span::styled(
                        format!("Error reading: {}", e),
                        Style::default().fg(Color::Red),
                    )));
                }
            }
        } else {
            lines.push(Line::from(Span::styled(
                "File not found",
                Style::default().fg(Color::Yellow),
            )));
        }

        let title = format!("{} — {}", tool_key, group_label(group));
        let editor_path = if path.exists() { Some(path) } else { None };
        let hint = if editor_path.is_some() {
            "Esc:close  e:edit"
        } else {
            "Esc:close"
        };
        self.popup = Some(PopupState::Info {
            popup: super::popup::ScrollablePopup::new(&title, lines).with_close_hint(hint),
            editor_path,
            link_context: None,
        });
    }

    fn get_group_files(&self, tool_key: &str, group: &FileGroup) -> Vec<PathBuf> {
        let tool = match self.config.tools.get(tool_key) {
            Some(t) => t,
            None => return vec![],
        };
        let config_dir = expand_tilde(&tool.config_dir);
        let file_list: &[String] = match group {
            FileGroup::Settings => &tool.settings,
            FileGroup::Auth => &tool.auth,
            FileGroup::Mcp => &tool.mcp,
        };
        file_list
            .iter()
            .map(|f| {
                if std::path::Path::new(f).is_absolute() || f.starts_with('~') {
                    expand_tilde(f)
                } else {
                    config_dir.join(f)
                }
            })
            .collect()
    }

    // ------------------------------------------------------------------
    // Edit
    // ------------------------------------------------------------------

    fn open_in_editor(
        &self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        paths: &[PathBuf],
    ) {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        let ed = editor::get_editor(&self.config);
        let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
        let _ = editor::open_files(&ed, &refs);
        let _ = stdout().execute(EnterAlternateScreen);
        let _ = enable_raw_mode();
        let _ = terminal.clear();
    }

    fn handle_edit(
        &mut self,
        row: &ToolRow,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) {
        match row {
            ToolRow::ToolHeader { key, .. } => {
                use super::log::LogLevel;
                let config_path = self
                    .config_path
                    .clone()
                    .unwrap_or_else(|| expand_tilde("~/.config/agm/config.toml"));
                let config_text = match std::fs::read_to_string(&config_path) {
                    Ok(t) => t,
                    Err(e) => {
                        self.set_status(format!("✗ Failed to read config: {}", e));
                        return;
                    }
                };
                let (section_lines, _, _) = match extract_tool_section(&config_text, key) {
                    Some(v) => v,
                    None => {
                        self.set_status(format!("✗ Section [tools.{}] not found", key));
                        return;
                    }
                };
                let section_text = section_lines.join("\n") + "\n";

                let tmp_path = std::env::temp_dir().join(format!("agm-{}.toml", key));
                if let Err(e) = std::fs::write(&tmp_path, &section_text) {
                    self.set_status(format!("✗ Failed to write temp file: {}", e));
                    return;
                }

                self.open_in_editor(terminal, std::slice::from_ref(&tmp_path));

                let new_section = match std::fs::read_to_string(&tmp_path) {
                    Ok(t) => t,
                    Err(e) => {
                        self.set_status(format!("✗ Failed to read temp file: {}", e));
                        return;
                    }
                };
                let _ = std::fs::remove_file(&tmp_path);

                // Validate TOML syntax
                if new_section.parse::<toml::Value>().is_err() {
                    self.log.push(
                        LogLevel::Error,
                        format!("[{}] Invalid TOML syntax, changes discarded", key),
                    );
                    self.set_status("✗ Invalid TOML, changes discarded");
                    return;
                }

                // Re-read config (editor may have been slow, file may have changed)
                let config_text = match std::fs::read_to_string(&config_path) {
                    Ok(t) => t,
                    Err(e) => {
                        self.set_status(format!("✗ Failed to re-read config: {}", e));
                        return;
                    }
                };

                let new_config_text =
                    match replace_tool_section(&config_text, key, new_section.trim_end()) {
                        Some(t) => t,
                        None => {
                            self.set_status(format!(
                                "✗ Section [tools.{}] not found for replacement",
                                key
                            ));
                            return;
                        }
                    };

                if let Err(e) = std::fs::write(&config_path, &new_config_text) {
                    self.set_status(format!("✗ Failed to write config: {}", e));
                    return;
                }

                // Reload config
                if let Ok(new_config) = Config::load_from(self.config_path.clone()) {
                    self.config = new_config;
                    self.rebuild_rows();
                    self.log
                        .push(LogLevel::Success, format!("[{}] Config updated", key));
                    self.set_status(format!("✓ {} config updated", key));
                } else {
                    self.log.push(
                        LogLevel::Error,
                        format!("[{}] Config reload failed after edit", key),
                    );
                    self.set_status("✗ Config reload failed");
                }
            }
            ToolRow::CentralItem(CentralField::Config) => {
                let path = self
                    .config_path
                    .clone()
                    .unwrap_or_else(|| expand_tilde("~/.config/agm/config.toml"));
                if path.exists() {
                    self.open_in_editor(terminal, &[path]);
                    if let Ok(new_config) = Config::load_from(self.config_path.clone()) {
                        self.config = new_config;
                        self.rebuild_rows();
                    }
                } else {
                    self.set_status(format!("File not found: {}", contract_tilde(&path)));
                }
            }
            ToolRow::CentralItem(CentralField::Prompt) => {
                let path = expand_tilde(&self.config.central.prompt_source);
                if path.exists() {
                    self.open_in_editor(terminal, &[path]);
                } else {
                    self.set_status(format!("File not found: {}", contract_tilde(&path)));
                }
            }
            ToolRow::FileGroupHeader { tool_key, group } => {
                let files = self.get_group_files(tool_key, group);
                if let Some(path) = files.first() {
                    if path.exists() {
                        self.open_in_editor(terminal, std::slice::from_ref(path));
                    } else {
                        self.set_status(format!("File not found: {}", contract_tilde(path)));
                    }
                }
            }
            ToolRow::FileItem {
                tool_key,
                group,
                index,
            } => {
                let files = self.get_group_files(tool_key, group);
                if let Some(path) = files.get(*index) {
                    if path.exists() {
                        self.open_in_editor(terminal, std::slice::from_ref(path));
                    } else {
                        self.set_status(format!("File not found: {}", contract_tilde(path)));
                    }
                }
            }
            ToolRow::LinkItem {
                tool_key,
                field: LinkField::Prompt,
            } => {
                if let Some((link_path, target, _, _)) =
                    self.get_link_paths(tool_key, &LinkField::Prompt)
                {
                    let status = linker::check_link(&link_path, &target, false);
                    let path = match status {
                        LinkStatus::Linked => target,
                        _ => link_path,
                    };
                    if path.exists() {
                        self.open_in_editor(terminal, &[path]);
                    } else {
                        self.popup = Some(PopupState::ConfirmCreate { path });
                    }
                }
            }
            _ => {}
        }
    }

    fn save_central_path(&mut self, field: CentralField, value: String) {
        use super::log::LogLevel;
        let contracted = contract_tilde(&expand_tilde(&value));
        match field {
            CentralField::Skills => self.config.central.skills_source = contracted.clone(),
            CentralField::Agents => self.config.central.agents_source = contracted.clone(),
            CentralField::Commands => self.config.central.commands_source = contracted.clone(),
            CentralField::Source => self.config.central.source_dir = contracted.clone(),
            _ => return,
        };
        let save_result = if let Some(ref path) = self.config_path {
            self.config.save_to(path)
        } else {
            self.config.save()
        };
        match save_result {
            Ok(()) => {
                let expanded_path = expand_tilde(&contracted);
                if expanded_path.exists() {
                    self.log
                        .push(LogLevel::Success, format!("Updated path: {}", contracted));
                    self.set_status(format!("✓ Path updated: {}", contracted));
                } else {
                    self.log.push(
                        LogLevel::Warning,
                        format!("Updated path (not found): {}", contracted),
                    );
                    self.set_status(format!("⚠ Path updated but does not exist: {}", contracted));
                }
            }
            Err(e) => {
                self.log
                    .push(LogLevel::Error, format!("Failed to save config: {}", e));
                self.set_status(format!("✗ Save failed: {}", e));
            }
        }
        self.rebuild_rows();
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

fn resolve_display(tool: &crate::config::ToolConfig, path: &str) -> String {
    if std::path::Path::new(path).is_absolute() || path.starts_with('~') {
        contract_tilde(&expand_tilde(path))
    } else {
        let full = expand_tilde(&tool.config_dir).join(path);
        contract_tilde(&full)
    }
}

fn link_status_spans(status: &LinkStatus, link_path: &std::path::Path) -> Vec<Span<'static>> {
    match status {
        LinkStatus::Linked => vec![
            Span::styled("✓ linked", Style::default().fg(Color::Green)),
            Span::raw(format!(" → {}", contract_tilde(link_path))),
        ],
        LinkStatus::Missing => vec![
            Span::styled("✗ not linked", Style::default().fg(Color::Yellow)),
            Span::raw(format!(" → {}", contract_tilde(link_path))),
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
            Span::styled("✗ not linked", Style::default().fg(Color::Yellow)),
            Span::raw(format!(" → {}", contract_tilde(link_path))),
        ],
    }
}

fn compute_tool_status(config: &Config, tool_key: &str) -> (u8, &'static str, Color) {
    let tool = match config.tools.get(tool_key) {
        Some(t) => t,
        None => return (0, "Not linked", Color::DarkGray),
    };
    if !tool.is_installed() {
        return (0, "Not installed", Color::DarkGray);
    }
    let config_dir = expand_tilde(&tool.config_dir);
    let mut linked = 0u8;
    for (link_sub, target_src, is_dir) in [
        (&tool.prompt_filename, &config.central.prompt_source, false),
        (&tool.skills_dir, &config.central.skills_source, true),
        (&tool.agents_dir, &config.central.agents_source, true),
    ] {
        let link_path = config_dir.join(link_sub);
        let target = expand_tilde(target_src);
        if matches!(
            linker::check_link(&link_path, &target, is_dir),
            LinkStatus::Linked
        ) {
            linked += 1;
        }
    }
    match linked {
        3 => (3, "All linked", Color::Green),
        0 => (0, "Not linked", Color::DarkGray),
        _ => (linked, "Partially linked", Color::Yellow),
    }
}

fn hint_key(k: &str) -> Span<'static> {
    Span::styled(k.to_string(), Style::default().fg(Color::Yellow))
}

fn hint_text(t: &str) -> Span<'static> {
    Span::raw(t.to_string())
}

fn build_tool_hints(row: Option<&ToolRow>, config: &Config) -> Line<'static> {
    let mut spans = Vec::new();
    match row {
        Some(ToolRow::CentralHeader) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" toggle  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        Some(ToolRow::ToolHeader { .. }) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" toggle  ")]);
            spans.extend([hint_key("e"), hint_text(" edit  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        Some(ToolRow::CentralItem(CentralField::Config))
        | Some(ToolRow::CentralItem(CentralField::Prompt)) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" info  ")]);
            spans.extend([hint_key("e"), hint_text(" edit  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        Some(ToolRow::CentralItem(_)) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" edit path  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        Some(ToolRow::StatusHeader { .. }) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" toggle  ")]);
            spans.extend([hint_key("i"), hint_text(" link  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        Some(ToolRow::LinkItem {
            field: LinkField::Prompt,
            ..
        }) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" info  ")]);
            spans.extend([hint_key("e"), hint_text(" edit  ")]);
            spans.extend([hint_key("i"), hint_text(" link  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        Some(ToolRow::LinkItem { .. }) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" info  ")]);
            spans.extend([hint_key("i"), hint_text(" link  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        Some(ToolRow::FileGroupHeader { tool_key, group }) => {
            let files: &[String] = match config.tools.get(tool_key) {
                Some(t) => match group {
                    FileGroup::Settings => &t.settings,
                    FileGroup::Auth => &t.auth,
                    FileGroup::Mcp => &t.mcp,
                },
                None => &[],
            };
            if files.len() <= 1 {
                spans.extend([hint_key("␣/⏎"), hint_text(" info  ")]);
                spans.extend([hint_key("e"), hint_text(" edit  ")]);
            } else {
                spans.extend([hint_key("␣/⏎"), hint_text(" toggle  ")]);
            }
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        Some(ToolRow::FileItem { .. }) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" info  ")]);
            spans.extend([hint_key("e"), hint_text(" edit  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        None => {
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
    }
    Line::from(spans)
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
        Some(PopupState::Info { ref mut popup, .. }) => popup.render(frame, frame.area()),
        Some(PopupState::PathEditor { .. }) => render_path_editor(app, frame, frame.area()),
        Some(PopupState::ConfirmCreate { .. }) => render_confirm_create(app, frame, frame.area()),
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
        let p = Paragraph::new("No tools configured.").style(Style::default().fg(Color::DarkGray));
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

fn render_row(
    row: &ToolRow,
    is_cursor: bool,
    config: &Config,
    expanded: &HashSet<String>,
) -> Line<'static> {
    let cursor_prefix = if is_cursor { "▸ " } else { "  " };

    match row {
        ToolRow::CentralHeader => {
            let arrow = if expanded.contains("central") {
                "▼"
            } else {
                "▶"
            };
            let style = if is_cursor {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            };
            Line::from(Span::styled(
                format!("{}{} central", cursor_prefix, arrow),
                style,
            ))
        }

        ToolRow::CentralItem(field) => {
            let (label, value) = match field {
                CentralField::Config => (
                    "config".to_string(),
                    "~/.config/agm/config.toml".to_string(),
                ),
                CentralField::Prompt => (
                    "prompt".to_string(),
                    contract_tilde(&expand_tilde(&config.central.prompt_source)),
                ),
                CentralField::Skills => (
                    "skills".to_string(),
                    contract_tilde(&expand_tilde(&config.central.skills_source)),
                ),
                CentralField::Agents => (
                    "agents".to_string(),
                    contract_tilde(&expand_tilde(&config.central.agents_source)),
                ),
                CentralField::Commands => (
                    "commands".to_string(),
                    contract_tilde(&expand_tilde(&config.central.commands_source)),
                ),
                CentralField::Source => (
                    "source".to_string(),
                    contract_tilde(&expand_tilde(&config.central.source_dir)),
                ),
            };
            let style = if is_cursor {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(vec![
                Span::raw(format!("{}    ", cursor_prefix)),
                Span::styled(
                    format!("{:<8}", label),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(value, style),
            ])
        }

        ToolRow::ToolHeader {
            key,
            name,
            installed,
        } => {
            let arrow = if expanded.contains(key) { "▼" } else { "▶" };
            let status = if *installed { "" } else { " (not installed)" };
            let style = if is_cursor {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if !installed {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            };
            Line::from(Span::styled(
                format!("{}{} {} ({}){}", cursor_prefix, arrow, key, name, status),
                style,
            ))
        }

        ToolRow::StatusHeader { tool_key } => {
            let status_key = format!("{}:status", tool_key);
            let arrow = if expanded.contains(&status_key) {
                "▼"
            } else {
                "▶"
            };
            let (_, status_text, status_color) = compute_tool_status(config, tool_key);
            let spans = vec![
                Span::raw(format!("{}    {} ", cursor_prefix, arrow)),
                Span::styled("status", Style::default().fg(Color::DarkGray)),
                Span::raw("   "),
                Span::styled(status_text.to_string(), Style::default().fg(status_color)),
            ];
            if is_cursor {
                Line::from(spans).style(Style::default().fg(Color::Yellow))
            } else {
                Line::from(spans)
            }
        }

        ToolRow::LinkItem { tool_key, field } => {
            let tool = match config.tools.get(tool_key) {
                Some(t) => t,
                None => return Line::from(""),
            };
            let config_dir = expand_tilde(&tool.config_dir);
            let (link_path, target, is_dir, label) = match field {
                LinkField::Prompt => {
                    let link = config_dir.join(&tool.prompt_filename);
                    let target = expand_tilde(&config.central.prompt_source);
                    (link, target, false, "prompt")
                }
                LinkField::Skills => {
                    let link = config_dir.join(&tool.skills_dir);
                    let target = expand_tilde(&config.central.skills_source);
                    (link, target, true, "skills")
                }
                LinkField::Agents => {
                    let link = config_dir.join(&tool.agents_dir);
                    let target = expand_tilde(&config.central.agents_source);
                    (link, target, true, "agents")
                }
                LinkField::Commands => {
                    let link = config_dir.join(&tool.commands_dir);
                    let target = expand_tilde(&config.central.commands_source);
                    (link, target, true, "commands")
                }
            };
            let status = linker::check_link(&link_path, &target, is_dir);
            let status_spans = link_status_spans(&status, &link_path);
            let mut spans = vec![
                Span::raw(format!("{}      ", cursor_prefix)),
                Span::styled(
                    format!("{:<8} ", label),
                    Style::default().fg(Color::DarkGray),
                ),
            ];
            spans.extend(status_spans);
            if is_cursor {
                Line::from(spans).style(Style::default().fg(Color::Yellow))
            } else {
                Line::from(spans)
            }
        }

        ToolRow::FileGroupHeader { tool_key, group } => {
            let tool = match config.tools.get(tool_key) {
                Some(t) => t,
                None => return Line::from(""),
            };
            let label = group_label(group);
            let files: &[String] = match group {
                FileGroup::Settings => &tool.settings,
                FileGroup::Auth => &tool.auth,
                FileGroup::Mcp => &tool.mcp,
            };

            if files.len() <= 1 {
                // Single file: inline display
                let display = files
                    .first()
                    .map(|f| resolve_display(tool, f))
                    .unwrap_or_default();
                let style = if is_cursor {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                Line::from(vec![
                    Span::raw(format!("{}    ", cursor_prefix)),
                    Span::styled(
                        format!("{:<8} ", label),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(display, style),
                ])
            } else {
                // Multi file: expandable
                let gk = format!("{}:{}", tool_key, group_key_suffix(group));
                let arrow = if expanded.contains(&gk) { "▼" } else { "▶" };
                let style = if is_cursor {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                Line::from(vec![
                    Span::raw(format!("{}    {} ", cursor_prefix, arrow)),
                    Span::styled(label.to_string(), style),
                ])
            }
        }

        ToolRow::FileItem {
            tool_key,
            group,
            index,
        } => {
            let tool = match config.tools.get(tool_key) {
                Some(t) => t,
                None => return Line::from(""),
            };
            let files: &[String] = match group {
                FileGroup::Settings => &tool.settings,
                FileGroup::Auth => &tool.auth,
                FileGroup::Mcp => &tool.mcp,
            };
            let display = files
                .get(*index)
                .map(|f| resolve_display(tool, f))
                .unwrap_or_default();
            let style = if is_cursor {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::raw(format!("{}      ", cursor_prefix)),
                Span::styled(display, style),
            ])
        }
    }
}

fn render_footer(app: &ToolApp, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let hints = build_tool_hints(app.current_row(), &app.config);

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

fn render_path_editor(app: &ToolApp, frame: &mut Frame, area: Rect) {
    if let Some(PopupState::PathEditor {
        ref field,
        ref value,
        cursor_pos,
    }) = app.popup
    {
        let popup_area = super::dialog_area(area, 3);
        frame.render_widget(Clear, popup_area);
        let label = match field {
            CentralField::Skills => "skills",
            CentralField::Agents => "agents",
            CentralField::Commands => "commands",
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
            spans.push(Span::styled(
                " ",
                Style::default().fg(Color::Black).bg(Color::White),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), inner);
    }
}

fn render_confirm_create(app: &ToolApp, frame: &mut Frame, area: Rect) {
    if let Some(PopupState::ConfirmCreate { ref path }) = app.popup {
        let popup_area = super::dialog_area(area, 3);
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Create File ")
            .title_bottom(" y:create  n/Esc:cancel ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let text = format!("Create file: {}", contract_tilde(path));
        let paragraph = Paragraph::new(text).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

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

        // Process pending editor from popup
        if let Some(path) = app.pending_editor_path.take() {
            app.open_in_editor(&mut terminal, &[path]);
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
                commands_source: "~/.local/share/agm/commands".to_string(),
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
            settings: if with_optional {
                vec!["settings.json".to_string()]
            } else {
                vec![]
            },
            auth: if with_optional {
                vec!["auth.json".to_string()]
            } else {
                vec![]
            },
            prompt_filename: "PROMPT.md".to_string(),
            skills_dir: "skills".to_string(),
            agents_dir: "agents".to_string(),
            commands_dir: "commands".to_string(),
            mcp: if with_optional {
                vec!["mcp.json".to_string()]
            } else {
                vec![]
            },
        }
    }

    #[test]
    fn test_build_rows_all_collapsed() {
        let config = test_config_with_tools(vec![
            (
                "claude",
                test_tool_config("Claude Code", "/nonexistent/claude", true),
            ),
            (
                "copilot",
                test_tool_config("Copilot CLI", "/nonexistent/copilot", true),
            ),
        ]);
        let expanded = HashSet::new();
        let rows = build_rows(&config, &expanded);

        assert_eq!(rows.len(), 3);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::ToolHeader { ref key, .. } if key == "claude"));
        assert!(matches!(rows[2], ToolRow::ToolHeader { ref key, .. } if key == "copilot"));
    }

    #[test]
    fn test_build_rows_central_expanded() {
        let config = test_config_with_tools(vec![(
            "claude",
            test_tool_config("Claude Code", "/nonexistent/claude", true),
        )]);
        let mut expanded = HashSet::new();
        expanded.insert("central".to_string());
        let rows = build_rows(&config, &expanded);

        assert_eq!(rows.len(), 8);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(
            rows[1],
            ToolRow::CentralItem(CentralField::Config)
        ));
        assert!(matches!(
            rows[2],
            ToolRow::CentralItem(CentralField::Prompt)
        ));
        assert!(matches!(
            rows[3],
            ToolRow::CentralItem(CentralField::Skills)
        ));
        assert!(matches!(
            rows[4],
            ToolRow::CentralItem(CentralField::Agents)
        ));
        assert!(matches!(
            rows[5],
            ToolRow::CentralItem(CentralField::Commands)
        ));
        assert!(matches!(
            rows[6],
            ToolRow::CentralItem(CentralField::Source)
        ));
        assert!(matches!(rows[7], ToolRow::ToolHeader { ref key, .. } if key == "claude"));
    }

    #[test]
    fn test_build_rows_tool_expanded() {
        let _tmp = TempDir::new().unwrap();
        let tool_dir = _tmp.path().join("claude");
        std::fs::create_dir_all(&tool_dir).unwrap();

        let config = test_config_with_tools(vec![(
            "claude",
            test_tool_config("Claude Code", &tool_dir.to_string_lossy(), true),
        )]);
        let mut expanded = HashSet::new();
        expanded.insert("claude".to_string());
        let rows = build_rows(&config, &expanded);

        // CentralHeader + ToolHeader + StatusHeader + 3 FileGroupHeaders (settings, auth, mcp single-file)
        // StatusHeader is collapsed so no LinkItems
        assert_eq!(rows.len(), 6);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(
            matches!(rows[1], ToolRow::ToolHeader { ref key, installed: true, .. } if key == "claude")
        );
        assert!(matches!(rows[2], ToolRow::StatusHeader { ref tool_key } if tool_key == "claude"));
        assert!(
            matches!(rows[3], ToolRow::FileGroupHeader { ref group, .. } if *group == FileGroup::Settings)
        );
        assert!(
            matches!(rows[4], ToolRow::FileGroupHeader { ref group, .. } if *group == FileGroup::Auth)
        );
        assert!(
            matches!(rows[5], ToolRow::FileGroupHeader { ref group, .. } if *group == FileGroup::Mcp)
        );
    }

    #[test]
    fn test_build_rows_tool_with_status_expanded() {
        let _tmp = TempDir::new().unwrap();
        let tool_dir = _tmp.path().join("claude");
        std::fs::create_dir_all(&tool_dir).unwrap();

        let config = test_config_with_tools(vec![(
            "claude",
            test_tool_config("Claude Code", &tool_dir.to_string_lossy(), true),
        )]);
        let mut expanded = HashSet::new();
        expanded.insert("claude".to_string());
        expanded.insert("claude:status".to_string());
        let rows = build_rows(&config, &expanded);

        // CentralHeader + ToolHeader + StatusHeader + 4 LinkItems + Settings + Auth + Mcp
        assert_eq!(rows.len(), 10);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::ToolHeader { .. }));
        assert!(matches!(rows[2], ToolRow::StatusHeader { .. }));
        assert!(
            matches!(rows[3], ToolRow::LinkItem { ref field, .. } if *field == LinkField::Prompt)
        );
        assert!(
            matches!(rows[4], ToolRow::LinkItem { ref field, .. } if *field == LinkField::Skills)
        );
        assert!(
            matches!(rows[5], ToolRow::LinkItem { ref field, .. } if *field == LinkField::Agents)
        );
        assert!(
            matches!(rows[6], ToolRow::LinkItem { ref field, .. } if *field == LinkField::Commands)
        );
        assert!(
            matches!(rows[7], ToolRow::FileGroupHeader { ref group, .. } if *group == FileGroup::Settings)
        );
        assert!(
            matches!(rows[8], ToolRow::FileGroupHeader { ref group, .. } if *group == FileGroup::Auth)
        );
        assert!(
            matches!(rows[9], ToolRow::FileGroupHeader { ref group, .. } if *group == FileGroup::Mcp)
        );
    }

    #[test]
    fn test_build_rows_empty_vec_skipped() {
        let _tmp = TempDir::new().unwrap();
        let tool_dir = _tmp.path().join("minimal");
        std::fs::create_dir_all(&tool_dir).unwrap();

        let config = test_config_with_tools(vec![(
            "minimal",
            test_tool_config("Minimal Tool", &tool_dir.to_string_lossy(), false),
        )]);
        let mut expanded = HashSet::new();
        expanded.insert("minimal".to_string());
        expanded.insert("minimal:status".to_string());
        let rows = build_rows(&config, &expanded);

        // CentralHeader + ToolHeader + StatusHeader + 4 LinkItems (no file groups since empty)
        assert_eq!(rows.len(), 7);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::ToolHeader { ref key, .. } if key == "minimal"));
        assert!(matches!(rows[2], ToolRow::StatusHeader { .. }));
        assert!(
            matches!(rows[3], ToolRow::LinkItem { ref field, .. } if *field == LinkField::Prompt)
        );
        assert!(
            matches!(rows[4], ToolRow::LinkItem { ref field, .. } if *field == LinkField::Skills)
        );
        assert!(
            matches!(rows[5], ToolRow::LinkItem { ref field, .. } if *field == LinkField::Agents)
        );
        assert!(
            matches!(rows[6], ToolRow::LinkItem { ref field, .. } if *field == LinkField::Commands)
        );

        // No FileGroupHeader rows
        for row in &rows {
            assert!(!matches!(row, ToolRow::FileGroupHeader { .. }));
        }
    }

    #[test]
    fn test_build_rows_alphabetical() {
        let config = test_config_with_tools(vec![
            (
                "zed",
                test_tool_config("Zed Editor", "/nonexistent/zed", true),
            ),
            (
                "alpha",
                test_tool_config("Alpha Tool", "/nonexistent/alpha", true),
            ),
        ]);
        let expanded = HashSet::new();
        let rows = build_rows(&config, &expanded);

        assert_eq!(rows.len(), 3);
        assert!(matches!(rows[0], ToolRow::CentralHeader));
        assert!(matches!(rows[1], ToolRow::ToolHeader { ref key, .. } if key == "alpha"));
        assert!(matches!(rows[2], ToolRow::ToolHeader { ref key, .. } if key == "zed"));
    }

    #[test]
    fn test_build_rows_multi_file_expansion() {
        let _tmp = TempDir::new().unwrap();
        let tool_dir = _tmp.path().join("claude");
        std::fs::create_dir_all(&tool_dir).unwrap();

        let tool = ToolConfig {
            name: "Claude".to_string(),
            config_dir: tool_dir.to_string_lossy().to_string(),
            settings: vec![
                "a.json".to_string(),
                "b.json".to_string(),
                "c.json".to_string(),
            ],
            auth: vec!["cred.json".to_string()],
            prompt_filename: "PROMPT.md".to_string(),
            skills_dir: "skills".to_string(),
            agents_dir: "agents".to_string(),
            commands_dir: "commands".to_string(),
            mcp: vec!["mcp.json".to_string()],
        };
        let config = test_config_with_tools(vec![("claude", tool)]);
        let mut expanded = HashSet::new();
        expanded.insert("claude".to_string());
        expanded.insert("claude:settings".to_string());
        let rows = build_rows(&config, &expanded);

        // CentralHeader + ToolHeader + StatusHeader + FileGroupHeader(settings) + 3 FileItems + FileGroupHeader(auth) + FileGroupHeader(mcp)
        assert_eq!(rows.len(), 9);
        assert!(
            matches!(rows[3], ToolRow::FileGroupHeader { ref group, .. } if *group == FileGroup::Settings)
        );
        assert!(
            matches!(rows[4], ToolRow::FileItem { ref group, index: 0, .. } if *group == FileGroup::Settings)
        );
        assert!(
            matches!(rows[5], ToolRow::FileItem { ref group, index: 1, .. } if *group == FileGroup::Settings)
        );
        assert!(
            matches!(rows[6], ToolRow::FileItem { ref group, index: 2, .. } if *group == FileGroup::Settings)
        );
        assert!(
            matches!(rows[7], ToolRow::FileGroupHeader { ref group, .. } if *group == FileGroup::Auth)
        );
        assert!(
            matches!(rows[8], ToolRow::FileGroupHeader { ref group, .. } if *group == FileGroup::Mcp)
        );
    }

    const SAMPLE_CONFIG: &str = r#"[central]
prompt_source = "~/.local/share/agm/prompts/MASTER.md"
skills_source = "~/.local/share/agm/skills"

[tools.claude]
name = "Claude Code"
config_dir = "~/.claude"
prompt_filename = "CLAUDE.md"
skills_dir = "skills"

[tools.codex]
name = "Codex"
config_dir = "~/.codex"
prompt_filename = "AGENTS.md"
skills_dir = "skills"

[tools.copilot]
name = "Copilot"
config_dir = "~/.copilot"
"#;

    #[test]
    fn test_extract_tool_section() {
        let result = extract_tool_section(SAMPLE_CONFIG, "codex");
        assert!(result.is_some());
        let (section, start, end) = result.unwrap();
        assert!(section[0].contains("[tools.codex]"));
        assert!(start < end);
    }

    #[test]
    fn test_extract_tool_section_first_tool() {
        let result = extract_tool_section(SAMPLE_CONFIG, "claude");
        assert!(result.is_some());
        let (section, _, _) = result.unwrap();
        assert!(section[0].contains("[tools.claude]"));
        assert!(section.iter().any(|l| l.contains("Claude Code")));
    }

    #[test]
    fn test_extract_tool_section_last_tool() {
        let result = extract_tool_section(SAMPLE_CONFIG, "copilot");
        assert!(result.is_some());
        let (section, _, _) = result.unwrap();
        assert!(section[0].contains("[tools.copilot]"));
    }

    #[test]
    fn test_extract_tool_section_not_found() {
        let result = extract_tool_section(SAMPLE_CONFIG, "nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_replace_tool_section() {
        let new_section = "[tools.codex]\nname = \"OpenAI Codex\"\nconfig_dir = \"~/.codex\"\n";
        let result = replace_tool_section(SAMPLE_CONFIG, "codex", new_section);
        assert!(result.is_some());
        let new_config = result.unwrap();
        assert!(new_config.contains("OpenAI Codex"));
        assert!(!new_config.contains("\"Codex\""));
        assert!(new_config.contains("[tools.claude]"));
        assert!(new_config.contains("[tools.copilot]"));
    }

    #[test]
    fn test_replace_tool_section_not_found() {
        let result = replace_tool_section(SAMPLE_CONFIG, "nonexistent", "whatever");
        assert!(result.is_none());
    }
}
