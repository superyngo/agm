# Implementation Plan: TUI Redesign

Spec: `docs/superpowers/specs/tui-redesign.md`

## Overview

Refactor the AGM TUI into a modular `src/tui/` framework, fix the source TUI's blocking update and ephemeral info, build a new tool TUI, and consolidate 7 CLI subcommands into `agm tool`.

## Phases

1. **Shared TUI infrastructure** — popup, log, background task modules
2. **Source TUI migration** — move manage.rs → tui/source.rs, integrate new modules
3. **Source TUI features** — background update, info popup, log popup
4. **Tool TUI** — new TUI for managing tools, links, config
5. **CLI consolidation** — replace old subcommands with `agm tool`
6. **Integration testing & polish**

---

## Phase 1: Shared TUI Infrastructure

### Task 1.1: Create tui module skeleton

Create `src/tui/mod.rs` with shared types and re-exports.

**Create `src/tui/mod.rs`:**
```rust
pub mod background;
pub mod log;
pub mod popup;
pub mod source;
pub mod tool;

use ratatui::layout::Rect;

/// Calculate centered popup area (80% width × 80% height, min 40×12)
pub fn popup_area(area: Rect) -> Rect {
    let width = (area.width * 80 / 100).max(40).min(area.width);
    let height = (area.height * 80 / 100).max(12).min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

/// Calculate small centered popup area for dialogs (50% width, auto height)
pub fn dialog_area(area: Rect, content_height: u16) -> Rect {
    let width = (area.width * 50 / 100).max(30).min(area.width);
    let height = (content_height + 4).min(area.height); // +4 for borders+padding
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}
```

Update `src/main.rs` line 5: change `mod manage;` → `mod tui;` (we'll add the source module in Phase 2).

**Tests:** Unit test `popup_area` and `dialog_area` with various terminal sizes including edge cases (tiny terminal 20×6, large terminal 200×60).

**Verify:** `cargo test`

---

### Task 1.2: Implement ScrollablePopup (`src/tui/popup.rs`)

A reusable scrollable popup widget.

```rust
use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

const MAX_CONTENT_LINES: usize = 5000;

pub struct ScrollablePopup {
    pub title: String,
    pub lines: Vec<Line<'static>>,
    pub scroll_offset: usize,
    visible_height: usize,
    close_hint: String, // e.g., "Esc:close" or "l:close"
}

impl ScrollablePopup {
    pub fn new(title: impl Into<String>, lines: Vec<Line<'static>>) -> Self { ... }
    pub fn with_close_hint(mut self, hint: impl Into<String>) -> Self { ... }

    /// Returns true if the popup handled the key (consumed it).
    pub fn handle_key(&mut self, code: KeyCode) -> PopupAction {
        match code {
            KeyCode::Up | KeyCode::Char('k') => { self.scroll_up(1); PopupAction::Consumed }
            KeyCode::Down | KeyCode::Char('j') => { self.scroll_down(1); PopupAction::Consumed }
            KeyCode::PageUp => { self.scroll_up(self.visible_height); PopupAction::Consumed }
            KeyCode::PageDown => { self.scroll_down(self.visible_height); PopupAction::Consumed }
            KeyCode::Home => { self.scroll_offset = 0; PopupAction::Consumed }
            KeyCode::End => { self.scroll_to_end(); PopupAction::Consumed }
            KeyCode::Esc => PopupAction::Close,
            _ => PopupAction::Ignored,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        // Use super::popup_area(area) to calculate centered rect
        // Render Clear + Block with title + close_hint
        // Render Paragraph with scroll_offset
        // Render position indicator [line X/Y] bottom-right
        // Update self.visible_height from inner area
    }

    fn scroll_up(&mut self, amount: usize) { ... }
    fn scroll_down(&mut self, amount: usize) { ... }
    fn scroll_to_end(&mut self) { ... }
    fn max_scroll(&self) -> usize { ... }
}

pub enum PopupAction {
    Consumed,  // key was handled, popup stays open
    Close,     // popup should close
    Ignored,   // key not relevant to popup
}
```

**Tests:**
- `test_scroll_up_down` — scroll within bounds
- `test_scroll_clamp` — can't scroll past content
- `test_page_up_down` — page scroll amounts
- `test_home_end` — jump to boundaries
- `test_handle_key_esc` — returns `PopupAction::Close`
- `test_handle_key_unknown` — returns `PopupAction::Ignored`
- `test_truncation` — lines > 5000 get truncated with indicator

**Verify:** `cargo test`

---

### Task 1.3: Implement LogBuffer (`src/tui/log.rs`)

Ring buffer for operation logs + overlay widget.

```rust
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
    pub fn new(max_entries: usize) -> Self { ... }

    pub fn push(&mut self, level: LogLevel, message: impl Into<String>) {
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        // Push entry, pop front if over max_entries
    }

    pub fn len(&self) -> usize { ... }
    pub fn is_empty(&self) -> bool { ... }

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
```

The LogOverlay is built by creating a `ScrollablePopup` from `LogBuffer::to_lines()` with title "Log" and close hint "l:close". This is done at the call site, not as a separate struct — keeps things simple.

**Tests:**
- `test_push_and_len` — push entries, check count
- `test_max_entries_eviction` — push beyond max, verify oldest removed
- `test_to_lines_colors` — verify correct color per level
- `test_auto_scroll_default_true` — new buffer starts with auto_scroll = true

**Verify:** `cargo test`

---

### Task 1.4: Implement BackgroundTask (`src/tui/background.rs`)

Thread-based background task execution with channel communication.

```rust
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Clone)]
pub enum TaskEvent {
    /// A repo update started
    UpdateRepoStart { name: String },
    /// A repo update finished
    UpdateRepoComplete { name: String, success: bool, message: String },
    /// All updates finished
    UpdateAllDone { total: usize, updated: usize, new_skills: usize, new_agents: usize },
    /// Generic operation result (for tool TUI link/unlink etc.)
    OperationResult { message: String, success: bool },
}

pub struct BackgroundTask {
    receiver: mpsc::Receiver<TaskEvent>,
    pub is_running: bool,
    pub progress: Option<String>,
}

impl BackgroundTask {
    pub fn new(receiver: mpsc::Receiver<TaskEvent>) -> Self {
        Self { receiver, is_running: true, progress: None }
    }

    /// Non-blocking drain of all pending events. Returns collected events.
    pub fn poll(&mut self) -> Vec<TaskEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            match &event {
                TaskEvent::UpdateAllDone { .. } => self.is_running = false,
                TaskEvent::UpdateRepoStart { name } => {
                    self.progress = Some(format!("Updating {}...", name));
                }
                _ => {}
            }
            events.push(event);
        }
        events
    }
}

/// Spawn a background update. Returns (BackgroundTask, JoinHandle).
/// The caller stores BackgroundTask in app state; handle can be ignored.
pub fn spawn_update(
    skills_dir: std::path::PathBuf,
    agents_dir: std::path::PathBuf,
    source_dir: std::path::PathBuf,
) -> BackgroundTask {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        crate::skills::update_all_with_progress(&skills_dir, &agents_dir, &source_dir, tx);
    });
    BackgroundTask::new(rx)
}
```

**Tests:**
- `test_poll_drains_events` — send events through channel, verify poll collects them
- `test_poll_sets_not_running_on_done` — send `UpdateAllDone`, verify `is_running = false`
- `test_poll_empty_channel` — poll on empty channel returns empty vec

**Verify:** `cargo test`

---

## Phase 2: Source TUI Migration

### Task 2.1: Add `update_all_with_progress` to skills.rs

Add a callback-based variant of `update_all` that reports per-repo progress. Uses a generic callback instead of `TaskEvent` directly to avoid circular dependency (`skills.rs` → `tui::background`).

**In `src/skills.rs`, after the existing `update_all` function (ends ~line 430):**

```rust
/// Progress report from update_all_with_progress
pub enum UpdateProgress {
    RepoStart { name: String },
    RepoComplete { name: String, success: bool, message: String },
    AllDone { total: usize, updated: usize, new_skills: usize, new_agents: usize },
}

/// Like update_all, but reports progress through a callback.
/// Used by the TUI for non-blocking background updates.
pub fn update_all_with_progress<F>(
    skills_dir: &Path,
    agents_dir: &Path,
    source_dir: &Path,
    mut on_progress: F,
) where F: FnMut(UpdateProgress) {
    // Same logic as update_all but:
    // 1. Call on_progress(RepoStart{..}) before each git pull
    // 2. Call on_progress(RepoComplete{..}) after each
    // 3. Call on_progress(AllDone{..}) at the end
    // 4. No println! calls — all output goes through callback
    // 5. Don't bail on errors — report them and continue
}
```

Then in `tui/background.rs`, `spawn_update` converts `UpdateProgress` → `TaskEvent` via the callback:
```rust
thread::spawn(move || {
    skills::update_all_with_progress(&skills_dir, &agents_dir, &source_dir, |progress| {
        let event = match progress {
            UpdateProgress::RepoStart { name } => TaskEvent::UpdateRepoStart { name },
            UpdateProgress::RepoComplete { name, success, message } =>
                TaskEvent::UpdateRepoComplete { name, success, message },
            UpdateProgress::AllDone { total, updated, new_skills, new_agents } =>
                TaskEvent::UpdateAllDone { total, updated, new_skills, new_agents },
        };
        let _ = tx.send(event);
    });
});
```

This avoids `skills.rs` depending on `tui::background::TaskEvent`.

The existing `update_all()` remains unchanged — it's used by `agm source --update` (non-TUI).

**Tests:**
- `test_update_all_with_progress_sends_events` — create temp repos, run update, verify callback called
- `test_update_all_with_progress_empty_source` — no repos, verify `AllDone` with zeros

**Verify:** `cargo test`

---

### Task 2.2: Move manage.rs → tui/source.rs

Mechanical migration — move file, fix imports, verify compilation.

**Steps:**
1. `mv src/manage.rs src/tui/source.rs`
2. In `src/tui/source.rs`:
   - Change `use crate::config::Config;` (no change needed — crate paths still work)
   - Change `use crate::skills::...` imports (same — still valid)
   - Make the `run` function `pub` (it already is)
3. In `src/main.rs`:
   - Remove `mod manage;`
   - Ensure `mod tui;` is present (from Task 1.1)
   - Change all `manage::run(...)` calls to `tui::source::run(...)`
4. In `src/tui/mod.rs`: `pub mod source;` already declared

**Verify:** `cargo build && cargo test` — all 59 existing tests must pass. No functional changes.

---

## Phase 3: Source TUI Features

### Task 3.1: Integrate LogBuffer into source TUI

Add `LogBuffer` to the source `App` struct and wire all operations to push log entries.

**In `src/tui/source.rs`:**

1. Add fields to `App` struct (after line 85):
```rust
    log: super::log::LogBuffer,
    show_log: bool,
```

2. Initialize in `App::new()`:
```rust
    log: super::log::LogBuffer::new(500),
    show_log: false,
```

3. Add `l` key binding in `handle_key` normal mode (after the `'/' => ...` arm):
```rust
    KeyCode::Char('l') => {
        self.show_log = !self.show_log;
    }
```

4. Replace all `self.set_status(...)` calls that report *operation results* with `self.log.push(...)`:
   - `toggle_item()` install/uninstall results → `log.push(Success/Error, ...)`
   - `do_update()` → will be replaced in Task 3.2
   - `do_add()` results → `log.push(Success/Error, ...)`
   - `execute_delete()` results → `log.push(Success/Error, ...)`
   - `refresh()` → `log.push(Info, "Refreshed")`
   - Keep `set_status` for transient UI messages like "Delete cancelled", "Collapsed all"

5. Render log overlay in `render()` function — after `render_list` and `render_footer`, if `app.show_log`:
```rust
fn render(app: &App, frame: &mut Frame) {
    // ... existing layout code ...
    render_list(app, frame, chunks[0]);
    render_footer(app, frame, chunks[1]);

    if app.show_log {
        // Build popup from log buffer
        let lines = app.log.to_lines();
        let mut popup = ScrollablePopup::new("Log", lines)
            .with_close_hint("l:close");
        popup.render(frame, frame.area());
    }
}
```

6. When `show_log` is true, intercept keys in `handle_key` before normal mode:
```rust
    if self.show_log {
        match code {
            KeyCode::Char('l') | KeyCode::Esc => { self.show_log = false; }
            // Delegate scroll keys to log popup
            _ => { /* scroll handling */ }
        }
        return;
    }
```

Note: For the log popup scroll state, we need a `ScrollablePopup` stored in App state (not reconstructed each frame). Add `log_popup: Option<ScrollablePopup>` to App, create it when `show_log` is toggled on, destroy when toggled off.

**Update footer hints:** Add `l:log` to the keybind hint line.

**Verify:** `cargo build && cargo test` — existing tests pass. Manual test: run `agm source`, press `l` to toggle log.

---

### Task 3.2: Background update for source TUI

Replace blocking `do_update()` with background task.

**In `src/tui/source.rs`:**

1. Add field to `App`:
```rust
    background_task: Option<super::background::BackgroundTask>,
```

2. Replace `do_update()`:
```rust
    fn do_update(&mut self) {
        if self.background_task.as_ref().map_or(false, |t| t.is_running) {
            self.set_status("Update already in progress");
            return;
        }
        self.background_task = Some(super::background::spawn_update(
            self.skills_dir.clone(),
            self.agents_dir.clone(),
            self.source_dir.clone(),
        ));
        self.set_status("⟳ Update started...");
    }
```

3. In the event loop (after `app.clear_expired_status()`), add background task polling:
```rust
    if let Some(ref mut task) = app.background_task {
        for event in task.poll() {
            match event {
                TaskEvent::UpdateRepoStart { name } => {
                    app.log.push(LogLevel::Info, format!("Updating {}...", name));
                }
                TaskEvent::UpdateRepoComplete { name, success, message } => {
                    let level = if success { LogLevel::Success } else { LogLevel::Error };
                    app.log.push(level, format!("{}: {}", name, message));
                }
                TaskEvent::UpdateAllDone { total, updated, new_skills, new_agents } => {
                    app.log.push(LogLevel::Success, format!(
                        "Update complete: {} repos, {} updated, {} new skills, {} new agents",
                        total, updated, new_skills, new_agents
                    ));
                    app.refresh();
                    app.set_status("Update complete");
                }
                _ => {}
            }
        }
    }
```

4. Update footer rendering: when `background_task.is_running`, show `⟳ {progress}` in yellow before keybind hints.

5. Remove blocking update-on-startup from `run()` (lines 1146-1148). Instead, trigger `do_update()` after App is created:
```rust
    let mut app = App::new(...);
    app.do_update(); // Non-blocking now
```

**Verify:** `cargo build && cargo test`. Manual test: run `agm source`, verify update runs in background, TUI remains responsive.

---

### Task 3.3: Info popup for source TUI

Replace the 3-second status message with a scrollable popup.

**In `src/tui/source.rs`:**

1. Add field to `App`:
```rust
    info_popup: Option<super::popup::ScrollablePopup>,
```

2. Rewrite `show_info()` to build popup content:
```rust
    fn show_info(&mut self) {
        let row = match self.current_row() {
            Some(r) => r.clone(),
            None => return,
        };
        let lines = match row {
            ListRow::SkillItem { group_index, skill_index } => {
                let group = &self.groups[group_index];
                let skill = &group.skills[skill_index];
                self.build_skill_info_lines(group, skill)
            }
            ListRow::AgentItem { group_index, agent_index } => {
                let group = &self.groups[group_index];
                let agent = &group.agents[agent_index];
                self.build_agent_info_lines(group, agent)
            }
            ListRow::SourceHeader { group_index, .. } => {
                self.build_source_info_lines(&self.groups[group_index])
            }
            ListRow::CategoryHeader { category } => {
                self.build_category_info_lines(&category)
            }
        };
        self.info_popup = Some(ScrollablePopup::new(
            "Info", lines
        ).with_close_hint("Esc:close"));
    }
```

3. Implement `build_skill_info_lines()`:
   - Name, Source name, Path (contract_tilde), Status
   - Blank line
   - "Files:" header + directory listing (read_dir, sorted)
   - Blank line
   - "─── SKILL.md ───" separator
   - Read SKILL.md content (truncated at 5000 lines)

4. Similar builders for agent, source, category.

5. Intercept keys when `info_popup.is_some()`:
```rust
    if let Some(ref mut popup) = self.info_popup {
        match code {
            KeyCode::Char('i') => { self.info_popup = None; return; }
            _ => {
                match popup.handle_key(code) {
                    PopupAction::Close => { self.info_popup = None; }
                    _ => {}
                }
                return;
            }
        }
    }
```

6. Render popup after footer (same pattern as log overlay):
```rust
    if let Some(ref mut popup) = app.info_popup {
        popup.render(frame, frame.area());
    }
```

**Verify:** `cargo build && cargo test`. Manual test: select a skill, press `i`, scroll through info, press `Esc`.

---

## Phase 4: Tool TUI

### Task 4.1: ToolRow data model and row building

**Create core data structures in `src/tui/tool.rs`:**

```rust
use std::collections::HashSet;
use std::path::PathBuf;

use crate::config::{Config, CentralConfig, ToolConfig};
use crate::linker::{self, LinkStatus};
use crate::paths::{expand_tilde, contract_tilde};
use crate::skills;

#[derive(Debug, Clone, PartialEq)]
pub enum CentralField { Config, Prompt, Skills, Agents, Source }

#[derive(Debug, Clone, PartialEq)]
pub enum ToolField { Prompt, Skills, Agents, Settings, Auth, Mcp }

#[derive(Debug, Clone)]
pub enum ToolRow {
    CentralHeader,
    CentralItem(CentralField),
    ToolHeader { key: String, name: String, installed: bool },
    ToolItem { tool_key: String, field: ToolField },
}

fn build_rows(config: &Config, expanded: &HashSet<String>) -> Vec<ToolRow> {
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
    // Tool sections (BTreeMap = alphabetical order)
    for (key, tool) in &config.tools {
        let installed = tool.is_installed();
        rows.push(ToolRow::ToolHeader {
            key: key.clone(),
            name: tool.name.clone(),
            installed,
        });
        if expanded.contains(key) {
            // Only show linkable items for installed tools
            rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Prompt });
            rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Skills });
            rows.push(ToolRow::ToolItem { tool_key: key.clone(), field: ToolField::Agents });
            // Skip empty Vec fields
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
```

**Tests:**
- `test_build_rows_all_collapsed` — only headers
- `test_build_rows_central_expanded` — central + 5 items
- `test_build_rows_tool_expanded` — tool header + items
- `test_build_rows_empty_vec_skipped` — tool with empty auth/mcp/settings → those rows absent
- `test_build_rows_alphabetical` — tools appear in BTreeMap order

**Verify:** `cargo test`

---

### Task 4.2: ToolApp state, rendering, and navigation

Build the full App state and rendering pipeline.

**`src/tui/tool.rs` — ToolApp struct:**

```rust
pub struct ToolApp {
    config: Config,
    config_path: Option<PathBuf>,
    rows: Vec<ToolRow>,
    cursor: usize,
    scroll_offset: usize,
    expanded: HashSet<String>,
    log: super::log::LogBuffer,
    status_message: Option<(String, std::time::Instant)>,
    popup: Option<PopupState>,
    should_quit: bool,
}

enum PopupState {
    Log(super::popup::ScrollablePopup),
    FilePicker { files: Vec<(String, bool)>, cursor: usize }, // (path, exists)
    PathEditor { field: CentralField, value: String, cursor_pos: usize },
}
```

**Rendering — main layout:**
```rust
fn render(app: &ToolApp, frame: &mut Frame) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    render_list(app, frame, chunks[0]);
    render_footer(app, frame, chunks[1]);

    // Render popup overlay if active
    match &app.popup {
        Some(PopupState::Log(popup)) => { /* render log popup */ }
        Some(PopupState::FilePicker { .. }) => { /* render file picker */ }
        Some(PopupState::PathEditor { .. }) => { /* render path editor */ }
        None => {}
    }
}
```

**Row rendering details:**
- `CentralHeader`: `▼`/`▶` + "central"
- `CentralItem`: indent + field label + `: ` + value (contract_tilde)
  - Skills/Agents: append `(N installed, M sources)` count
- `ToolHeader`: `▼`/`▶` + key + `(name)` + installed status icon
- `ToolItem` linkable: indent + field + `: ` + link status icon + path
- `ToolItem` non-linkable: indent + field + `: ` + file path(s)

**Link status display helper:**
```rust
fn link_status_display(tool: &ToolConfig, field: &ToolField, config: &Config) -> (String, Color) {
    // Check LinkStatus using linker::check_link()
    // Return (display_string, color)
}
```

**Navigation:** Reuse same cursor/scroll logic as source TUI (move_cursor, ensure_visible, clamp_cursor, page_size).

**Footer:** Two-line footer:
- Line 1: `␣/⏎:toggle e:edit 0/9:all l:log q:quit`
- Line 2: status message (green, 3-second expiry) or context info for current row

**Verify:** `cargo build`. Manual test: placeholder entry point, verify rendering.

---

### Task 4.3: Key handling — expand/collapse and toggle link

**Note:** This task depends on Task 5.1 (extract `migrate_skills_dir` to `skills.rs`) for the blocked-link handling logic.

**Expand/collapse:**
```rust
KeyCode::Char(' ') | KeyCode::Enter => {
    match current_row {
        ToolRow::CentralHeader => toggle_expanded("central"),
        ToolRow::ToolHeader { key, .. } => toggle_expanded(key),
        ToolRow::ToolItem { field: ToolField::Prompt | ToolField::Skills | ToolField::Agents, .. } => {
            self.toggle_link();
        }
        ToolRow::CentralItem(CentralField::Skills | CentralField::Agents | CentralField::Source) => {
            self.open_path_editor();
        }
        _ => {} // no-op for settings/auth/mcp/config/prompt
    }
}
KeyCode::Char('0') => { self.expanded.clear(); self.rebuild_rows(); }
KeyCode::Char('9') => {
    self.expanded.insert("central".to_string());
    for key in self.config.tools.keys() {
        self.expanded.insert(key.clone());
    }
    self.rebuild_rows();
}
```

**Toggle link — `toggle_link()`:**
```rust
fn toggle_link(&mut self) {
    let (tool_key, field) = match self.current_row() { ... };
    let tool = match self.config.tools.get(&tool_key) { ... };
    if !tool.is_installed() {
        self.set_status("Tool not installed");
        return;
    }

    let config_dir = tool.resolved_config_dir();
    let (link_path, target, is_dir) = match field {
        ToolField::Prompt => {
            let link = config_dir.join(&tool.prompt_filename);
            let target = expand_tilde(&self.config.central.prompt_source);
            (link, target, false)
        }
        ToolField::Skills => {
            let link = config_dir.join(&tool.skills_dir);
            let target = expand_tilde(&self.config.central.skills_source);
            (link, target, true)
        }
        ToolField::Agents => {
            let link = config_dir.join(&tool.agents_dir);
            let target = expand_tilde(&self.config.central.agents_source);
            (link, target, true)
        }
        _ => return,
    };

    let status = linker::check_link(&link_path, &target, is_dir);
    match status {
        LinkStatus::Linked => {
            // Unlink
            match linker::remove_link(&link_path, &field_label, is_dir) {
                Ok(true) => {
                    self.log.push(LogLevel::Success, format!("Unlinked {} {}", tool_key, field_label));
                    self.set_status(format!("✓ {} {} unlinked", tool_key, field_label));
                }
                Ok(false) => self.set_status("Nothing to unlink"),
                Err(e) => {
                    self.log.push(LogLevel::Error, format!("Unlink failed: {}", e));
                    self.set_status(format!("✗ Unlink failed: {}", e));
                }
            }
        }
        LinkStatus::Missing | LinkStatus::Broken => {
            // Link
            match linker::create_link(&link_path, &target, &field_label, is_dir) {
                Ok(_) => {
                    self.log.push(LogLevel::Success, format!("Linked {} {}", tool_key, field_label));
                    self.set_status(format!("✓ {} {} linked", tool_key, field_label));
                }
                Err(e) => {
                    self.log.push(LogLevel::Error, format!("Link failed: {}", e));
                    self.set_status(format!("✗ Link failed: {}", e));
                }
            }
        }
        LinkStatus::Blocked => {
            // Existing file/dir — handle backup/migration
            self.handle_blocked_link(&tool_key, &field, &link_path, &target, is_dir);
        }
        LinkStatus::Wrong(_) => {
            // Repair: remove wrong link, create correct one
            let _ = platform::remove_link(&link_path);
            match linker::create_link(&link_path, &target, &field_label, is_dir) { ... }
        }
    }
}
```

**`handle_blocked_link()` — backup/migration logic:**
- Prompt file: backup to `{filename}.{YYYYMMDD_HHMMSS}.bak`, then create symlink
- Skills/Agents dir: call migration logic (reuse from main.rs `migrate_skills_dir()` — extract to a shared function first)

**Tests:**
- `test_toggle_link_creates_symlink` — tempdir, toggle on missing → linked
- `test_toggle_link_removes_symlink` — tempdir, create link, toggle → removed
- `test_toggle_link_not_installed` — tool not installed → status message, no change
- `test_toggle_link_blocked_backup` — existing file → backed up + linked

**Verify:** `cargo test`

---

### Task 4.4: Editor integration (`e` key)

```rust
KeyCode::Char('e') => {
    match self.current_row() {
        Some(ToolRow::CentralItem(CentralField::Config)) => {
            self.open_in_editor(terminal, &[config_path]);
        }
        Some(ToolRow::CentralItem(CentralField::Prompt)) => {
            let path = expand_tilde(&self.config.central.prompt_source);
            self.open_in_editor(terminal, &[path]);
        }
        Some(ToolRow::ToolItem { tool_key, field: ToolField::Prompt }) => {
            let tool = &self.config.tools[&tool_key];
            let path = tool.resolved_config_dir().join(&tool.prompt_filename);
            if path.exists() {
                self.open_in_editor(terminal, &[path]);
            } else {
                self.set_status(format!("File not found: {}", contract_tilde(&path)));
            }
        }
        Some(ToolRow::ToolItem { tool_key, field: ToolField::Settings | ToolField::Auth | ToolField::Mcp }) => {
            self.open_tool_files(terminal, &tool_key, &field);
        }
        _ => {} // No action for skills/agents/headers
    }
}
```

**`open_tool_files()` with file picker popup:**
```rust
fn open_tool_files(&mut self, terminal: &mut Terminal<...>, tool_key: &str, field: &ToolField) {
    let tool = &self.config.tools[tool_key];
    let files: Vec<String> = match field {
        ToolField::Settings => tool.settings.clone(),
        ToolField::Auth => tool.auth.clone(),
        ToolField::Mcp => tool.mcp.clone(),
        _ => return,
    };
    if files.is_empty() { return; }
    if files.len() == 1 {
        let path = tool.resolve_path(&files[0]);
        if path.exists() {
            self.open_in_editor(terminal, &[path]);
        } else {
            self.set_status(format!("File not found: {}", contract_tilde(&path)));
        }
    } else {
        // Multiple files → show file picker popup
        let file_entries: Vec<(String, bool)> = files.iter().map(|f| {
            let path = tool.resolve_path(f);
            (contract_tilde(&path).to_string(), path.exists())
        }).collect();
        self.popup = Some(PopupState::FilePicker { files: file_entries, cursor: 0 });
    }
}
```

**`open_in_editor()` — same pattern as source TUI:**
```rust
fn open_in_editor(&self, terminal: &mut Terminal<...>, files: &[PathBuf]) {
    let _ = disable_raw_mode();
    let _ = stdout().execute(LeaveAlternateScreen);
    let ed = editor::get_editor(&self.config);
    let refs: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
    let _ = editor::open_files(&ed, &refs);
    let _ = stdout().execute(EnterAlternateScreen);
    let _ = enable_raw_mode();
    let _ = terminal.clear();
}
```

**Verify:** `cargo build`. Manual test: press `e` on config, verify editor opens.

---

### Task 4.5: File picker popup

Render and handle the file picker overlay.

**Rendering:**
```rust
fn render_file_picker(files: &[(String, bool)], cursor: usize, frame: &mut Frame, area: Rect) {
    let popup_area = super::dialog_area(area, files.len() as u16);
    frame.render_widget(Clear, popup_area);
    let block = Block::default()
        .title(" Select file to edit ")
        .title_bottom(" Esc:cancel ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    for (i, (path, exists)) in files.iter().enumerate() {
        let prefix = if i == cursor { "> " } else { "  " };
        let style = if !exists {
            Style::default().fg(Color::Yellow) // missing file
        } else if i == cursor {
            Style::default().fg(Color::Yellow).bg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };
        let display = if !exists { format!("{}⚠ {}", prefix, path) } else { format!("{}{}", prefix, path) };
        // render line at (inner.x, inner.y + i)
    }
}
```

**Key handling (when `PopupState::FilePicker` is active):**
```rust
PopupState::FilePicker { files, cursor } => {
    match code {
        KeyCode::Up | KeyCode::Char('k') => { *cursor = cursor.saturating_sub(1); }
        KeyCode::Down | KeyCode::Char('j') => { *cursor = (*cursor + 1).min(files.len() - 1); }
        KeyCode::Enter => {
            let (path, exists) = &files[*cursor];
            if *exists {
                // Open in editor, close popup
                let resolved = expand_tilde(path);
                self.popup = None;
                self.open_in_editor(terminal, &[resolved]);
            } else {
                self.set_status(format!("File not found: {}", path));
            }
        }
        KeyCode::Esc => { self.popup = None; }
        _ => {}
    }
}
```

**Verify:** `cargo build`. Manual test: press `e` on settings with multiple files.

---

### Task 4.6: Path editor popup

Inline editing for central skills/agents/source paths.

**Key handling (when `PopupState::PathEditor` is active):**
```rust
PopupState::PathEditor { field, value, cursor_pos } => {
    match code {
        KeyCode::Char(c) => { value.insert(*cursor_pos, c); *cursor_pos += 1; }
        KeyCode::Backspace => {
            if *cursor_pos > 0 {
                value.remove(*cursor_pos - 1);
                *cursor_pos -= 1;
            }
        }
        KeyCode::Delete => {
            if *cursor_pos < value.len() { value.remove(*cursor_pos); }
        }
        KeyCode::Left => { *cursor_pos = cursor_pos.saturating_sub(1); }
        KeyCode::Right => { *cursor_pos = (*cursor_pos + 1).min(value.len()); }
        KeyCode::Home => { *cursor_pos = 0; }
        KeyCode::End => { *cursor_pos = value.len(); }
        KeyCode::Enter => {
            // Save to config
            self.save_central_path(field.clone(), value.clone());
            self.popup = None;
        }
        KeyCode::Esc => { self.popup = None; }
        _ => {}
    }
}
```

**`save_central_path()`:**
```rust
fn save_central_path(&mut self, field: CentralField, value: String) {
    let contracted = contract_tilde(&expand_tilde(&value)).to_string();
    match field {
        CentralField::Skills => self.config.central.skills_source = contracted.clone(),
        CentralField::Agents => self.config.central.agents_source = contracted.clone(),
        CentralField::Source => self.config.central.source_dir = contracted.clone(),
        _ => return,
    }
    match self.config.save() {
        Ok(()) => {
            let expanded = expand_tilde(&contracted);
            if expanded.exists() {
                self.log.push(LogLevel::Success, format!("Updated path: {}", contracted));
            } else {
                self.log.push(LogLevel::Warning, format!("Updated path (not found): {}", contracted));
                self.set_status(format!("⚠ Path does not exist: {}", contracted));
            }
        }
        Err(e) => {
            self.log.push(LogLevel::Error, format!("Failed to save config: {}", e));
            self.set_status(format!("✗ Save failed: {}", e));
        }
    }
    self.rebuild_rows();
}
```

**Rendering:**
```rust
fn render_path_editor(field: &CentralField, value: &str, cursor_pos: usize, frame: &mut Frame, area: Rect) {
    let popup_area = super::dialog_area(area, 3);
    frame.render_widget(Clear, popup_area);
    let title = format!(" Edit {} path ", field_label(field));
    let block = Block::default()
        .title(title)
        .title_bottom(" Enter:save  Esc:cancel ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    // Render value with cursor indicator (█ or highlighted char at cursor_pos)
}
```

**Verify:** `cargo build && cargo test`. Manual test: press Enter on central skills path, edit, press Enter to save.

---

### Task 4.7: Log popup in tool TUI

Wire `l` key to toggle log overlay — same pattern as source TUI.

```rust
KeyCode::Char('l') => {
    if self.popup.is_some() { return; }
    let lines = self.log.to_lines();
    self.popup = Some(PopupState::Log(
        ScrollablePopup::new("Log", lines).with_close_hint("l:close")
    ));
}
```

When `PopupState::Log` is active:
```rust
PopupState::Log(ref mut popup) => {
    match code {
        KeyCode::Char('l') | KeyCode::Esc => { self.popup = None; }
        _ => { popup.handle_key(code); }
    }
}
```

**Verify:** `cargo build`. Manual test: perform operations, press `l`, scroll through log.

---

### Task 4.8: Entry point and event loop

**`pub fn run(config_path: Option<PathBuf>) -> Result<()>`**

Same pattern as source TUI's `run()`:
1. Load config
2. Panic hook for terminal safety
3. Enter alternate screen + raw mode
4. Create ToolApp with central expanded by default
5. Event loop: draw → clear_expired_status → poll 100ms → handle_key
6. Restore terminal on exit
7. Save config if changed

**Verify:** `cargo build`. Full manual test of all interactions.

---

## Phase 5: CLI Consolidation

### Task 5.1: Extract shared functions from main.rs

Before removing old subcommands, extract reusable functions:

1. **`migrate_skills_dir()`** (main.rs lines 190-268) → move to `skills.rs` as `pub fn migrate_tool_dir()` so both tool TUI (Task 4.3 blocked-link handling) and link_all can use it.

2. **`copy_dir_all()`** (main.rs lines 270-300) → move to `skills.rs` as a helper (used by migrate).

3. **`link_all()`** — extract from current `Commands::Link` match arm (lines 396-677, target="all", yes=true) into a standalone function.

4. **`unlink_all()`** — extract from current `Commands::Unlink` match arm (lines 678-749, target="all") including central content restoration logic (copy files back before removing symlinks).

5. **Suppress `linker::create_link()` stdout in TUI context**: The current `create_link()` uses `println!` for status. For TUI use, add an optional `quiet: bool` parameter or create a variant `create_link_quiet()` that returns status strings instead of printing. The TUI then pushes those strings to LogBuffer.

**Verify:** `cargo build && cargo test` — behavior unchanged.

---

### Task 5.2: Replace Commands enum

**Remove these variants from `Commands`:**
- `Link`, `Unlink`, `Status`, `Config`, `Prompt`, `Auth`, `Mcp`

**Add:**
```rust
    /// Manage tools, links, and configuration
    Tool {
        /// Link all tools (non-interactive)
        #[arg(short, long)]
        link: bool,

        /// Unlink all tools (non-interactive)
        #[arg(short = 'u', long)]
        unlink: bool,

        /// Show status table (non-interactive)
        #[arg(short, long)]
        status: bool,
    },
```

**Update routing in `main()`:**
```rust
Some(Commands::Tool { link, unlink, status }) => {
    if [link, unlink, status].iter().filter(|&&x| x).count() > 1 {
        anyhow::bail!("Only one of --link, --unlink, --status can be specified");
    }
    if link {
        let mut config = config::Config::load_from(cli.config.clone())?;
        link_all(&mut config, cli.config.as_deref())?;
    } else if unlink {
        let config = config::Config::load_from(cli.config.clone())?;
        unlink_all(&config)?;
    } else if status {
        status::status()?;
    } else {
        tui::tool::run(cli.config.clone())?;
    }
    Ok(())
}
```

**Remove:** All `open_tool_files()`, `pick_target()`, `pick_link_target()`, `pick_file()` functions that were only used by removed subcommands. Keep `migrate_skills_dir()` and `copy_dir_all()` as they're used by link_all and tool TUI.

**Verify:** `cargo build && cargo test`. Test CLI: `agm tool --status`, `agm tool --link`, `agm tool --unlink`, `agm tool` (TUI).

---

## Phase 6: Integration Testing & Polish

### Task 6.1: Verify all existing tests pass

```bash
cargo test
```

All 59 existing tests must pass. Fix any breakage from refactoring.

---

### Task 6.2: Add integration tests for new CLI

**In `tests/cli.rs` (new file):**
```rust
use assert_cmd::Command;

#[test]
fn test_tool_status_runs() {
    // agm tool --status should not panic (may fail if no config, that's ok)
    let cmd = Command::cargo_bin("agm").unwrap();
    // Just verify it doesn't crash
}

#[test]
fn test_tool_mutually_exclusive_flags() {
    let cmd = Command::cargo_bin("agm").unwrap()
        .args(&["tool", "--link", "--unlink"])
        .assert()
        .failure();
}

#[test]
fn test_help_shows_tool_subcommand() {
    Command::cargo_bin("agm").unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("tool"));
}
```

**Verify:** `cargo test`

---

### Task 6.3: Manual acceptance testing

Checklist:
- [ ] `agm source` — TUI launches, update runs in background, list is interactive
- [ ] `agm source` — press `l` → log popup appears with update history, scrollable
- [ ] `agm source` — press `i` on a skill → info popup with path, files, SKILL.md content
- [ ] `agm source` — press `u` during browse → update runs in background, TUI responsive
- [ ] `agm tool` — TUI shows central + all tools, expand/collapse works
- [ ] `agm tool` — press `␣` on prompt → link/unlink toggles
- [ ] `agm tool` — press `e` on config → editor opens config.toml
- [ ] `agm tool` — press `e` on settings with multiple files → file picker popup
- [ ] `agm tool` — press `Enter` on central skills path → path editor → save
- [ ] `agm tool` — press `l` → log popup with operation history
- [ ] `agm tool --link` — links all tools (non-interactive)
- [ ] `agm tool --unlink` — unlinks all tools
- [ ] `agm tool --status` — shows status table
- [ ] Old commands removed: `agm link`, `agm status`, etc. → error

---

### Task 6.4: Update documentation

- Update `README.md`: replace old command examples with `agm tool`
- Update `CHANGELOG.md`: document the breaking change
- Update custom instructions (`.custom_instructions` or equivalent)

**Verify:** Read through docs, ensure accuracy.
