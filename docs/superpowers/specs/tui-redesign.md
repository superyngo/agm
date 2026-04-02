# TUI Redesign: Source Improvements + Tool Manager

## Problem Statement

The AGM CLI has three interrelated UX issues:

1. **Source TUI update blocks**: `update_all()` runs synchronous `git pull` calls, freezing the TUI and corrupting the display. After completion the screen is not fully restored.
2. **Source TUI info is ephemeral**: Skill/agent info is shown as a 3-second status message — insufficient for paths, file lists, and markdown content.
3. **Fragmented tool management**: `link`, `unlink`, `status`, `config`, `prompt`, `auth`, `mcp` are seven separate subcommands with no unified view. Users must run multiple commands to understand and manage tool state.

## Approach

**Modular TUI framework** — extract shared widgets (popup, log, background tasks) into a `tui/` module, then refactor the source TUI and build a new tool TUI on the same foundation.

### Module Structure

```
src/
  tui/
    mod.rs            — Shared types, AppMode enum, re-exports
    popup.rs          — ScrollablePopup widget (info, log, file picker, path editor)
    log.rs            — LogBuffer ring buffer + LogOverlay widget
    background.rs     — BackgroundTask spawner (thread + mpsc channel)
    source.rs         — Source TUI (refactored from manage.rs)
    tool.rs           — Tool TUI (new)
  config.rs           — unchanged
  editor.rs           — unchanged
  init.rs             — unchanged
  linker.rs           — unchanged
  main.rs             — updated: remove old subcommands, add `tool`
  paths.rs            — unchanged
  platform.rs         — unchanged
  skills.rs           — unchanged (update_all may need per-repo callback)
  status.rs           — retained for `agm tool --status` non-TUI output
```

---

## Shared TUI Infrastructure (`tui/mod.rs`, `popup.rs`, `log.rs`, `background.rs`)

### ScrollablePopup

A generic overlay widget used by info, log, file picker, and path editor popups.

```rust
struct ScrollablePopup {
    title: String,
    lines: Vec<Line<'static>>,  // ratatui styled lines
    scroll_offset: usize,
    visible_height: usize,       // calculated from area
}
```

**Rendering:**
- Centered: width = 80% of terminal width, height = 80% of terminal height (minimum 40×12)
- `Block::bordered()` with title on top-left and close hint on top-right
- Bottom-right: `[line X/Y]` position indicator
- Bottom-left: scroll hint `↑↓/PgUp/PgDn/Home/End`
- On terminal resize: popup recalculates size and clamps `scroll_offset` to valid range

**Key handling** (when popup is active — **all popups intercept keys; main list keys are inactive**):
| Key | Action |
|-----|--------|
| `↑` / `k` | Scroll up 1 line |
| `↓` / `j` | Scroll down 1 line |
| `PgUp` | Scroll up 1 page |
| `PgDn` | Scroll down 1 page |
| `Home` | Jump to top |
| `End` | Jump to bottom |
| `Esc` | Close popup (universal — works for ALL popup types) |
| `q` | **Does NOT quit** while popup is open — only closes popup |

**Large file safeguard:** If content exceeds 5000 lines, truncate and show `[truncated — file too large]` at bottom.

### LogBuffer

Ring buffer holding operation log entries with timestamps.

```rust
struct LogEntry {
    timestamp: chrono::NaiveTime,  // HH:MM:SS, local time
    message: String,
    level: LogLevel,
}

enum LogLevel { Info, Success, Warning, Error }

struct LogBuffer {
    entries: VecDeque<LogEntry>,
    max_entries: usize,            // default 500
    auto_scroll: bool,             // true until user scrolls up
}
```

**Color coding by level:**
- `Info` → white — generic messages (e.g., "Refresh complete", "Loading config")
- `Success` → green — operations succeeded (e.g., "Installed skill-x", "✓ linked prompt")
- `Warning` → yellow — non-fatal issues (e.g., "⚠ Path does not exist", "Skipped (local)")
- `Error` → red — operations failed (e.g., "✗ link failed: permission denied")

**Auto-scroll behavior:**
- New entries auto-scroll to bottom when `auto_scroll` is true
- Any `↑`/`PgUp`/`Home` sets `auto_scroll = false`
- Pressing `End` resets `auto_scroll = true`

### LogOverlay

Wraps `ScrollablePopup` to render `LogBuffer` contents. `l` key is a toggle: press to open, press again to close.

- Title: `─ Log ─`
- Close hint: `l:close` in top-right corner (also closeable with `Esc`)
- Bottom-right: `[N entries]`
- Available in both source and tool TUIs

### BackgroundTask

Spawns operations in a background thread, communicating results via `mpsc::channel`.

```rust
enum TaskEvent {
    // Update events
    UpdateRepoStart { name: String },
    UpdateRepoComplete { name: String, result: Result<String, String> },
    UpdateAllDone { total: usize, updated: usize, new_skills: usize },

    // Generic operation events (for tool TUI)
    OperationComplete { message: String, level: LogLevel },
}

struct BackgroundTask {
    receiver: mpsc::Receiver<TaskEvent>,
    is_running: bool,
    progress: Option<String>,  // e.g., "Updating 3/5 repos..."
}
```

**Integration with event loop:**
```rust
// In the main event loop, after event::poll():
while let Ok(event) = self.background_task.receiver.try_recv() {
    match event {
        TaskEvent::UpdateRepoStart { name } => {
            self.background_task.progress = Some(format!("Updating {}...", name));
        }
        TaskEvent::UpdateRepoComplete { name, result } => {
            self.log.push(/* ... */);
        }
        TaskEvent::UpdateAllDone { .. } => {
            self.background_task.is_running = false;
            self.background_task.progress = None;
            self.refresh();  // reload data
        }
        // ...
    }
}
```

**Status bar rendering** (in footer):
- When background task is running: `⟳ {progress}` in yellow, before keybind hints
- When idle: normal keybind hints

---

## Source TUI Changes (`tui/source.rs`)

### 2a. Background Update

**Before:** `do_update()` calls `skills::update_all()` synchronously — blocks event loop.

**After:** `do_update()` spawns a thread:

```rust
fn do_update(&mut self) {
    if self.background_task.is_running {
        self.set_status("Update already in progress");
        return;
    }
    let (tx, rx) = mpsc::channel();
    self.background_task = BackgroundTask::new(rx);

    let skills_dir = self.skills_dir.clone();
    let agents_dir = self.agents_dir.clone();
    let source_dir = self.source_dir.clone();

    thread::spawn(move || {
        // Per-repo update with progress reporting via tx
        // ... calls update logic, sends TaskEvent for each repo
    });
}
```

**Required change in `skills.rs`:** Add a callback-based or channel-based variant of `update_all` that reports per-repo progress. Options:
- `update_all_with_progress(dirs, sender: mpsc::Sender<TaskEvent>)`
- Or refactor `update_all` internals to be callable per-repo

### 2b. Log Popup

- `l` key toggles `LogOverlay` visibility
- All operations push to shared `LogBuffer`:
  - Install/uninstall skill/agent → Success/Error
  - Delete source → Success/Error
  - Update repos → per-repo progress + summary
  - Add source → clone/copy result
  - Refresh → prune results

### 2c. Info Popup (Source TUI only)

**Triggered by:** `i` key on any selected row in Source TUI.

**Note:** Tool TUI does NOT have an info popup — all information is displayed inline in the tree view. The `i` key is not bound in Tool TUI.

**Content by row type:**

| Row Type | Info Content |
|----------|-------------|
| **CategoryHeader** | Category name, total installed count, total source count |
| **SourceHeader** | Source name, kind (repo/local/migrated), URL/path, skill count, agent count |
| **SkillItem** | Name, source, full path, install status, file listing (`ls`), SKILL.md content |
| **AgentItem** | Name, source, full path, install status, file content (.md) |

**File listing:** Read directory entries of the skill folder, list files/subdirs.

**Markdown rendering:** Read SKILL.md (or agent .md) content as plain text lines. No markdown formatting needed — just display raw text with line numbers context.

### 2d. Startup Change

**Before:** Blocking `update_all()` with `println!` before TUI starts.

**After:** TUI starts immediately. If update-on-startup is desired, trigger `do_update()` automatically after TUI renders first frame, running in background.

### 2e. Source TUI Key Bindings (complete reference)

Existing key bindings from `manage.rs` are **preserved**. Changes marked with ★.

**Normal mode:**
| Key | Action |
|-----|--------|
| `q` / `Ctrl+c` | Quit |
| `↑` / `k` | Move cursor up |
| `↓` / `j` | Move cursor down |
| `PgUp` | Page up |
| `PgDn` | Page down |
| `Home` | Jump to top |
| `End` | Jump to bottom |
| `␣` / `Enter` | Toggle item (expand/collapse or install/uninstall) |
| `e` | Open editor (SKILL.md or agent file) |
| `d` / `Delete` | Start delete confirmation |
| `i` | ★ Open info popup (was: show status message) |
| `r` | Refresh (prune + rescan) |
| `u` | ★ Start background update (was: blocking update) |
| `a` | Add new source (exits TUI for dialoguer input) |
| `l` | ★ **NEW** Toggle log popup |
| `0` | Collapse all |
| `9` | Expand all |
| `/` | Enter search mode |
| `Esc` | Clear filter / dismiss status / close popup |

**When popup is active** (info or log): all keys route to popup (see ScrollablePopup key handling). `l` closes log popup. `Esc` or `i` closes info popup.

**Search mode and confirmation mode:** unchanged from current implementation.

---

## Tool TUI (`tui/tool.rs`)

### Data Model

```rust
enum ToolRow {
    CentralHeader,                        // "central" section header
    CentralItem(CentralField),            // individual central config item
    ToolHeader { key: String, name: String, installed: bool },
    ToolItem { tool_key: String, field: ToolField },
}

enum CentralField {
    Config,   // hardcoded path: ~/.config/agm/config.toml (NOT in CentralConfig struct)
    Prompt,   // maps to config.central.prompt_source
    Skills,   // maps to config.central.skills_source + counts
    Agents,   // maps to config.central.agents_source + counts
    Source,   // maps to config.central.source_dir
}

enum ToolField {
    Prompt,    // link status; uses tool.prompt_filename relative to tool.config_dir
    Skills,    // link status; uses tool.skills_dir relative to tool.config_dir
    Agents,    // link status; uses tool.agents_dir relative to tool.config_dir
    Settings,  // file path(s); from tool.settings: Vec<String>
    Auth,      // file path(s); from tool.auth: Vec<String>
    Mcp,       // file path(s); from tool.mcp: Vec<String>
}
```

**CentralField → Config struct mapping:**
| CentralField | Config Field | Notes |
|---|---|---|
| `Config` | N/A (hardcoded `~/.config/agm/config.toml`) | Not editable via path editor; `e` opens in editor |
| `Prompt` | `config.central.prompt_source` | Editable via path editor |
| `Skills` | `config.central.skills_source` | Editable via path editor |
| `Agents` | `config.central.agents_source` | Editable via path editor |
| `Source` | `config.central.source_dir` | Editable via path editor |

**ToolField → ToolConfig struct mapping:**
| ToolField | ToolConfig Field | Resolved Path |
|---|---|---|
| `Prompt` | `prompt_filename: String` | `{config_dir}/{prompt_filename}` |
| `Skills` | `skills_dir: String` | `{config_dir}/{skills_dir}` |
| `Agents` | `agents_dir: String` | `{config_dir}/{agents_dir}` |
| `Settings` | `settings: Vec<String>` | Each resolved via `tool.resolve_path()` |
| `Auth` | `auth: Vec<String>` | Each resolved via `tool.resolve_path()` |
| `Mcp` | `mcp: Vec<String>` | Each resolved via `tool.resolve_path()` |

### App State

```rust
struct ToolApp {
    config: Config,
    config_path: PathBuf,
    rows: Vec<ToolRow>,
    cursor: usize,
    scroll_offset: usize,

    // Expand/collapse
    expanded: HashSet<String>,  // "central", tool keys

    // Shared infrastructure
    log: LogBuffer,
    background_task: Option<BackgroundTask>,
    status_message: Option<(String, Instant)>,  // auto-expires after 3 seconds (same as source TUI)

    // Popup state (only one popup at a time)
    popup: Option<PopupState>,

    should_quit: bool,
}

enum PopupState {
    FilePicker { files: Vec<String>, cursor: usize },
    PathEditor { field: CentralField, value: String, cursor_pos: usize },
    Log,  // log overlay active
}
```

**Note:** Source TUI has its own `PopupState` that includes `Info(ScrollablePopup)` and `Log`. Tool TUI does not have an info popup — all info is inline.

### Layout

```
Vertical:
  [Min(3)]  → Main list area (bordered, title "AGM Tool Manager")
  [Length(3)] → Footer (keybinds + status message)
```

### Row Rendering

**CentralHeader:**
```
▼ central                      (or ▶ if collapsed)
```

**CentralItem:**
```
    config : ~/.config/agm/config.toml
    prompt : ~/.local/share/agm/prompts/MASTER.md
    skills : ~/.local/share/agm/skills (143 installed, 11 sources)
    agents : ~/.local/share/agm/agents (1 installed)
    source : ~/.local/share/agm/source
```

**ToolHeader:**
```
▼ claude (Claude Code) ✓ installed     (green ✓)
▶ codex (Codex) ✗ not installed        (gray, dimmed)
```

**ToolItem — linkable (prompt/skills/agents):**
```
    prompt : ✓ linked → ~/.claude/CLAUDE.md        (green ✓)
    prompt : ✗ missing                              (red ✗)
    prompt : ⚠ broken → ~/.claude/CLAUDE.md         (yellow ⚠)
    prompt : ⊘ blocked (file exists, not a link)    (yellow ⊘)
    skills : ✓ linked → ~/.claude/skills            (green ✓)
```

**ToolItem — non-linkable (settings/auth/mcp):**
```
    settings: ~/.claude.json                        (single file)
    settings: ~/.claude.json, ~/.claude/settings... (multiple, truncated)
    auth   : ⚠ not found                           (file doesn't exist)
```

### Key Handling Matrix

| Context | Key | Action |
|---------|-----|--------|
| **CentralHeader** | `␣`/`Enter` | Toggle expand/collapse |
| **CentralItem: config** | `e` | Open config.toml in editor |
| **CentralItem: prompt** | `e` | Open MASTER.md in editor |
| **CentralItem: skills/agents/source** | `Enter` | Open path editor popup |
| **ToolHeader** | `␣`/`Enter` | Toggle expand/collapse |
| **ToolItem: prompt** | `␣`/`Enter` | Toggle link/unlink |
| **ToolItem: prompt** | `e` | Open prompt file in editor |
| **ToolItem: skills** | `␣`/`Enter` | Toggle link/unlink |
| **ToolItem: skills** | `e` | No action |
| **ToolItem: agents** | `␣`/`Enter` | Toggle link/unlink |
| **ToolItem: agents** | `e` | No action |
| **ToolItem: settings** | `e` | Open in editor (file picker if multiple) |
| **ToolItem: auth** | `e` | Open in editor (file picker if multiple) |
| **ToolItem: mcp** | `e` | Open in editor (file picker if multiple) |
| **Any** | `0` | Collapse all |
| **Any** | `9` | Expand all |
| **Any** | `l` | Toggle log overlay |
| **Any** | `q`/`Ctrl+c` | Quit |
| **Any** | `↑`/`k` | Move cursor up |
| **Any** | `↓`/`j` | Move cursor down |
| **Any** | `PgUp`/`PgDn` | Page up/down |
| **Any** | `Home`/`End` | Jump to top/bottom |

### Link/Unlink Behavior in TUI

When user presses `␣`/`Enter` on a linkable ToolItem:

**Linking (currently unlinked/missing):**
1. Check current `LinkStatus`
2. If `Blocked` (real file/dir exists):
   - **Prompt file:** Backup to `{filename}.{YYYYMMDD_HHMMSS}.bak`, then create symlink
   - **Skills dir:** Trigger `migrate_skills_dir()` (move skills to central store), then create symlink
   - **Agents dir:** Same migration pattern as skills
3. If `Missing` or `Broken`: Create/repair symlink directly
4. Push result to LogBuffer
5. Update row display
6. Show status message: "✓ {tool} prompt linked" or "✗ link failed: {reason}"

**Unlinking (currently linked):**
1. Copy central content back to tool directory (for prompt: copy file; for skills/agents: handled by existing unlink logic)
2. Remove symlink
3. Push result to LogBuffer
4. Update row display
5. Show status message

### Path Editor Popup

Triggered by `Enter` on central skills/agents/source path.

```
┌─ Edit skills path ─────── Esc:cancel ──┐
│                                         │
│  ~/.local/share/agm/skills█             │
│                                         │
│  Enter: save  Esc: cancel               │
└─────────────────────────────────────────┘
```

**Key handling:**
| Key | Action |
|-----|--------|
| Characters | Insert at cursor position |
| `Backspace` | Delete character before cursor |
| `Delete` | Delete character at cursor |
| `←` / `→` | Move cursor |
| `Home` / `End` | Jump to start/end of input |
| `Enter` | Validate path, save to config.toml, close |
| `Esc` | Cancel, close without saving |

**On save:**
- Input is stored as-is with `~` prefix (using `contract_tilde` if user entered absolute path)
- Expand `~` to validate path exists:
  - If exists: push Success log "Updated skills path to ~/.../skills"
  - If not exists: push Warning log "⚠ Path does not exist: ~/.../skills" + show yellow status message
  - Either way: **save proceeds** (user may create the directory later)
- TOML serialization handled by `Config::save()` — path is stored as a TOML string value, no escaping issues
- Update `config.central.{mapped_field}` (see CentralField mapping table above)
- Call `config.save()` to persist
- Rebuild rows to reflect new path

### File Picker Popup

Triggered by `e` on settings/auth/mcp with multiple files.

```
┌─ Select file to edit ───── Esc:cancel ─┐
│                                         │
│  > ~/.claude.json                       │
│    ~/.claude/settings.json              │
│                                         │
└─────────────────────────────────────────┘
```

**Key handling:**
| Key | Action |
|-----|--------|
| `↑` / `↓` | Move selection |
| `Enter` | Open selected file in editor |
| `Esc` | Cancel |

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| File doesn't exist (auth, settings, mcp) | Show `⚠ not found` in yellow; `e` key shows status "File not found: {path}" |
| Config dir doesn't exist (tool not installed) | Show `✗ not installed` on ToolHeader; all ToolItems dimmed/gray; `␣`/`Enter`/`e` shows "Tool not installed" |
| Link target wrong | Show `⚠ wrong → {actual_target}`; `␣`/`Enter` repairs (remove + recreate) |
| Link broken | Show `⚠ broken`; `␣`/`Enter` repairs |
| Path editor: path doesn't exist | Save anyway; push warning to log: "⚠ Path does not exist: ..."; show status msg |
| Multiple files, some missing | File picker shows all; missing ones marked `⚠` prefix + dimmed; selecting missing shows error status |
| Link/unlink fails (permission etc.) | Show error in status (3s) + log; don't change display state; no rollback needed |
| Empty Vec field (e.g., `auth: []`) | Row not displayed — skip rows with no files configured |
| All files in Vec don't exist | Row shows `⚠ not found` (same as single missing file) |
| Central dirs don't exist | Show path with `⚠ not found`; `e`/`Enter` still works (path editor saves, editor may create) |
| Terminal resize during popup | Popup recalculates size; `scroll_offset` clamped to `max(0, total_lines - visible_height)` |
| `agm tool` while config.toml missing | Error exit with message: "Config not found. Run `agm init` first." |
| Symlink cycle | Detected by `canonicalize()` failure → show `⚠ error` status + log |
| Background update + user quits (`q`) | Quit immediately; background thread detached (orphaned but harmless — git pull will finish) |
| `␣`/`Enter` behave identically | Both keys trigger the same action everywhere — no difference |

---

## CLI Changes (`main.rs`)

### New Command Structure

```rust
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

enum Commands {
    /// Initialize agm config and directories
    Init,

    /// Manage skill/agent sources
    Source {
        #[arg(short, long)]
        add: Option<String>,
        #[arg(short, long)]
        update: bool,
        #[arg(short, long)]
        list: bool,
        #[arg(long)]
        all: bool,
    },

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
}
```

### Removed Subcommands

The following are **removed** from `Commands` enum:
- `Link` (replaced by `agm tool --link`)
- `Unlink` (replaced by `agm tool --unlink`)
- `Status` (replaced by `agm tool --status`)
- `Config` (replaced by `agm tool` TUI → central config → `e`)
- `Prompt` (replaced by `agm tool` TUI → central/tool prompt → `e`)
- `Auth` (replaced by `agm tool` TUI → tool auth → `e`)
- `Mcp` (replaced by `agm tool` TUI → tool mcp → `e`)

### Routing

```rust
match cli.command {
    None => { /* show help or default action */ }
    Some(Commands::Init) => init::run(config_path),
    Some(Commands::Source { .. }) => { /* existing source logic */ },
    Some(Commands::Tool { link, unlink, status }) => {
        if link {
            // Run link-all logic (from current Link command with target="all", yes=true)
        } else if unlink {
            // Run unlink-all logic (from current Unlink command with target="all")
        } else if status {
            status::status(config_path);
        } else {
            // No flags → launch Tool TUI
            tui::tool::run(config_path)?;
        }
    }
}
```

---

## Migration from manage.rs

### Steps

1. Create `src/tui/` module directory
2. Create `src/tui/mod.rs` with shared types and re-exports
3. Create `src/tui/popup.rs` — ScrollablePopup implementation
4. Create `src/tui/log.rs` — LogBuffer + LogOverlay
5. Create `src/tui/background.rs` — BackgroundTask infrastructure
6. Move `src/manage.rs` → `src/tui/source.rs`:
   - Replace blocking `do_update()` with background task
   - Replace `show_info()` status message with info popup
   - Add log buffer integration (all operations push to log)
   - Add `l` key handler for log toggle
   - Wire up popup key handling (delegate to popup when active)
7. Create `src/tui/tool.rs` — new Tool TUI
8. Update `src/main.rs`:
   - Remove old subcommands (Link, Unlink, Status, Config, Prompt, Auth, Mcp)
   - Add `Tool` subcommand with `--link`/`--unlink`/`--status` flags
   - Move link-all/unlink-all logic to functions callable from tool command
   - Update `mod` declarations

### Preserved Behavior

- `agm tool --link` behaves exactly like current `agm link all --yes`
- `agm tool --unlink` behaves exactly like current `agm unlink all`
- `agm tool --status` behaves exactly like current `agm status`
- Source TUI retains all existing functionality (toggle, search, delete, add, etc.)

---

## skills.rs Changes

### New Function: `update_all_with_progress`

```rust
pub fn update_all_with_progress(
    skills_dir: &Path,
    agents_dir: &Path,
    source_dir: &Path,
    sender: mpsc::Sender<TaskEvent>,
) -> anyhow::Result<()>
```

This is a variant of `update_all` that sends `TaskEvent` messages through the channel for each repo processed, enabling the TUI to show real-time progress.

The existing `update_all` can be kept as-is (used by `agm source --update`) or refactored to call the new function internally.

---

## Visual Reference

See mockups: `docs/superpowers/specs/tui-mockups.html` (or serve locally)

Tabs:
1. Source TUI — Log Overlay (normal + overlay states)
2. Source TUI — Info Popup
3. Tool TUI — Main View (with cursor, link statuses)
4. Tool TUI — File Picker Popup
5. Tool TUI — Path Editor Popup
