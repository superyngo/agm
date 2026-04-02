# AGM TUI Optimizations — Design Spec

**Date:** 2026-04-02
**Scope:** 8 improvements to the AGM TUI (tool.rs, source.rs, popup.rs)
**Branch:** `feat/tui-redesign`

---

## 1. Info Popup: Page-Based Position Indicator

### Problem

The info popup (`popup.rs`) displays `[current_line/total_lines]` at the bottom-right. Users expect page-based navigation feedback, not raw line numbers.

### Current Code

```rust
// popup.rs:108-110
let current_line = self.scroll_offset + 1;
let total_lines = self.lines.len();
let position_text = format!("[{}/{}]", current_line, total_lines);
```

### Design

Dynamically compute page numbers from `scroll_offset` and `visible_height`:

```
current_page = scroll_offset / visible_height + 1
total_pages  = ceil(lines.len() / visible_height)
display      = "[1/5]"
```

**Edge cases:**
- `visible_height == 0` → show `[1/1]`
- All content fits in one page → show `[1/1]`
- `scroll_offset` at last partial page → show `[N/N]` (clamp current_page to total_pages)

**Note on Wrap:** `Paragraph` uses `Wrap { trim: true }`, so wrapped long lines may cause the actual rendered line count to exceed `lines.len()`. Ratatui does not expose post-wrap line count. We accept the estimation based on logical lines — this is accurate for the vast majority of popup content (short metadata lines + code).

### Files Changed

- `src/tui/popup.rs`: Replace line-based indicator with page-based in `render()`

---

## 2. Source Search: Preserve Query on Re-Entry

### Problem

When the user presses Enter in search mode, the filter stays active (display state). But pressing `/` again to refine the search clears `search_query` and `filtered_rows`, losing the previous search.

### Current Code

```rust
// source.rs:1051-1057
KeyCode::Char('/') => {
    self.search_mode = true;
    self.search_query.clear();      // ← clears previous search
    self.filtered_rows = None;       // ← removes filter
    self.expand_all();
}
```

### Design

When entering search mode via `/`:
- Set `self.search_mode = true`
- **Do not** clear `search_query` or `filtered_rows`
- Call `self.expand_all()` (still needed for visibility)

This allows the user to see and continue editing their previous search query. If they want a fresh search, they can select-all/backspace.

### Files Changed

- `src/tui/source.rs`: Remove the two `.clear()` / `= None` lines in the `/` handler

---

## 3. Source Search: Prevent Toggled Sources from Disappearing

### Problem

In search display mode (filter active, `search_mode == false`), toggling (collapsing) a SourceHeader calls `rebuild_rows()` → `apply_search_filter()`. Since collapsed sources have no child rows, the filter removes the source header entirely.

### Root Cause

`apply_search_filter()` determines matching groups by scanning `self.rows` — but collapsed sources have no child skill/agent rows in `self.rows`, so nothing matches.

### Design

Refactor `apply_search_filter()` to determine matches from `self.groups` data directly, not from `self.rows`:

1. **Phase 1: Scan groups for matches** — iterate `self.groups[i].skills` and `.agents`, run fuzzy match against names. Build `matching_groups_skills: HashSet<usize>` and `matching_groups_agents: HashSet<usize>`.

2. **Phase 2: Build filtered row indices** — iterate `self.rows`, include:
   - `CategoryHeader` if the corresponding matching set is non-empty
   - `SourceHeader` if its `group_index` is in the matching set
   - `SkillItem`/`AgentItem` if its name fuzzy-matches the query

This decouples filtering from expansion state, so collapsed sources remain visible as long as they have matching children.

### Files Changed

- `src/tui/source.rs`: Rewrite `apply_search_filter()` — Phase 1 scans `self.groups`, Phase 2 scans `self.rows`

---

## 4. Source Search: Performance Optimization

### Problem

Search input feels sluggish. Each keystroke triggers `apply_search_filter()` which scans all rows twice.

### Analysis

1. `visible_items.contains(&i)` (line 273) uses `Vec<usize>` — O(n) per check, O(n²) total
2. Full re-scan on every keystroke
3. `SkimMatcherV2::fuzzy_match()` is fast per call, but n × m calls add up

### Design

**Phase 1 (immediate):** Change `visible_items` from `Vec<usize>` to `HashSet<usize>` — O(1) per lookup, dramatic improvement for large skill sets.

**Phase 2 (if needed):** Incremental search — when the new query is a strict prefix extension of the previous query, only search within the previous match set. Add a field `last_search_query: String` and `last_matching_items: HashSet<usize>` to `App` for caching.

**Phase 3 (if needed):** Debounce — use `crossterm::event::poll(Duration)` timeout to batch rapid keystrokes. Only call `apply_search_filter()` if no key arrives within 50ms.

Start with Phase 1 only. Phases 2-3 are deferred unless performance is still insufficient.

### Files Changed

- `src/tui/source.rs`: Change `visible_items` from `Vec` to `HashSet` in `apply_search_filter()`

---

## 5. Tool Prompt: Edit Tool's Own File When Unlinked

### Problem

Pressing `e` on a tool's prompt `LinkItem` when unlinked opens `MASTER.md` (the central prompt) instead of the tool's own prompt file.

### Current Code

```rust
// tool.rs:1076-1090
ToolRow::LinkItem { tool_key, field: LinkField::Prompt } => {
    let path = match status {
        LinkStatus::Linked => target,         // MASTER.md — correct
        _ if link_path.exists() => link_path,  // tool's file — correct
        _ => target,                           // ← BUG: falls back to MASTER.md
    };
    ...
}
```

### Design

Change the fallback from `target` to `link_path`:

```rust
let path = match status {
    LinkStatus::Linked => target,
    _ => link_path,  // always use tool's own path when not linked
};
```

If `link_path` does not exist, the existing `ConfirmCreate` flow handles it (prompts user to create the file).

### Files Changed

- `src/tui/tool.rs`: Simplify the match in `handle_edit()` for `LinkItem::Prompt`

---

## 6. Unlink Skills/Agents: Move Items Back to Tool Directory

### Problem

When unlinking skills/agents, `recover_after_unlink()` looks in `agm_tools/{tool}/skills/` and `agm_tools/{tool}/agents/` subdirectories. But migrated skills are stored directly under `agm_tools/{tool}/` (not in a `skills/` subdirectory). The agents directory is `agm_tools/{tool}/agents/`.

### Actual Structure

```
~/.local/share/agm/source/agm_tools/codex/
  skill-installer/     ← skill (contains SKILL.md)
  plugin-creator/      ← skill (contains SKILL.md)
  openai-docs/         ← skill (contains SKILL.md)
  imagegen/            ← skill (contains SKILL.md)
  codex_skill-creator/ ← skill (contains SKILL.md)
  agents/              ← agents directory (contains .md files)
```

### Design

Rewrite `recover_after_unlink()` for Skills and Agents:

**Skills unlink:**
1. Remove the symlink at tool's `skills_dir` path (e.g., `~/.codex/skills`)
2. Create the tool's `skills_dir` as a real directory
3. Scan `agm_tools/{tool}/` for all directories that:
   - Are NOT named `agents`
   - Contain a `SKILL.md` file
4. Move each matching directory into the tool's skills directory
5. After moving, check if `agm_tools/{tool}/` is empty → remove it

**Agents unlink:**
1. Remove the symlink at tool's `agents_dir` path (e.g., `~/.codex/agents`)
2. Move `agm_tools/{tool}/agents/` to tool's agents path (rename the entire directory)
3. After moving, check if `agm_tools/{tool}/` is empty → remove it

**Edge cases:**
- `agm_tools/{tool}/` does not exist → skip recovery, just create empty dir
- Destination already has a file/dir with same name → skip that item, log warning
- Only some items moved → still clean up if source becomes empty

### Files Changed

- `src/tui/tool.rs`: Rewrite `recover_after_unlink()` — separate logic for Skills vs Agents

---

## 7. Unlinked Status: Show Configured Path and Info Content

### Problem

When a link is in `Missing` (not linked) state:
1. The list row only shows `✗ not linked` without showing the configured path
2. The info popup should show the tool's own directory contents

### Design

**List row display:**

Modify `link_status_spans()` to accept the tool's configured path and display it for `Missing` status:

```
│ skills  ✗ not linked → ~/.codex/skills
│ agents  ✗ not linked → ~/.codex/agents
```

Change function signature:
```rust
fn link_status_spans(status: &LinkStatus, link_path: &Path, tool_path: &Path) -> Vec<Span<'static>>
```

For `Missing`, append ` → {contract_tilde(tool_path)}`.

**Info popup:** The existing `show_link_info()` already uses `link_path` for unlinked state to scan directory contents. This works correctly as long as the tool's directory exists. No change needed for the popup itself.

### Files Changed

- `src/tui/tool.rs`: Update `link_status_spans()` signature and `Missing` branch; update all call sites

---

## 8. ToolHeader: `e` Hotkey for Config Section Editing

### Problem

No way to quickly edit a tool's configuration section from the TUI. Users must manually find and edit `config.toml`.

### Design

When pressing `e` on a `ToolHeader` row:

1. **Extract section:** Read `config.toml` as raw text. Find the range from `[tools.{key}]` to the next `[tools.*]` or `[central]` or EOF. Include trailing blank lines.

2. **Write temp file:** Use `tempfile::NamedTempFile` (keep it alive). Write the extracted section. Filename: `agm-{tool_key}-XXXXXX.toml`.

3. **Open editor:** Launch configured editor on the temp file.

4. **Read back:** After editor exits, read the temp file content.

5. **Validate:** Wrap content in a dummy `[tools.{key}]` header if missing, attempt `toml::from_str::<toml::Value>()` to validate syntax.

6. **Replace section:** In the original config.toml text, replace the old section range with the new content. Write back to disk.

7. **Reload:** Call `Config::load_from()` to pick up changes. Rebuild rows.

8. **Cleanup:** Temp file auto-removed when `NamedTempFile` drops.

**Section extraction strategy:**

Use line-by-line scanning (not regex) for robustness:
- Find line matching `[tools.{key}]` exactly
- Collect all lines until next top-level section header (`[xxx]` pattern where `xxx` does not start with `tools.{key}.`)
- This preserves comments, formatting, and sub-tables like `[tools.{key}.extra]`

**Error handling:**
- If section not found in config → show error status
- If edited TOML is invalid → show error, ask to re-edit or discard
- If file write fails → show error, original preserved

**Hint bar update:**

Add `e` hint to `ToolHeader` in `build_tool_hints()`.

### Files Changed

- `src/tui/tool.rs`:
  - Add `handle_edit()` match arm for `ToolRow::ToolHeader`
  - New helper: `extract_tool_section()` and `replace_tool_section()`
  - Update `build_tool_hints()` for ToolHeader

---

## Implementation Order

| Phase | Items | Rationale |
|-------|-------|-----------|
| 1 | #2, #5 | Bug fixes, minimal code changes |
| 2 | #1, #7, #4 | Display improvements + perf, low risk |
| 3 | #3, #6 | Logic refactoring, moderate risk |
| 4 | #8 | New feature, highest complexity |

---

## Testing Strategy

- **#1:** Unit test `page_calculation()` with various line counts and visible heights
- **#2:** Manual TUI test: search → Enter → `/` → verify query preserved
- **#3:** Unit test `apply_search_filter()` with collapsed groups
- **#4:** Verify `HashSet` usage in filter; benchmark optional
- **#5:** Manual test: unlink prompt → press `e` → confirm opens tool's file
- **#6:** Integration test: link → unlink → verify files moved back, empty dir cleaned
- **#7:** Manual TUI test: unlinked tool shows `→ path` and info popup content
- **#8:** Integration test: edit section → validate TOML → verify config updated
