use std::collections::HashSet;
use std::io::{self, stdout};
use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
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
use crate::skills::{self, SkillInstallStatus, SourceGroup, SourceKind};

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Category {
    Skills,
    Agents,
    Commands,
}

#[derive(Clone)]
enum ListRow {
    CategoryHeader {
        category: Category,
    },
    SourceHeader {
        category: Category,
        group_index: usize,
    },
    SkillItem {
        group_index: usize,
        skill_index: usize,
    },
    AgentItem {
        group_index: usize,
        agent_index: usize,
    },
    CommandItem {
        group_index: usize,
        command_index: usize,
    },
}

#[derive(Clone)]
enum ConfirmState {
    Normal {
        group_index: usize,
    },
    Migrated {
        group_index: usize,
        typed: String,
    },
    BulkToggle {
        group_index: usize,
        category: Category,
        install: bool,
    },
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
    skills_dir: PathBuf,
    agents_dir: PathBuf,
    commands_dir: PathBuf,
    source_dir: PathBuf,
    expanded_categories: HashSet<Category>,
    expanded_skills_sources: HashSet<usize>,
    expanded_agents_sources: HashSet<usize>,
    expanded_commands_sources: HashSet<usize>,
    confirm_state: Option<ConfirmState>,
    matcher: SkimMatcherV2,
    log: super::log::LogBuffer,
    show_log: bool,
    log_popup: Option<super::popup::ScrollablePopup>,
    background_task: Option<super::background::BackgroundTask>,
    info_popup: Option<super::popup::ScrollablePopup>,
}

impl App {
    fn new(
        config: Config,
        groups: Vec<SourceGroup>,
        skills_dir: PathBuf,
        agents_dir: PathBuf,
        commands_dir: PathBuf,
        source_dir: PathBuf,
    ) -> Self {
        let rows = Vec::new();
        let mut app = Self {
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
            skills_dir,
            agents_dir,
            commands_dir,
            source_dir,
            expanded_categories: HashSet::new(),
            expanded_skills_sources: HashSet::new(),
            expanded_agents_sources: HashSet::new(),
            expanded_commands_sources: HashSet::new(),
            confirm_state: None,
            matcher: SkimMatcherV2::default(),
            log: super::log::LogBuffer::new(500),
            show_log: false,
            log_popup: None,
            background_task: None,
            info_popup: None,
        };
        app.rebuild_rows();
        app
    }

    fn rebuild_rows(&mut self) {
        self.rows = build_rows(
            &self.groups,
            &self.expanded_categories,
            &self.expanded_skills_sources,
            &self.expanded_agents_sources,
            &self.expanded_commands_sources,
        );
        self.apply_search_filter();
    }

    fn visible_rows(&self) -> Vec<usize> {
        match &self.filtered_rows {
            Some(indices) => indices.clone(),
            None => (0..self.rows.len()).collect(),
        }
    }

    fn refresh(&mut self) {
        let _ = skills::prune_broken_skills(&self.skills_dir);
        let _ = skills::prune_broken_agents(&self.agents_dir);
        let _ = skills::prune_broken_commands(&self.commands_dir);
        self.groups = skills::scan_all_sources(
            &self.source_dir,
            &self.skills_dir,
            &self.agents_dir,
            &self.commands_dir,
            &self.config.central.source_repos,
        );
        self.rebuild_rows();
        self.clamp_cursor();
    }

    fn clamp_cursor(&mut self) {
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
        let query = &self.search_query;

        // Phase 1: Determine matching groups from self.groups directly
        // (decoupled from expansion state of rows)
        let mut matching_groups_skills: HashSet<usize> = HashSet::new();
        let mut matching_groups_agents: HashSet<usize> = HashSet::new();
        let mut matching_groups_commands: HashSet<usize> = HashSet::new();
        let mut matching_skills: HashSet<(usize, usize)> = HashSet::new();
        let mut matching_agents: HashSet<(usize, usize)> = HashSet::new();
        let mut matching_commands: HashSet<(usize, usize)> = HashSet::new();

        for (gi, group) in self.groups.iter().enumerate() {
            for (si, skill) in group.skills.iter().enumerate() {
                if self.matcher.fuzzy_match(&skill.name, query).is_some() {
                    matching_groups_skills.insert(gi);
                    matching_skills.insert((gi, si));
                }
            }
            for (ai, agent) in group.agents.iter().enumerate() {
                if self.matcher.fuzzy_match(&agent.name, query).is_some() {
                    matching_groups_agents.insert(gi);
                    matching_agents.insert((gi, ai));
                }
            }
            for (ci, command) in group.commands.iter().enumerate() {
                if self.matcher.fuzzy_match(&command.name, query).is_some() {
                    matching_groups_commands.insert(gi);
                    matching_commands.insert((gi, ci));
                }
            }
        }

        // Phase 2: Build filtered row indices from self.rows
        let mut result = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            match row {
                ListRow::CategoryHeader { category } => {
                    let has_matches = match category {
                        Category::Skills => !matching_groups_skills.is_empty(),
                        Category::Agents => !matching_groups_agents.is_empty(),
                        Category::Commands => !matching_groups_commands.is_empty(),
                    };
                    if has_matches {
                        result.push(i);
                    }
                }
                ListRow::SourceHeader {
                    category,
                    group_index,
                } => {
                    let is_match = match category {
                        Category::Skills => matching_groups_skills.contains(group_index),
                        Category::Agents => matching_groups_agents.contains(group_index),
                        Category::Commands => matching_groups_commands.contains(group_index),
                    };
                    if is_match {
                        result.push(i);
                    }
                }
                ListRow::SkillItem {
                    group_index,
                    skill_index,
                } => {
                    if matching_skills.contains(&(*group_index, *skill_index)) {
                        result.push(i);
                    }
                }
                ListRow::AgentItem {
                    group_index,
                    agent_index,
                } => {
                    if matching_agents.contains(&(*group_index, *agent_index)) {
                        result.push(i);
                    }
                }
                ListRow::CommandItem {
                    group_index,
                    command_index,
                } => {
                    if matching_commands.contains(&(*group_index, *command_index)) {
                        result.push(i);
                    }
                }
            }
        }
        self.filtered_rows = Some(result);
    }

    fn toggle_item(&mut self) {
        let row = match self.current_row() {
            Some(r) => r.clone(),
            None => return,
        };
        match row {
            ListRow::CategoryHeader { category } => {
                if self.expanded_categories.contains(&category) {
                    self.expanded_categories.remove(&category);
                } else {
                    self.expanded_categories.insert(category);
                }
                self.rebuild_rows();
                self.clamp_cursor();
            }
            ListRow::SourceHeader {
                category,
                group_index,
            } => {
                let set = match category {
                    Category::Skills => &mut self.expanded_skills_sources,
                    Category::Agents => &mut self.expanded_agents_sources,
                    Category::Commands => &mut self.expanded_commands_sources,
                };
                if set.contains(&group_index) {
                    set.remove(&group_index);
                } else {
                    set.insert(group_index);
                }
                self.rebuild_rows();
                self.clamp_cursor();
            }
            ListRow::SkillItem {
                group_index,
                skill_index,
            } => {
                self.toggle_skill(group_index, skill_index);
            }
            ListRow::AgentItem {
                group_index,
                agent_index,
            } => {
                self.toggle_agent(group_index, agent_index);
            }
            ListRow::CommandItem {
                group_index,
                command_index,
            } => {
                self.toggle_command(group_index, command_index);
            }
        }
    }

    fn toggle_skill(&mut self, group_index: usize, skill_index: usize) {
        let skill = &self.groups[group_index].skills[skill_index];
        let name = skill.name.clone();
        let source_path = skill.source_path.clone();
        match skill.install_status {
            SkillInstallStatus::Installed => {
                match skills::uninstall_skill(&name, &self.skills_dir) {
                    Ok(()) => {
                        self.groups[group_index].skills[skill_index].install_status =
                            SkillInstallStatus::NotInstalled;
                        self.log
                            .push(super::log::LogLevel::Success, format!("Uninstalled {name}"));
                        self.set_status(format!("Uninstalled {name}"));
                    }
                    Err(e) => {
                        self.log
                            .push(super::log::LogLevel::Error, format!("Uninstall error: {e}"));
                        self.set_status(format!("Error: {e}"));
                    }
                }
            }
            SkillInstallStatus::NotInstalled => {
                match skills::install_skill(&name, &source_path, &self.skills_dir) {
                    Ok(()) => {
                        self.groups[group_index].skills[skill_index].install_status =
                            SkillInstallStatus::Installed;
                        self.log
                            .push(super::log::LogLevel::Success, format!("Installed {name}"));
                        self.set_status(format!("Installed {name}"));
                    }
                    Err(e) => {
                        self.log
                            .push(super::log::LogLevel::Error, format!("Install error: {e}"));
                        self.set_status(format!("Error: {e}"));
                    }
                }
            }
            SkillInstallStatus::Conflict => {
                self.log.push(
                    super::log::LogLevel::Warning,
                    format!("Conflict: {name} installed from another source"),
                );
                self.set_status(format!("Conflict: {name} installed from another source"));
            }
        }
    }

    fn toggle_agent(&mut self, group_index: usize, agent_index: usize) {
        let agent = &self.groups[group_index].agents[agent_index];
        let name = agent.name.clone();
        let source_path = agent.source_path.clone();
        match agent.install_status {
            SkillInstallStatus::Installed => {
                match skills::uninstall_agent(&name, &self.agents_dir) {
                    Ok(()) => {
                        self.groups[group_index].agents[agent_index].install_status =
                            SkillInstallStatus::NotInstalled;
                        self.log
                            .push(super::log::LogLevel::Success, format!("Uninstalled {name}"));
                        self.set_status(format!("Uninstalled agent {name}"));
                    }
                    Err(e) => {
                        self.log
                            .push(super::log::LogLevel::Error, format!("Uninstall error: {e}"));
                        self.set_status(format!("Error: {e}"));
                    }
                }
            }
            SkillInstallStatus::NotInstalled => {
                match skills::install_agent(&name, &source_path, &self.agents_dir) {
                    Ok(()) => {
                        self.groups[group_index].agents[agent_index].install_status =
                            SkillInstallStatus::Installed;
                        self.log
                            .push(super::log::LogLevel::Success, format!("Installed {name}"));
                        self.set_status(format!("Installed agent {name}"));
                    }
                    Err(e) => {
                        self.log
                            .push(super::log::LogLevel::Error, format!("Install error: {e}"));
                        self.set_status(format!("Error: {e}"));
                    }
                }
            }
            SkillInstallStatus::Conflict => {
                self.log.push(
                    super::log::LogLevel::Warning,
                    format!("Conflict: agent {name} from another source"),
                );
                self.set_status(format!("Conflict: agent {name} from another source"));
            }
        }
    }

    fn toggle_command(&mut self, group_index: usize, command_index: usize) {
        let command = &self.groups[group_index].commands[command_index];
        let name = command.name.clone();
        let source_path = command.source_path.clone();
        match command.install_status {
            SkillInstallStatus::Installed => {
                match skills::uninstall_command(&name, &self.commands_dir) {
                    Ok(()) => {
                        self.groups[group_index].commands[command_index].install_status =
                            SkillInstallStatus::NotInstalled;
                        self.log
                            .push(super::log::LogLevel::Success, format!("Uninstalled {name}"));
                        self.set_status(format!("Uninstalled command {name}"));
                    }
                    Err(e) => {
                        self.log
                            .push(super::log::LogLevel::Error, format!("Uninstall error: {e}"));
                        self.set_status(format!("Error: {e}"));
                    }
                }
            }
            SkillInstallStatus::NotInstalled => {
                match skills::install_command(&name, &source_path, &self.commands_dir) {
                    Ok(()) => {
                        self.groups[group_index].commands[command_index].install_status =
                            SkillInstallStatus::Installed;
                        self.log
                            .push(super::log::LogLevel::Success, format!("Installed {name}"));
                        self.set_status(format!("Installed command {name}"));
                    }
                    Err(e) => {
                        self.log
                            .push(super::log::LogLevel::Error, format!("Install error: {e}"));
                        self.set_status(format!("Error: {e}"));
                    }
                }
            }
            SkillInstallStatus::Conflict => {
                self.log.push(
                    super::log::LogLevel::Warning,
                    format!("Conflict: command {name} from another source"),
                );
                self.set_status(format!("Conflict: command {name} from another source"));
            }
        }
    }

    fn start_delete(&mut self) {
        let row = match self.current_row() {
            Some(r) => r.clone(),
            None => return,
        };
        if let ListRow::SourceHeader { group_index, .. } = row {
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
        match skills::delete_source(&group, &self.skills_dir, &self.agents_dir, &self.commands_dir) {
            Ok(()) => {
                if let SourceKind::Repo { url: Some(ref url) } = group.kind {
                    self.config.remove_source_repo(url);
                    let _ = self.config.save();
                }
                self.log.push(
                    super::log::LogLevel::Success,
                    format!("Deleted source: {}", group.name),
                );
                self.set_status(format!("Deleted source: {}", group.name));
                self.refresh();
            }
            Err(e) => {
                self.log
                    .push(super::log::LogLevel::Error, format!("Delete error: {e}"));
                self.set_status(format!("Error deleting: {e}"));
            }
        }
        self.confirm_state = None;
    }

    fn start_bulk_toggle(&mut self, group_index: usize, category: Category) {
        let group = &self.groups[group_index];
        let all_installed = match category {
            Category::Skills => group
                .skills
                .iter()
                .filter(|s| s.install_status != SkillInstallStatus::Conflict)
                .all(|s| s.install_status == SkillInstallStatus::Installed),
            Category::Agents => group
                .agents
                .iter()
                .filter(|a| a.install_status != SkillInstallStatus::Conflict)
                .all(|a| a.install_status == SkillInstallStatus::Installed),
            Category::Commands => group
                .commands
                .iter()
                .filter(|c| c.install_status != SkillInstallStatus::Conflict)
                .all(|c| c.install_status == SkillInstallStatus::Installed),
        };
        self.confirm_state = Some(ConfirmState::BulkToggle {
            group_index,
            category,
            install: !all_installed,
        });
    }

    fn execute_bulk_toggle(&mut self, group_index: usize, category: Category, install: bool) {
        let mut count = 0usize;
        match category {
            Category::Skills => {
                let len = self.groups[group_index].skills.len();
                for si in 0..len {
                    let status = &self.groups[group_index].skills[si].install_status;
                    if *status == SkillInstallStatus::Conflict {
                        continue;
                    }
                    let should_act = if install {
                        *status == SkillInstallStatus::NotInstalled
                    } else {
                        *status == SkillInstallStatus::Installed
                    };
                    if should_act {
                        self.toggle_skill(group_index, si);
                        count += 1;
                    }
                }
            }
            Category::Agents => {
                let len = self.groups[group_index].agents.len();
                for ai in 0..len {
                    let status = &self.groups[group_index].agents[ai].install_status;
                    if *status == SkillInstallStatus::Conflict {
                        continue;
                    }
                    let should_act = if install {
                        *status == SkillInstallStatus::NotInstalled
                    } else {
                        *status == SkillInstallStatus::Installed
                    };
                    if should_act {
                        self.toggle_agent(group_index, ai);
                        count += 1;
                    }
                }
            }
            Category::Commands => {
                let len = self.groups[group_index].commands.len();
                for ci in 0..len {
                    let status = &self.groups[group_index].commands[ci].install_status;
                    if *status == SkillInstallStatus::Conflict {
                        continue;
                    }
                    let should_act = if install {
                        *status == SkillInstallStatus::NotInstalled
                    } else {
                        *status == SkillInstallStatus::Installed
                    };
                    if should_act {
                        self.toggle_command(group_index, ci);
                        count += 1;
                    }
                }
            }
        }
        let action = if install { "Installed" } else { "Uninstalled" };
        let kind = match category {
            Category::Skills => "skill(s)",
            Category::Agents => "agent(s)",
            Category::Commands => "command(s)",
        };
        self.set_status(format!("{action} {count} {kind}"));
    }

    fn open_editor(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
        let row = match self.current_row() {
            Some(r) => r.clone(),
            None => return,
        };
        let file_path = match row {
            ListRow::SkillItem {
                group_index,
                skill_index,
            } => {
                let skill = &self.groups[group_index].skills[skill_index];
                let skill_md = skill.source_path.join("SKILL.md");
                if !skill_md.exists() {
                    self.set_status("SKILL.md not found");
                    return;
                }
                skill_md
            }
            ListRow::AgentItem {
                group_index,
                agent_index,
            } => {
                let agent = &self.groups[group_index].agents[agent_index];
                if !agent.source_path.exists() {
                    self.set_status("Agent file not found");
                    return;
                }
                agent.source_path.clone()
            }
            ListRow::CommandItem {
                group_index,
                command_index,
            } => {
                let command = &self.groups[group_index].commands[command_index];
                if !command.source_path.exists() {
                    self.set_status("Command file not found");
                    return;
                }
                command.source_path.clone()
            }
            _ => return,
        };

        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);

        let ed = editor::get_editor(&self.config);
        let _ = editor::open_files(&ed, &[file_path.as_path()]);

        let _ = stdout().execute(EnterAlternateScreen);
        let _ = enable_raw_mode();
        let _ = terminal.clear();
    }

    fn show_info(&mut self) {
        let row = match self.current_row() {
            Some(r) => r.clone(),
            None => return,
        };

        let lines = match row {
            ListRow::SkillItem {
                group_index,
                skill_index,
            } => self.build_skill_info_lines(group_index, skill_index),
            ListRow::AgentItem {
                group_index,
                agent_index,
            } => self.build_agent_info_lines(group_index, agent_index),
            ListRow::CommandItem {
                group_index,
                command_index,
            } => self.build_command_info_lines(group_index, command_index),
            ListRow::SourceHeader { group_index, .. } => self.build_source_info_lines(group_index),
            ListRow::CategoryHeader { .. } => {
                return; // No info for category headers
            }
        };

        self.info_popup =
            Some(super::popup::ScrollablePopup::new("Info", lines).with_close_hint("Esc:close"));
    }

    fn build_skill_info_lines(&self, group_index: usize, skill_index: usize) -> Vec<Line<'static>> {
        let group = &self.groups[group_index];
        let skill = &group.skills[skill_index];
        let mut lines = Vec::new();

        // Header info
        lines.push(Line::from(vec![
            Span::styled("Name: ", Style::default().fg(Color::Yellow)),
            Span::raw(skill.name.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Source: ", Style::default().fg(Color::Yellow)),
            Span::raw(group.name.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Path: ", Style::default().fg(Color::Yellow)),
            Span::raw(contract_tilde(&skill.source_path).to_string()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{:?}", skill.install_status)),
        ]));
        lines.push(Line::default()); // blank line

        // Directory listing
        lines.push(Line::from(Span::styled(
            "Files:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        if let Ok(mut entries) = std::fs::read_dir(&skill.source_path) {
            let mut file_names: Vec<String> = Vec::new();
            while let Some(Ok(entry)) = entries.next() {
                if let Some(name) = entry.file_name().to_str() {
                    let suffix = if entry.path().is_dir() { "/" } else { "" };
                    file_names.push(format!("  {}{}", name, suffix));
                }
            }
            file_names.sort();
            for name in file_names {
                lines.push(Line::from(name));
            }
        } else {
            lines.push(Line::from("  (unable to read directory)"));
        }
        lines.push(Line::default()); // blank line

        // SKILL.md content
        let skill_md_path = skill.source_path.join("SKILL.md");
        if skill_md_path.exists() {
            lines.push(Line::from(Span::styled(
                "─── SKILL.md ───",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            match std::fs::read_to_string(&skill_md_path) {
                Ok(content) => {
                    for line in content.lines().take(5000) {
                        lines.push(Line::from(line.to_string()));
                    }
                }
                Err(e) => {
                    lines.push(Line::from(format!("(error reading SKILL.md: {})", e)));
                }
            }
        }

        lines
    }

    fn build_agent_info_lines(&self, group_index: usize, agent_index: usize) -> Vec<Line<'static>> {
        let group = &self.groups[group_index];
        let agent = &group.agents[agent_index];
        let mut lines = Vec::new();

        lines.push(Line::from(vec![
            Span::styled("Name: ", Style::default().fg(Color::Yellow)),
            Span::raw(agent.name.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Source: ", Style::default().fg(Color::Yellow)),
            Span::raw(group.name.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Path: ", Style::default().fg(Color::Yellow)),
            Span::raw(contract_tilde(&agent.source_path).to_string()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{:?}", agent.install_status)),
        ]));
        lines.push(Line::default());

        // Agent .md content
        if agent.source_path.exists() {
            lines.push(Line::from(Span::styled(
                format!(
                    "─── {} ───",
                    agent
                        .source_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("agent.md")
                ),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            match std::fs::read_to_string(&agent.source_path) {
                Ok(content) => {
                    for line in content.lines().take(5000) {
                        lines.push(Line::from(line.to_string()));
                    }
                }
                Err(e) => {
                    lines.push(Line::from(format!("(error reading agent file: {})", e)));
                }
            }
        }

        lines
    }

    fn build_command_info_lines(&self, group_index: usize, command_index: usize) -> Vec<Line<'static>> {
        let group = &self.groups[group_index];
        let command = &group.commands[command_index];
        let mut lines = Vec::new();

        lines.push(Line::from(vec![
            Span::styled("Name: ", Style::default().fg(Color::Yellow)),
            Span::raw(command.name.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Source: ", Style::default().fg(Color::Yellow)),
            Span::raw(group.name.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Path: ", Style::default().fg(Color::Yellow)),
            Span::raw(contract_tilde(&command.source_path).to_string()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{:?}", command.install_status)),
        ]));
        lines.push(Line::default());

        if command.source_path.exists() {
            lines.push(Line::from(Span::styled(
                format!(
                    "─── {} ───",
                    command
                        .source_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("command.md")
                ),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            match std::fs::read_to_string(&command.source_path) {
                Ok(content) => {
                    for line in content.lines().take(5000) {
                        lines.push(Line::from(line.to_string()));
                    }
                }
                Err(e) => {
                    lines.push(Line::from(format!("(error reading command file: {})", e)));
                }
            }
        }

        lines
    }

    fn build_source_info_lines(&self, group_index: usize) -> Vec<Line<'static>> {
        let group = &self.groups[group_index];
        let mut lines = Vec::new();

        lines.push(Line::from(vec![
            Span::styled("Name: ", Style::default().fg(Color::Yellow)),
            Span::raw(group.name.clone()),
        ]));

        let kind_str = match &group.kind {
            SourceKind::Repo { url: Some(url) } => format!("Repo ({})", url),
            SourceKind::Repo { url: None } => "Repo (no URL)".to_string(),
            SourceKind::Local => "Local".to_string(),
            SourceKind::Migrated { tool } => format!("Migrated from {}", tool),
        };
        lines.push(Line::from(vec![
            Span::styled("Type: ", Style::default().fg(Color::Yellow)),
            Span::raw(kind_str),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Path: ", Style::default().fg(Color::Yellow)),
            Span::raw(contract_tilde(&group.path).to_string()),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Skills: ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{}", group.skills.len())),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Agents: ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{}", group.agents.len())),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Commands: ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{}", group.commands.len())),
        ]));

        lines.push(Line::default());

        // List skills
        if !group.skills.is_empty() {
            lines.push(Line::from(Span::styled(
                "Skills:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            for skill in &group.skills {
                let status_icon = match skill.install_status {
                    SkillInstallStatus::Installed => "✓",
                    SkillInstallStatus::NotInstalled => "○",
                    SkillInstallStatus::Conflict => "⚠",
                };
                lines.push(Line::from(format!("  {} {}", status_icon, skill.name)));
            }
            lines.push(Line::default());
        }

        // List agents
        if !group.agents.is_empty() {
            lines.push(Line::from(Span::styled(
                "Agents:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            for agent in &group.agents {
                let status_icon = match agent.install_status {
                    SkillInstallStatus::Installed => "✓",
                    SkillInstallStatus::NotInstalled => "○",
                    SkillInstallStatus::Conflict => "⚠",
                };
                lines.push(Line::from(format!("  {} {}", status_icon, agent.name)));
            }
        }

        // List commands
        if !group.commands.is_empty() {
            lines.push(Line::from(Span::styled(
                "Commands:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            for command in &group.commands {
                let status_icon = match command.install_status {
                    SkillInstallStatus::Installed => "✓",
                    SkillInstallStatus::NotInstalled => "○",
                    SkillInstallStatus::Conflict => "⚠",
                };
                lines.push(Line::from(format!("  {} {}", status_icon, command.name)));
            }
        }

        lines
    }

    fn expand_all(&mut self) {
        self.expanded_categories.insert(Category::Skills);
        self.expanded_categories.insert(Category::Agents);
        self.expanded_categories.insert(Category::Commands);
        for i in 0..self.groups.len() {
            if self.groups[i].skills.iter().any(|_| true) {
                self.expanded_skills_sources.insert(i);
            }
            if self.groups[i].agents.iter().any(|_| true) {
                self.expanded_agents_sources.insert(i);
            }
            if self.groups[i].commands.iter().any(|_| true) {
                self.expanded_commands_sources.insert(i);
            }
        }
        self.rebuild_rows();
        self.clamp_cursor();
    }

    fn collapse_all(&mut self) {
        self.expanded_categories.clear();
        self.expanded_skills_sources.clear();
        self.expanded_agents_sources.clear();
        self.rebuild_rows();
        self.clamp_cursor();
    }

    fn do_update(&mut self) {
        if self.background_task.as_ref().is_some_and(|t| t.is_running) {
            self.set_status("Update already in progress");
            return;
        }
        self.background_task = Some(super::background::spawn_update(
            self.skills_dir.clone(),
            self.agents_dir.clone(),
            self.commands_dir.clone(),
            self.source_dir.clone(),
        ));
        self.log.push(super::log::LogLevel::Info, "Update started");
        self.set_status("⟳ Updating...");
    }

    fn do_add(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);

        use dialoguer::Input;
        let source: Result<String, _> = Input::new()
            .with_prompt("URL or local path")
            .interact_text();

        let _ = stdout().execute(EnterAlternateScreen);
        let _ = enable_raw_mode();
        let _ = terminal.clear();

        let source = match source {
            Ok(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => {
                self.set_status("Add cancelled");
                return;
            }
        };

        if skills::is_url(&source) {
            match skills::clone_or_pull(&source, &self.source_dir) {
                Ok((repo_path, found_skills)) => {
                    let mut count = 0;
                    for (name, skill_path) in &found_skills {
                        if skills::install_skill(name, skill_path, &self.skills_dir).is_ok() {
                            count += 1;
                        }
                    }
                    let found_agents = skills::scan_agents(&repo_path);
                    let mut agent_count = 0;
                    for (name, agent_path) in &found_agents {
                        if skills::install_agent(name, agent_path, &self.agents_dir).is_ok() {
                            agent_count += 1;
                        }
                    }
                    let _ = self.config.add_source_repo(&source);
                    self.log.push(
                        super::log::LogLevel::Success,
                        format!("Added from URL: {count} skill(s), {agent_count} agent(s)"),
                    );
                    self.set_status(format!("Added: {count} skill(s), {agent_count} agent(s)"));
                }
                Err(e) => {
                    self.log
                        .push(super::log::LogLevel::Error, format!("Add error: {e}"));
                    self.set_status(format!("Error: {e}"));
                }
            }
        } else {
            let source_path = expand_tilde(&source);
            match skills::add_local_copy(&source_path, &self.source_dir) {
                Ok((_dest, found_skills)) => {
                    let mut count = 0;
                    for (name, skill_path) in &found_skills {
                        if skills::install_skill(name, skill_path, &self.skills_dir).is_ok() {
                            count += 1;
                        }
                    }
                    self.log.push(
                        super::log::LogLevel::Success,
                        format!("Added local: {count} skill(s)"),
                    );
                    self.set_status(format!("Added: {count} skill(s)"));
                }
                Err(e) => {
                    self.log
                        .push(super::log::LogLevel::Error, format!("Add error: {e}"));
                    self.set_status(format!("Error: {e}"));
                }
            }
        }
        self.refresh();
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        area_height: u16,
    ) {
        // Log popup intercepts all keys when visible
        if self.show_log {
            match code {
                KeyCode::Char('l') | KeyCode::Esc => {
                    self.show_log = false;
                    self.log_popup = None;
                }
                _ => {
                    if let Some(ref mut popup) = self.log_popup {
                        let _ = popup.handle_key(code);
                    }
                }
            }
            return;
        }

        // Info popup intercepts all keys when visible
        if let Some(ref mut popup) = self.info_popup {
            match code {
                KeyCode::Char('i') | KeyCode::Esc => {
                    self.info_popup = None;
                }
                _ => {
                    if popup.handle_key(code) == super::popup::PopupAction::Close {
                        self.info_popup = None;
                    }
                }
            }
            return;
        }

        // Confirmation mode
        if let Some(state) = self.confirm_state.clone() {
            match state {
                ConfirmState::Normal { group_index } => match code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => self.execute_delete(group_index),
                    _ => {
                        self.confirm_state = None;
                        self.set_status("Delete cancelled");
                    }
                },
                ConfirmState::Migrated {
                    group_index,
                    mut typed,
                } => match code {
                    KeyCode::Char(c) => {
                        typed.push(c);
                        if typed == "delete" {
                            self.execute_delete(group_index);
                        } else if !"delete".starts_with(&typed) {
                            self.confirm_state = None;
                            self.set_status("Delete cancelled");
                        } else {
                            self.confirm_state =
                                Some(ConfirmState::Migrated { group_index, typed });
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
                ConfirmState::BulkToggle {
                    group_index,
                    category,
                    install,
                } => match code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        self.execute_bulk_toggle(group_index, category, install);
                        self.confirm_state = None;
                    }
                    _ => {
                        self.confirm_state = None;
                        self.set_status("Cancelled");
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
                KeyCode::Char(c) if !modifiers.contains(KeyModifiers::CONTROL) => {
                    self.search_query.push(c);
                    self.apply_search_filter();
                    self.cursor = 0;
                }
                KeyCode::Up | KeyCode::Char('k') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.move_cursor(-1);
                }
                KeyCode::Down | KeyCode::Char('j') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.move_cursor(1);
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
            KeyCode::Char(' ') | KeyCode::Enter => {
                let row = self.current_row().cloned();
                match row {
                    Some(ListRow::SkillItem { .. }) | Some(ListRow::AgentItem { .. }) | Some(ListRow::CommandItem { .. }) => {
                        self.show_info();
                    }
                    _ => self.toggle_item(),
                }
            }
            KeyCode::Char('e') => self.open_editor(terminal),
            KeyCode::Delete | KeyCode::Char('d') => self.start_delete(),
            KeyCode::Char('i') => {
                let row = self.current_row().cloned();
                match row {
                    Some(ListRow::SkillItem {
                        group_index,
                        skill_index,
                    }) => {
                        if self.config.central.is_disabled("skills") {
                            self.set_status("Skills feature is disabled");
                        } else {
                            self.toggle_skill(group_index, skill_index);
                        }
                    }
                    Some(ListRow::AgentItem {
                        group_index,
                        agent_index,
                    }) => {
                        if self.config.central.is_disabled("agents") {
                            self.set_status("Agents feature is disabled");
                        } else {
                            self.toggle_agent(group_index, agent_index);
                        }
                    }
                    Some(ListRow::CommandItem {
                        group_index,
                        command_index,
                    }) => {
                        if self.config.central.is_disabled("commands") {
                            self.set_status("Commands feature is disabled");
                        } else {
                            self.toggle_command(group_index, command_index);
                        }
                    }
                    Some(ListRow::SourceHeader {
                        group_index,
                        category,
                    }) => {
                        let feature = match category {
                            Category::Skills => "skills",
                            Category::Agents => "agents",
                            Category::Commands => "commands",
                        };
                        if self.config.central.is_disabled(feature) {
                            self.set_status(format!("{} feature is disabled", feature));
                        } else {
                            self.start_bulk_toggle(group_index, category);
                        }
                    }
                    _ => {}
                }
            }
            KeyCode::Char('r') => {
                self.refresh();
                self.log.push(super::log::LogLevel::Info, "Refreshed");
                self.set_status("Refreshed");
            }
            KeyCode::Char('u') => self.do_update(),
            KeyCode::Char('a') => self.do_add(terminal),
            KeyCode::Char('0') => {
                self.collapse_all();
                self.set_status("Collapsed all");
            }
            KeyCode::Char('9') => {
                self.expand_all();
                self.set_status("Expanded all");
            }
            KeyCode::Char('/') => {
                self.search_mode = true;
                // Preserve existing search_query and filtered_rows
                // so user can continue editing their previous search
                self.expand_all();
            }
            KeyCode::Char('l') => {
                self.show_log = true;
                let lines = self.log.to_lines();
                let mut popup =
                    super::popup::ScrollablePopup::new("Log", lines).with_close_hint("l:close");
                // Auto-scroll to end
                popup.scroll_offset = popup.lines.len().saturating_sub(1);
                self.log_popup = Some(popup);
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
// Row building
// ---------------------------------------------------------------------------

fn build_rows(
    groups: &[SourceGroup],
    expanded_categories: &HashSet<Category>,
    expanded_skills_sources: &HashSet<usize>,
    expanded_agents_sources: &HashSet<usize>,
    expanded_commands_sources: &HashSet<usize>,
) -> Vec<ListRow> {
    let mut rows = Vec::new();

    // Skills section
    let has_skills = groups.iter().any(|g| !g.skills.is_empty());
    if has_skills {
        rows.push(ListRow::CategoryHeader {
            category: Category::Skills,
        });
        if expanded_categories.contains(&Category::Skills) {
            for (gi, group) in groups.iter().enumerate() {
                if group.skills.is_empty() {
                    continue;
                }
                rows.push(ListRow::SourceHeader {
                    category: Category::Skills,
                    group_index: gi,
                });
                if expanded_skills_sources.contains(&gi) {
                    for si in 0..group.skills.len() {
                        rows.push(ListRow::SkillItem {
                            group_index: gi,
                            skill_index: si,
                        });
                    }
                }
            }
        }
    }

    // Agents section
    let has_agents = groups.iter().any(|g| !g.agents.is_empty());
    if has_agents {
        rows.push(ListRow::CategoryHeader {
            category: Category::Agents,
        });
        if expanded_categories.contains(&Category::Agents) {
            for (gi, group) in groups.iter().enumerate() {
                if group.agents.is_empty() {
                    continue;
                }
                rows.push(ListRow::SourceHeader {
                    category: Category::Agents,
                    group_index: gi,
                });
                if expanded_agents_sources.contains(&gi) {
                    for ai in 0..group.agents.len() {
                        rows.push(ListRow::AgentItem {
                            group_index: gi,
                            agent_index: ai,
                        });
                    }
                }
            }
        }
    }

    // Commands section
    let has_commands = groups.iter().any(|g| !g.commands.is_empty());
    if has_commands {
        rows.push(ListRow::CategoryHeader {
            category: Category::Commands,
        });
        if expanded_categories.contains(&Category::Commands) {
            for (gi, group) in groups.iter().enumerate() {
                if group.commands.is_empty() {
                    continue;
                }
                rows.push(ListRow::SourceHeader {
                    category: Category::Commands,
                    group_index: gi,
                });
                if expanded_commands_sources.contains(&gi) {
                    for ci in 0..group.commands.len() {
                        rows.push(ListRow::CommandItem {
                            group_index: gi,
                            command_index: ci,
                        });
                    }
                }
            }
        }
    }

    rows
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn kind_icon(kind: &SourceKind) -> &'static str {
    match kind {
        SourceKind::Repo { .. } => "📦",
        SourceKind::Local => "📁",
        SourceKind::Migrated { .. } => "🔀",
    }
}

fn kind_label(kind: &SourceKind) -> &'static str {
    match kind {
        SourceKind::Repo { .. } => "repo",
        SourceKind::Local => "local",
        SourceKind::Migrated { .. } => "migrated",
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

fn count_label(items: &[impl std::any::Any], kind: &str) -> String {
    let n = items.len();
    if n == 1 {
        format!("1 {kind}")
    } else {
        format!("{n} {kind}s")
    }
}

fn render_item_line(
    name: &str,
    status: &SkillInstallStatus,
    is_cursor: bool,
    prefix_char: &str,
    match_indices: Option<&[usize]>,
) -> Line<'static> {
    let icon = status_icon(status);
    let color = status_color(status);
    let label = status_label(status);

    let prefix = if is_cursor {
        format!("      {prefix_char} ")
    } else {
        "        ".to_string()
    };

    let mut spans = vec![
        Span::styled(
            prefix,
            if is_cursor {
                Style::default().fg(Color::Yellow).bg(Color::DarkGray)
            } else {
                Style::default()
            },
        ),
        Span::styled(
            format!("{icon} "),
            if is_cursor {
                Style::default().fg(color).bg(Color::DarkGray)
            } else {
                Style::default().fg(color)
            },
        ),
    ];

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

    if let Some(indices) = match_indices {
        let index_set: std::collections::HashSet<usize> = indices.iter().cloned().collect();
        let highlight_style = if is_cursor {
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        };

        let mut current = String::new();
        let mut current_hl = false;
        for (i, ch) in name.chars().enumerate() {
            let is_hl = index_set.contains(&i);
            if i > 0 && is_hl != current_hl {
                spans.push(Span::styled(
                    current.clone(),
                    if current_hl {
                        highlight_style
                    } else {
                        name_style
                    },
                ));
                current.clear();
            }
            current_hl = is_hl;
            current.push(ch);
        }
        if !current.is_empty() {
            spans.push(Span::styled(
                current,
                if current_hl {
                    highlight_style
                } else {
                    name_style
                },
            ));
        }
        let pad_len = 30usize.saturating_sub(name.len());
        if pad_len > 0 {
            spans.push(Span::styled(" ".repeat(pad_len), name_style));
        }
    } else {
        spans.push(Span::styled(format!("{:<30}", name), name_style));
    }

    spans.push(Span::styled(label.to_string(), label_style));

    Line::from(spans)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(app: &mut App, frame: &mut Frame) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    render_list(app, frame, chunks[0]);
    render_footer(app, frame, chunks[1]);

    // Popups (only one should be visible at a time)
    if let Some(ref mut popup) = app.log_popup {
        popup.render(frame, frame.area());
    }
    if let Some(ref mut popup) = app.info_popup {
        popup.render(frame, frame.area());
    }
}

fn render_list(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" AGM Source Manager ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = app.visible_rows();
    if visible.is_empty() {
        let msg = if app.search_query.is_empty() {
            "No sources found. Press 'a' to add one."
        } else {
            "No matching items."
        };
        let p = Paragraph::new(msg).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    }

    let height = inner.height as usize;
    let start = app.scroll_offset;
    let end = (start + height).min(visible.len());

    let mut lines: Vec<Line> = Vec::new();
    for (vis_idx, &row_idx) in visible.iter().enumerate().take(end).skip(start) {
        let is_cursor = vis_idx == app.cursor;
        let row = &app.rows[row_idx];

        let line = match row {
            ListRow::CategoryHeader { category } => {
                let (label, expanded) = match category {
                    Category::Skills => {
                        let total: usize = app.groups.iter().map(|g| g.skills.len()).sum();
                        let installed: usize = app
                            .groups
                            .iter()
                            .flat_map(|g| &g.skills)
                            .filter(|s| s.install_status == SkillInstallStatus::Installed)
                            .count();
                        (
                            format!("🔧 Skills [{installed}/{total}]"),
                            app.expanded_categories.contains(&Category::Skills),
                        )
                    }
                    Category::Agents => {
                        let total: usize = app.groups.iter().map(|g| g.agents.len()).sum();
                        let installed: usize = app
                            .groups
                            .iter()
                            .flat_map(|g| &g.agents)
                            .filter(|a| a.install_status == SkillInstallStatus::Installed)
                            .count();
                        (
                            format!("🤖 Agents [{installed}/{total}]"),
                            app.expanded_categories.contains(&Category::Agents),
                        )
                    }
                    Category::Commands => {
                        let total: usize = app.groups.iter().map(|g| g.commands.len()).sum();
                        let installed: usize = app
                            .groups
                            .iter()
                            .flat_map(|g| &g.commands)
                            .filter(|c| c.install_status == SkillInstallStatus::Installed)
                            .count();
                        (
                            format!("💬 Commands [{installed}/{total}]"),
                            app.expanded_categories.contains(&Category::Commands),
                        )
                    }
                };
                let disabled = match category {
                    Category::Skills => app.config.central.is_disabled("skills"),
                    Category::Agents => app.config.central.is_disabled("agents"),
                    Category::Commands => app.config.central.is_disabled("commands"),
                };

                let arrow = if expanded { "▼" } else { "▶" };
                let text = if disabled {
                    format!("{arrow} {label} (disabled)")
                } else {
                    format!("{arrow} {label}")
                };

                let style = if is_cursor {
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Blue)
                        .add_modifier(Modifier::BOLD)
                } else if disabled {
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                };
                Line::from(Span::styled(text, style))
            }
            ListRow::SourceHeader {
                category,
                group_index,
            } => {
                let group = &app.groups[*group_index];
                let icon = kind_icon(&group.kind);
                let label = kind_label(&group.kind);
                let (item_count, expanded) = match category {
                    Category::Skills => {
                        let c = count_label(
                            &group.skills.iter().map(|_| 0u8).collect::<Vec<_>>(),
                            "skill",
                        );
                        (c, app.expanded_skills_sources.contains(group_index))
                    }
                    Category::Agents => {
                        let c = count_label(
                            &group.agents.iter().map(|_| 0u8).collect::<Vec<_>>(),
                            "agent",
                        );
                        (c, app.expanded_agents_sources.contains(group_index))
                    }
                    Category::Commands => {
                        let c = count_label(
                            &group.commands.iter().map(|_| 0u8).collect::<Vec<_>>(),
                            "command",
                        );
                        (c, app.expanded_commands_sources.contains(group_index))
                    }
                };
                let arrow = if expanded { "▼" } else { "▶" };
                let text = format!("  {arrow} {icon} {} ({label})  [{item_count}]", group.name);

                let style = if is_cursor {
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                };
                Line::from(Span::styled(text, style))
            }
            ListRow::SkillItem {
                group_index,
                skill_index,
            } => {
                let skill = &app.groups[*group_index].skills[*skill_index];
                let disabled = app.config.central.is_disabled("skills");
                let indices = if app.filtered_rows.is_some() && !app.search_query.is_empty() {
                    app.matcher
                        .fuzzy_indices(&skill.name, &app.search_query)
                        .map(|(_, idx)| idx)
                } else {
                    None
                };
                let line = render_item_line(
                    &skill.name,
                    &skill.install_status,
                    is_cursor,
                    ">",
                    indices.as_deref(),
                );
                if disabled && !is_cursor {
                    line.style(Style::default().fg(Color::DarkGray))
                } else {
                    line
                }
            }
            ListRow::AgentItem {
                group_index,
                agent_index,
            } => {
                let agent = &app.groups[*group_index].agents[*agent_index];
                let disabled = app.config.central.is_disabled("agents");
                let indices = if app.filtered_rows.is_some() && !app.search_query.is_empty() {
                    app.matcher
                        .fuzzy_indices(&agent.name, &app.search_query)
                        .map(|(_, idx)| idx)
                } else {
                    None
                };
                let line = render_item_line(
                    &agent.name,
                    &agent.install_status,
                    is_cursor,
                    ">",
                    indices.as_deref(),
                );
                if disabled && !is_cursor {
                    line.style(Style::default().fg(Color::DarkGray))
                } else {
                    line
                }
            }
            ListRow::CommandItem {
                group_index,
                command_index,
            } => {
                let command = &app.groups[*group_index].commands[*command_index];
                let disabled = app.config.central.is_disabled("commands");
                let indices = if app.filtered_rows.is_some() && !app.search_query.is_empty() {
                    app.matcher
                        .fuzzy_indices(&command.name, &app.search_query)
                        .map(|(_, idx)| idx)
                } else {
                    None
                };
                let line = render_item_line(
                    &command.name,
                    &command.install_status,
                    is_cursor,
                    ">",
                    indices.as_deref(),
                );
                if disabled && !is_cursor {
                    line.style(Style::default().fg(Color::DarkGray))
                } else {
                    line
                }
            }
        };
        lines.push(line);
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn hint_key(k: &str) -> Span<'static> {
    Span::styled(k.to_string(), Style::default().fg(Color::Yellow))
}

fn hint_text(t: &str) -> Span<'static> {
    Span::raw(t.to_string())
}

fn build_source_hints(row: Option<&ListRow>) -> Line<'static> {
    let mut spans = Vec::new();
    match row {
        Some(ListRow::CategoryHeader { .. }) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" toggle  ")]);
            spans.extend([hint_key("a"), hint_text(" add  ")]);
            spans.extend([hint_key("u"), hint_text(" update  ")]);
            spans.extend([hint_key("/"), hint_text(" search  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        Some(ListRow::SourceHeader { .. }) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" toggle  ")]);
            spans.extend([hint_key("i"), hint_text(" install all  ")]);
            spans.extend([hint_key("d"), hint_text(" del  ")]);
            spans.extend([hint_key("u"), hint_text(" update  ")]);
            spans.extend([hint_key("/"), hint_text(" search  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        Some(ListRow::SkillItem { .. }) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" info  ")]);
            spans.extend([hint_key("i"), hint_text(" install  ")]);
            spans.extend([hint_key("e"), hint_text(" edit  ")]);
            spans.extend([hint_key("d"), hint_text(" del  ")]);
            spans.extend([hint_key("/"), hint_text(" search  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        Some(ListRow::AgentItem { .. }) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" info  ")]);
            spans.extend([hint_key("i"), hint_text(" install  ")]);
            spans.extend([hint_key("e"), hint_text(" edit  ")]);
            spans.extend([hint_key("/"), hint_text(" search  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        Some(ListRow::CommandItem { .. }) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" info  ")]);
            spans.extend([hint_key("i"), hint_text(" install  ")]);
            spans.extend([hint_key("e"), hint_text(" edit  ")]);
            spans.extend([hint_key("/"), hint_text(" search  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
        None => {
            spans.extend([hint_key("a"), hint_text(" add  ")]);
            spans.extend([hint_key("/"), hint_text(" search  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
    }
    Line::from(spans)
}

fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(ref state) = app.confirm_state {
        let prompt = match state {
            ConfirmState::Normal { group_index } => {
                let g = &app.groups[*group_index];
                format!(
                    "Delete \"{}\" ({} skill(s), {} agent(s), {} command(s))? [y/N]",
                    g.name,
                    g.skills.len(),
                    g.agents.len(),
                    g.commands.len()
                )
            }
            ConfirmState::Migrated { group_index, typed } => {
                let g = &app.groups[*group_index];
                format!("⚠ PERMANENT: Delete \"{}\"? Type 'delete': {typed}", g.name,)
            }
            ConfirmState::BulkToggle {
                group_index,
                category,
                install,
            } => {
                let g = &app.groups[*group_index];
                let action = if *install { "Install" } else { "Uninstall" };
                let (kind, count) = match category {
                    Category::Skills => {
                        let c = g
                            .skills
                            .iter()
                            .filter(|s| {
                                s.install_status != SkillInstallStatus::Conflict
                                    && if *install {
                                        s.install_status == SkillInstallStatus::NotInstalled
                                    } else {
                                        s.install_status == SkillInstallStatus::Installed
                                    }
                            })
                            .count();
                        ("skill(s)", c)
                    }
                    Category::Agents => {
                        let c = g
                            .agents
                            .iter()
                            .filter(|a| {
                                a.install_status != SkillInstallStatus::Conflict
                                    && if *install {
                                        a.install_status == SkillInstallStatus::NotInstalled
                                    } else {
                                        a.install_status == SkillInstallStatus::Installed
                                    }
                            })
                            .count();
                        ("agent(s)", c)
                    }
                    Category::Commands => {
                        let c = g
                            .commands
                            .iter()
                            .filter(|c| {
                                c.install_status != SkillInstallStatus::Conflict
                                    && if *install {
                                        c.install_status == SkillInstallStatus::NotInstalled
                                    } else {
                                        c.install_status == SkillInstallStatus::Installed
                                    }
                            })
                            .count();
                        ("command(s)", c)
                    }
                };
                format!("{action} {count} {kind} from \"{}\"? [y/N]", g.name)
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
        let hints = build_source_hints(app.current_row());

        let status_line = if app.background_task.as_ref().is_some_and(|t| t.is_running) {
            let progress_text = app
                .background_task
                .as_ref()
                .and_then(|t| t.progress.as_ref())
                .map(|p| format!("⟳ {}", p))
                .unwrap_or_else(|| "⟳ Updating...".to_string());
            Line::from(Span::styled(
                progress_text,
                Style::default().fg(Color::Yellow),
            ))
        } else if let Some((ref msg, _)) = app.status_message {
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
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Interactive TUI for managing skills and agents.
pub fn run(config: &mut Config) -> Result<()> {
    let skills_dir = expand_tilde(&config.central.skills_source);
    let agents_dir = expand_tilde(&config.central.agents_source);
    let commands_dir = expand_tilde(&config.central.commands_source);
    let source_dir = expand_tilde(&config.central.source_dir);

    // Prune broken symlinks before loading
    let _ = skills::prune_broken_skills(&skills_dir);
    let _ = skills::prune_broken_agents(&agents_dir);
    let _ = skills::prune_broken_commands(&commands_dir);

    // Load groups
    let groups = skills::scan_all_sources(
        &source_dir,
        &skills_dir,
        &agents_dir,
        &commands_dir,
        &config.central.source_repos,
    );

    if groups.is_empty() {
        println!("No sources found. Use `agm source --add <url>` to add sources.");
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

    let mut app = App::new(config.clone(), groups, skills_dir, agents_dir, commands_dir, source_dir);

    // Start background update immediately (non-blocking)
    app.do_update();

    // Event loop
    loop {
        let area_height = terminal.size()?.height;
        app.ensure_visible(area_height);
        terminal.draw(|frame| render(&mut app, frame))?;

        app.clear_expired_status();

        // Poll background task for events
        if let Some(ref mut task) = app.background_task {
            for event in task.poll() {
                use super::background::TaskEvent;
                match event {
                    TaskEvent::UpdateRepoStart { name } => {
                        app.log
                            .push(super::log::LogLevel::Info, format!("Updating {}...", name));
                    }
                    TaskEvent::UpdateRepoComplete {
                        name,
                        success,
                        message,
                    } => {
                        let level = if success {
                            super::log::LogLevel::Success
                        } else {
                            super::log::LogLevel::Error
                        };
                        app.log.push(level, format!("{}: {}", name, message));
                    }
                    TaskEvent::UpdateAllDone {
                        total,
                        updated,
                        new_skills,
                        new_agents,
                        new_commands,
                    } => {
                        app.log.push(
                            super::log::LogLevel::Success,
                            format!(
                            "Update complete: {} repos, {} updated, {} new skills, {} new agents, {} new commands",
                            total, updated, new_skills, new_agents, new_commands
                        ),
                        );
                        app.refresh();
                        app.set_status("Update complete");
                    }
                    TaskEvent::OperationResult { message, success } => {
                        let level = if success {
                            super::log::LogLevel::Success
                        } else {
                            super::log::LogLevel::Error
                        };
                        app.log.push(level, message);
                    }
                }
            }
        }

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
    let _ = std::panic::take_hook();

    // Write back any config changes
    *config = app.config;

    Ok(())
}
