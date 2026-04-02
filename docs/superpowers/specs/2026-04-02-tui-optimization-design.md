# TUI Optimization Design

## Overview

Comprehensive TUI optimization for both source and tool interfaces, covering search
highlighting, key binding redesign, tool layout restructuring, dynamic status bar
hints, and info popup improvements.

## 1. Source TUI Changes

### 1.1 Search Highlighting

Replace `SkimMatcherV2::fuzzy_match()` with `fuzzy_indices()` to obtain matched
character positions. During rendering of SkillItem/AgentItem names, apply a highlight
style (bold + contrasting color) to characters at fuzzy-matched positions. Only active
when a search filter is applied.

### 1.2 Key Swap on Entries

Swap `i` and `Space/Enter` on SkillItem and AgentItem rows (normal mode only):

| Key | Before | After |
|-----|--------|-------|
| `i` | Info popup | Toggle install/uninstall |
| `Space/Enter` | Toggle install/uninstall | Info popup |

Headers (CategoryHeader, SourceHeader) retain current `Space/Enter` = expand/collapse.

### 1.3 Search Mode Phases

Search has two phases:

1. **Input phase** (`/` → typing): All keys captured for query input, including Space
   (adds space character to query). No toggle/info actions available.
2. **Display phase** (after Enter): Normal mode with filter active — swapped key
   bindings apply. `Esc` clears filter.

Current behavior where Space toggles install during search input phase is removed.
Space in input phase now adds a space character to the search query.

### 1.4 SourceHeader `i` Key — Bulk Toggle

`i` on SourceHeader triggers a confirmation prompt, then bulk toggles all skills in
that source group:

- All installed → confirmation → uninstall all
- Partially installed → confirmation → install all
- None installed → confirmation → install all

Uses existing `confirm_state` pattern (Y/N prompt in status bar).

## 2. Tool TUI — Central Section

### 2.1 CentralItem(Config/Prompt) — Info Popup

- `Space/Enter` → info popup (ScrollablePopup) showing file content
- In popup: `e` opens the file in external editor (popup closes, editor takes terminal)
- Normal mode `e` key still works directly on these rows

### 2.2 CentralItem(Skills/Agents/Source) — Unchanged

- `Space/Enter` → path editor (current behavior preserved)

## 3. Tool TUI — Interface Restructure

### 3.1 New Row Hierarchy

When a tool is expanded, the children are organized into sub-groups:

```
▼ claude (Claude Code)
  ▼ status   All linked
    prompt ✓ linked → ~/.claude/CLAUDE.md
    skills ✓ linked → ~/.claude/skills
    agents ✓ linked → ~/.claude/agents
  ▼ settings
    ~/.claude.json
    ~/.claude/settings.json
    ~/.claude/settings.local.json
  auth ~/.claude/.credentials.json
  mcp  ~/.claude/settings.json
```

### 3.2 New ToolRow Variants

```rust
enum ToolRow {
    CentralHeader,
    CentralItem(CentralField),
    ToolHeader { key: String, name: String, installed: bool },
    StatusHeader { tool_key: String },                          // NEW
    LinkItem { tool_key: String, field: LinkField },            // NEW
    FileGroupHeader { tool_key: String, group: FileGroup },     // NEW
    FileItem { tool_key: String, group: FileGroup, index: usize }, // NEW
}

enum LinkField { Prompt, Skills, Agents }
enum FileGroup { Settings, Auth, Mcp }
```

Removed: The old `ToolItem` variant is fully replaced by `LinkItem`, `FileGroupHeader`,
and `FileItem`.

### 3.3 Status Summary

StatusHeader displays aggregate link status for the tool's prompt/skills/agents:

| State | Display | Color |
|-------|---------|-------|
| All 3 linked | "All linked" | Green |
| 1–2 linked | "Partially linked" | Yellow |
| None linked | "Not linked" | DarkGray |

### 3.4 Link Path Display

Change the link target display from the central path to the tool's own path:

- Before: `prompt ✓ linked → ~/.local/share/agm/prompts/MASTER.md`
- After: `prompt ✓ linked → ~/.claude/CLAUDE.md`

This shows users the filesystem location they interact with.

### 3.5 FileGroup Behavior

- **Single file**: Display path inline on the group row. `Space/Enter` → info popup.
- **Multiple files**: Show as expandable group with `▼/▶` indicator. `Space/Enter` →
  toggle fold. Individual FileItem rows show file paths.
- **FilePicker popup removed**: Replaced by inline expansion.

### 3.6 Expand Tracking

Extend `HashSet<String>` to support composite keys for sub-sections:
- `"central"` — central section
- `"claude"` — tool section
- `"claude:status"` — status sub-group
- `"claude:settings"` — settings sub-group
- `"claude:auth"` — auth sub-group (when multi-file)
- `"claude:mcp"` — mcp sub-group (when multi-file)

## 4. Key Behavior Matrix

### 4.1 Source TUI (Normal Mode)

| Row Type | Space/Enter | i | e | d |
|---|---|---|---|---|
| CategoryHeader | expand/collapse | — | — | — |
| SourceHeader | expand/collapse | confirm → toggle all skills | — | delete |
| SkillItem | info popup | toggle install | edit SKILL.md | delete |
| AgentItem | info popup | toggle install | edit .md | delete |

### 4.2 Tool TUI

| Row Type | Space/Enter | i | e |
|---|---|---|---|
| CentralHeader | expand/collapse | — | — |
| CentralItem(Config) | info popup | — | edit |
| CentralItem(Prompt) | info popup | — | edit |
| CentralItem(Skills/Agents/Source) | path editor | — | — |
| ToolHeader | expand/collapse | — | — |
| StatusHeader | expand/collapse | link/unlink all | — |
| LinkItem | info popup (`i` toggles link inside) | toggle link | — |
| FileGroupHeader (single file) | info popup (`e` edits inside) | — | edit |
| FileGroupHeader (multi file) | expand/collapse | — | — |
| FileItem | info popup (`e` edits inside) | — | edit |

### 4.3 Popup Action Keys

| Popup Context | Key | Action |
|---|---|---|
| LinkItem info popup | `i` | Toggle link, refresh popup content |
| CentralItem info popup | `e` | Open in editor, close popup |
| FileGroup/FileItem info popup | `e` | Open in editor, close popup |
| All info popups | `Esc` | Close popup |
| All info popups | Scroll keys | Standard scroll (j/k, PgUp/PgDn, Home/End) |

### 4.4 StatusHeader `i` Logic

| Current State | Action |
|---|---|
| All linked | Unlink all 3 (prompt, skills, agents) |
| Partially linked | Link all 3 |
| None linked | Link all 3 |

## 5. Dynamic Status Bar Hints

Hints update based on the currently selected row type. The `0/9` fold/unfold keys
remain functional but are hidden from hints.

### 5.1 Tool TUI Hints

| Selected Row | Hints |
|---|---|
| CentralHeader / ToolHeader | `␣/⏎ toggle  l log  q quit` |
| CentralItem(Config/Prompt) | `␣/⏎ info  e edit  l log  q quit` |
| CentralItem(Skills/Agents/Source) | `␣/⏎ edit path  l log  q quit` |
| StatusHeader | `␣/⏎ toggle  i link  l log  q quit` |
| LinkItem | `␣/⏎ info  i link  l log  q quit` |
| FileGroupHeader (single file) | `␣/⏎ info  e edit  l log  q quit` |
| FileGroupHeader (multi file) | `␣/⏎ toggle  l log  q quit` |
| FileItem | `␣/⏎ info  e edit  l log  q quit` |

### 5.2 Source TUI Hints

| Selected Row | Hints |
|---|---|
| CategoryHeader | `␣/⏎ toggle  a add  u update  / search  l log  q quit` |
| SourceHeader | `␣/⏎ toggle  i install all  d del  u update  / search  l log  q quit` |
| SkillItem | `␣/⏎ info  i install  e edit  d del  / search  l log  q quit` |
| AgentItem | `␣/⏎ info  i install  e edit  / search  l log  q quit` |

## 6. Info Popup Content

### 6.1 Source TUI Info Popups

Content unchanged from current implementation. Triggered by `Space/Enter` instead of
`i` after the swap.

- **SkillItem**: Name, source, path, status, file listing, SKILL.md content
- **AgentItem**: Name, source, path, status, agent .md content
- **SourceHeader**: Name, type, path, skill/agent counts and listings

### 6.2 Tool TUI Info Popups — New

| Row Type | Content |
|---|---|
| CentralItem(Config) | config.toml file content |
| CentralItem(Prompt) | Prompt file content |
| LinkItem(Prompt) | Link status + prompt file content |
| LinkItem(Skills) | Link status + directory listing + stats (N installed / N total) |
| LinkItem(Agents) | Link status + directory listing + stats (N installed / N total) |
| FileGroupHeader (single file) | File content |
| FileItem | File content |

## 7. Impact Analysis — Unchanged Areas

The following areas are **not** modified by this design:

1. **`r` refresh, `a` add, `u` update, `d` delete** in source TUI — unchanged
2. **Log popup** in both TUIs — unchanged
3. **Path editor** for central Skills/Agents/Source — unchanged
4. **Blocked link handling** — unchanged logic, triggered from new key bindings
5. **Delete confirmation** (normal and migrated sources) — unchanged
6. **Background task** for source updates — unchanged

## 8. Implementation Notes

### 8.1 PopupState Changes (tool.rs)

- **Add**: `Info(ScrollablePopup)` variant
- **Remove**: `FilePicker` variant and all associated handling code

### 8.2 ScrollablePopup Integration

Action keys (`e`, `i`) are handled by the parent after `PopupAction::Ignored` is
returned from `ScrollablePopup::handle_key()`. No changes needed to ScrollablePopup
itself.

To refresh popup content after `i` toggles a link: rebuild `ScrollablePopup.lines`
and reset scroll position.

### 8.3 Expand Tracking

Current `HashSet<String>` with keys `"central"` and tool keys (e.g., `"claude"`) is
extended with composite keys for sub-sections: `"tool_key:status"`,
`"tool_key:settings"`, `"tool_key:auth"`, `"tool_key:mcp"`.

### 8.4 SourceHeader Confirm State

Extend `ConfirmState` with a new variant for bulk skill toggle:

```rust
enum ConfirmState {
    Normal { group_index: usize },
    Migrated { group_index: usize, typed: String },
    BulkToggle { group_index: usize, install: bool }, // NEW
}
```

`install: bool` indicates whether confirming install-all or uninstall-all.
