# AGM TUI Optimizations — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement 8 TUI improvements covering popup page indicator, search state preservation, search filter decoupling, search performance, prompt edit path fix, unlink recovery, unlinked status display, and ToolHeader config editor.

**Architecture:** All changes are within the `src/tui/` module (popup.rs, source.rs, tool.rs). Changes are grouped into 7 tasks ordered by dependency and risk — bug fixes first, then display improvements, then logic refactoring, then new features.

**Tech Stack:** Rust, ratatui, crossterm, fuzzy-matcher, toml

---

## File Structure

| File | Changes |
|------|---------|
| `src/tui/popup.rs` | Task 1: page-based position indicator |
| `src/tui/source.rs` | Task 2: search preserve; Task 4: filter rewrite + perf |
| `src/tui/tool.rs` | Task 3: prompt edit fix; Task 5: unlinked display; Task 6: unlink recovery rewrite; Task 7: ToolHeader config editor |

---

### Task 1: Popup Page-Based Position Indicator

**Spec items:** #1 (page indicator)
**Files:**
- Modify: `src/tui/popup.rs:106-122` (render method)
- Modify: `src/tui/popup.rs:152+` (tests)

- [ ] **Step 1: Write failing test for page calculation**

Add to the `#[cfg(test)] mod tests` block at the bottom of `src/tui/popup.rs`:

```rust
#[test]
fn test_page_indicator_values() {
    // 20 lines, visible_height = 5 → 4 pages
    let lines = (1..=20).map(|i| Line::from(format!("line {}", i))).collect();
    let mut popup = ScrollablePopup::new("Test", lines);
    popup.visible_height = 5;

    // Page 1: offset 0
    assert_eq!(popup.current_page(), 1);
    assert_eq!(popup.total_pages(), 4);

    // Page 2: offset 5
    popup.scroll_offset = 5;
    assert_eq!(popup.current_page(), 2);

    // Page 4: offset 15
    popup.scroll_offset = 15;
    assert_eq!(popup.current_page(), 4);

    // Clamp: offset beyond max
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
fn test_page_indicator_empty() {
    let popup = ScrollablePopup::new("Test", vec![]);
    assert_eq!(popup.current_page(), 1);
    assert_eq!(popup.total_pages(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_page_indicator -- --nocapture`
Expected: FAIL — `current_page` and `total_pages` methods don't exist yet.

- [ ] **Step 3: Implement page calculation methods**

In `src/tui/popup.rs`, add methods to the `impl ScrollablePopup` block (before `scroll_up`):

```rust
pub fn current_page(&self) -> usize {
    if self.visible_height == 0 || self.lines.is_empty() {
        return 1;
    }
    let page = self.scroll_offset / self.visible_height + 1;
    page.min(self.total_pages())
}

pub fn total_pages(&self) -> usize {
    if self.visible_height == 0 || self.lines.is_empty() {
        return 1;
    }
    (self.lines.len() + self.visible_height - 1) / self.visible_height
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_page_indicator -- --nocapture`
Expected: All 3 tests PASS.

- [ ] **Step 5: Update render method to use page-based indicator**

In `src/tui/popup.rs`, replace the position indicator section in `render()` (lines 106-123):

Replace:
```rust
        // Render position indicator at bottom-right
        if !self.lines.is_empty() {
            let current_line = self.scroll_offset + 1;
            let total_lines = self.lines.len();
            let position_text = format!("[{}/{}]", current_line, total_lines);
```

With:
```rust
        // Render page indicator at bottom-right
        if !self.lines.is_empty() {
            let position_text = format!("[{}/{}]", self.current_page(), self.total_pages());
```

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/tui/popup.rs
git commit -m "feat: popup page-based position indicator [#1]

Replace line-based [1/88] with page-based [1/5] indicator.
Dynamically computed from scroll_offset and visible_height.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 2: Source Search Preserve Query on Re-Entry

**Spec items:** #2 (search preserve)
**Files:**
- Modify: `src/tui/source.rs:1051-1057`

- [ ] **Step 1: Update search mode entry to preserve state**

In `src/tui/source.rs`, find the `/` key handler in the normal mode match block (around line 1051-1057):

Replace:
```rust
            KeyCode::Char('/') => {
                self.search_mode = true;
                self.search_query.clear();
                self.filtered_rows = None;
                // Expand all for search visibility
                self.expand_all();
            }
```

With:
```rust
            KeyCode::Char('/') => {
                self.search_mode = true;
                // Preserve existing search_query and filtered_rows
                // so user can continue editing their previous search
                self.expand_all();
            }
```

- [ ] **Step 2: Run test suite**

Run: `cargo test`
Expected: All tests pass. No existing tests depend on search clearing behavior.

- [ ] **Step 3: Commit**

```bash
git add src/tui/source.rs
git commit -m "feat: preserve search query when re-entering search mode [#2]

Pressing / no longer clears the previous search. Users can
continue editing their query or backspace to start fresh.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 3: Tool Prompt Edit Path Fix

**Spec items:** #5 (prompt edit path when unlinked)
**Files:**
- Modify: `src/tui/tool.rs:1076-1090`

- [ ] **Step 1: Fix fallback path in handle_edit for prompt LinkItem**

In `src/tui/tool.rs`, find the `LinkItem { Prompt }` arm in `handle_edit()` (around line 1076-1090):

Replace:
```rust
            ToolRow::LinkItem { tool_key, field: LinkField::Prompt } => {
                if let Some((link_path, target, _, _)) = self.get_link_paths(tool_key, &LinkField::Prompt) {
                    let status = linker::check_link(&link_path, &target, false);
                    let path = match status {
                        LinkStatus::Linked => target,
                        _ if link_path.exists() => link_path,
                        _ => target,
                    };
                    if path.exists() {
                        self.open_in_editor(terminal, &[path]);
                    } else {
                        self.popup = Some(PopupState::ConfirmCreate { path });
                    }
                }
            }
```

With:
```rust
            ToolRow::LinkItem { tool_key, field: LinkField::Prompt } => {
                if let Some((link_path, target, _, _)) = self.get_link_paths(tool_key, &LinkField::Prompt) {
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
```

- [ ] **Step 2: Run test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/tui/tool.rs
git commit -m "fix: edit tool's own prompt file when unlinked [#5]

When not linked, 'e' on prompt now opens the tool's own prompt
path (e.g. ~/.codex/AGENTS.md) instead of falling back to
MASTER.md. If file doesn't exist, prompts to create it.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 4: Source Search Filter Decoupling + Performance

**Spec items:** #3 (toggle doesn't hide sources) + #4 (search performance)
**Files:**
- Modify: `src/tui/source.rs:210-280` (apply_search_filter)

- [ ] **Step 1: Rewrite apply_search_filter with group-based matching**

In `src/tui/source.rs`, replace the entire `apply_search_filter` method (lines 210-280):

Replace:
```rust
    fn apply_search_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_rows = None;
            return;
        }
        let query = &self.search_query;
        let mut matching_groups_skills = HashSet::new();
        let mut matching_groups_agents = HashSet::new();
        let mut visible_items = Vec::new();

        // Find matching items
        for (i, row) in self.rows.iter().enumerate() {
            match row {
                ListRow::SkillItem {
                    group_index,
                    skill_index,
                } => {
                    let skill = &self.groups[*group_index].skills[*skill_index];
                    if self.matcher.fuzzy_match(&skill.name, query).is_some() {
                        matching_groups_skills.insert(*group_index);
                        visible_items.push(i);
                    }
                }
                ListRow::AgentItem {
                    group_index,
                    agent_index,
                } => {
                    let agent = &self.groups[*group_index].agents[*agent_index];
                    if self.matcher.fuzzy_match(&agent.name, query).is_some() {
                        matching_groups_agents.insert(*group_index);
                        visible_items.push(i);
                    }
                }
                _ => {}
            }
        }

        // Build filtered list: include headers for matching groups
        let mut result = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            match row {
                ListRow::CategoryHeader { category } => {
                    let has_matches = match category {
                        Category::Skills => !matching_groups_skills.is_empty(),
                        Category::Agents => !matching_groups_agents.is_empty(),
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
                    };
                    if is_match {
                        result.push(i);
                    }
                }
                ListRow::SkillItem { .. } | ListRow::AgentItem { .. } => {
                    if visible_items.contains(&i) {
                        result.push(i);
                    }
                }
            }
        }
        self.filtered_rows = Some(result);
    }
```

With:
```rust
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
        let mut matching_skills: HashSet<(usize, usize)> = HashSet::new();
        let mut matching_agents: HashSet<(usize, usize)> = HashSet::new();

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
        }

        // Phase 2: Build filtered row indices from self.rows
        let mut result = Vec::new();
        for (i, row) in self.rows.iter().enumerate() {
            match row {
                ListRow::CategoryHeader { category } => {
                    let has_matches = match category {
                        Category::Skills => !matching_groups_skills.is_empty(),
                        Category::Agents => !matching_groups_agents.is_empty(),
                    };
                    if has_matches {
                        result.push(i);
                    }
                }
                ListRow::SourceHeader { category, group_index } => {
                    let is_match = match category {
                        Category::Skills => matching_groups_skills.contains(group_index),
                        Category::Agents => matching_groups_agents.contains(group_index),
                    };
                    if is_match {
                        result.push(i);
                    }
                }
                ListRow::SkillItem { group_index, skill_index } => {
                    if matching_skills.contains(&(*group_index, *skill_index)) {
                        result.push(i);
                    }
                }
                ListRow::AgentItem { group_index, agent_index } => {
                    if matching_agents.contains(&(*group_index, *agent_index)) {
                        result.push(i);
                    }
                }
            }
        }
        self.filtered_rows = Some(result);
    }
```

**Key improvements:**
1. Phase 1 scans `self.groups` data — decoupled from row expansion state (fixes #3)
2. All lookups use `HashSet` — O(1) instead of O(n) (fixes #4)

- [ ] **Step 2: Run test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/tui/source.rs
git commit -m "feat: decouple search filter from expansion state + HashSet perf [#3, #4]

Rewrite apply_search_filter() to determine matches from
self.groups data directly rather than self.rows. This prevents
collapsed sources from disappearing when filter is active.
Also uses HashSet for O(1) lookups instead of Vec::contains.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 5: Unlinked Status Display with Path

**Spec items:** #7 (show path for unlinked items)
**Files:**
- Modify: `src/tui/tool.rs:1166-1188` (link_status_spans function)

- [ ] **Step 1: Add path display for Missing status**

In `src/tui/tool.rs`, find the `link_status_spans` function (around line 1166-1188).

Replace:
```rust
        LinkStatus::Missing => vec![
            Span::styled("✗ not linked", Style::default().fg(Color::Yellow)),
        ],
```

With:
```rust
        LinkStatus::Missing => vec![
            Span::styled("✗ not linked", Style::default().fg(Color::Yellow)),
            Span::raw(format!(" → {}", contract_tilde(link_path))),
        ],
```

This makes the list row display:
```
skills  ✗ not linked → ~/.codex/skills
agents  ✗ not linked → ~/.codex/agents
```

The info popup (`show_link_info`) already handles unlinked directories correctly — it uses `link_path` to scan the tool's own directory when not linked.

- [ ] **Step 2: Run test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/tui/tool.rs
git commit -m "feat: show configured path for unlinked items [#7]

Unlinked skills/agents now display their configured directory
path (e.g., '✗ not linked → ~/.codex/skills') instead of just
'✗ not linked'. Info popup already shows directory contents.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 6: Unlink Skills/Agents Recovery Rewrite

**Spec items:** #6 (move items back to tool directory on unlink)
**Files:**
- Modify: `src/tui/tool.rs:686-754` (recover_after_unlink method)

- [ ] **Step 1: Add cleanup_empty_tool_store helper**

In `src/tui/tool.rs`, add a new helper method inside the `impl ToolApp` block (after `recover_after_unlink`):

```rust
    fn cleanup_empty_tool_store(&self, tool_store: &std::path::Path) {
        if !tool_store.exists() { return; }
        if let Ok(mut entries) = std::fs::read_dir(tool_store) {
            if entries.next().is_none() {
                let _ = std::fs::remove_dir(tool_store);
            }
        }
    }
```

- [ ] **Step 2: Rewrite recover_after_unlink for Skills**

In `src/tui/tool.rs`, find the `LinkField::Skills | LinkField::Agents` arm in `recover_after_unlink` (around lines 719-753).

Replace the entire `LinkField::Skills | LinkField::Agents => { ... }` block with two separate arms:

```rust
            LinkField::Skills => {
                let source_dir = expand_tilde(&self.config.central.source_dir);
                let tool_store = source_dir.join("agm_tools").join(tool_key);
                if !tool_store.exists() {
                    let _ = std::fs::create_dir_all(link_path);
                    return;
                }
                // Create the tool's skills directory
                if let Err(e) = std::fs::create_dir_all(link_path) {
                    self.log.push(LogLevel::Warning,
                        format!("[{}] Failed to create skills dir: {}", tool_key, e));
                    return;
                }
                // Move skill directories: any dir in tool_store that
                // is not "agents" and contains SKILL.md
                let mut count = 0usize;
                if let Ok(entries) = std::fs::read_dir(&tool_store) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name == "agents" { continue; }
                        let path = entry.path();
                        if !path.is_dir() { continue; }
                        if !path.join("SKILL.md").exists() { continue; }
                        let dest = link_path.join(&name);
                        if dest.exists() { continue; }
                        if std::fs::rename(&path, &dest).is_ok() {
                            count += 1;
                        }
                    }
                }
                if count > 0 {
                    self.log.push(LogLevel::Info,
                        format!("[{}] Restored {} skill(s)", tool_key, count));
                }
                self.cleanup_empty_tool_store(&tool_store);
            }
            LinkField::Agents => {
                let source_dir = expand_tilde(&self.config.central.source_dir);
                let tool_store = source_dir.join("agm_tools").join(tool_key);
                let agents_store = tool_store.join("agents");
                if !agents_store.exists() {
                    return;
                }
                // Move entire agents directory to tool's agents path
                match std::fs::rename(&agents_store, link_path) {
                    Ok(()) => {
                        self.log.push(LogLevel::Info,
                            format!("[{}] Restored agents directory", tool_key));
                    }
                    Err(e) => {
                        self.log.push(LogLevel::Warning,
                            format!("[{}] Failed to restore agents: {}", tool_key, e));
                    }
                }
                self.cleanup_empty_tool_store(&tool_store);
            }
```

Also remove the now-unused `_ => return,` line that was inside the old combined match arm.

- [ ] **Step 3: Run test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/tui/tool.rs
git commit -m "fix: unlink moves skills/agents back to tool directory [#6]

Skills: scan agm_tools/{tool}/ for directories containing
SKILL.md (excluding 'agents') and move them to tool's skills dir.
Agents: move agm_tools/{tool}/agents/ as tool's agents dir.
Clean up empty agm_tools/{tool}/ directory after recovery.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

### Task 7: ToolHeader Config Section Editor

**Spec items:** #8 (edit tool config section with e key)
**Files:**
- Modify: `src/tui/tool.rs` (handle_edit, build_tool_hints, new helper functions)

- [ ] **Step 1: Write failing tests for section extraction and replacement**

Add to the `#[cfg(test)] mod tests` block at the bottom of `src/tui/tool.rs` (create one if it doesn't exist):

```rust
#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(section.len(), 5); // header + 4 fields + blank line
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
        // Other tools preserved
        assert!(new_config.contains("[tools.claude]"));
        assert!(new_config.contains("[tools.copilot]"));
    }

    #[test]
    fn test_replace_tool_section_not_found() {
        let result = replace_tool_section(SAMPLE_CONFIG, "nonexistent", "whatever");
        assert!(result.is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib tui::tool::tests -- --nocapture`
Expected: FAIL — `extract_tool_section` and `replace_tool_section` functions don't exist.

- [ ] **Step 3: Implement helper functions**

Add these module-level functions in `src/tui/tool.rs` (before the `impl ToolApp` block, after the `build_rows` function):

```rust
/// Extract a [tools.{key}] section from raw config text.
/// Returns (section_lines, start_line_index, end_line_index).
fn extract_tool_section(config_text: &str, tool_key: &str) -> Option<(Vec<String>, usize, usize)> {
    let header = format!("[tools.{}]", tool_key);
    let lines: Vec<&str> = config_text.lines().collect();

    let start = lines.iter().position(|l| l.trim() == header)?;

    let sub_prefix = format!("[tools.{}.", tool_key);
    let mut end = lines.len();
    for i in (start + 1)..lines.len() {
        let trimmed = lines[i].trim();
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
    for i in (start + 1)..lines.len() {
        let trimmed = lines[i].trim();
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib tui::tool::tests -- --nocapture`
Expected: All 6 tests PASS.

- [ ] **Step 5: Add ToolHeader match arm to handle_edit**

In `src/tui/tool.rs`, find `handle_edit()` (around line 1033). Add a new match arm at the beginning of the match block, before `ToolRow::CentralItem`:

```rust
            ToolRow::ToolHeader { key, .. } => {
                use super::log::LogLevel;
                let config_path = self.config_path.clone()
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

                self.open_in_editor(terminal, &[tmp_path.clone()]);

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
                    self.log.push(LogLevel::Error, format!("[{}] Invalid TOML syntax, changes discarded", key));
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

                let new_config_text = match replace_tool_section(&config_text, key, new_section.trim_end()) {
                    Some(t) => t,
                    None => {
                        self.set_status(format!("✗ Section [tools.{}] not found for replacement", key));
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
                    self.log.push(LogLevel::Success, format!("[{}] Config updated", key));
                    self.set_status(format!("✓ {} config updated", key));
                } else {
                    self.log.push(LogLevel::Error, format!("[{}] Config reload failed after edit", key));
                    self.set_status("✗ Config reload failed");
                }
            }
```

- [ ] **Step 6: Update build_tool_hints to show e key for ToolHeader**

In `src/tui/tool.rs`, find `build_tool_hints()` (around line 1226). Split the combined `CentralHeader | ToolHeader` match arm:

Replace:
```rust
        Some(ToolRow::CentralHeader) | Some(ToolRow::ToolHeader { .. }) => {
            spans.extend([hint_key("␣/⏎"), hint_text(" toggle  ")]);
            spans.extend([hint_key("l"), hint_text(" log  ")]);
            spans.extend([hint_key("q"), hint_text(" quit")]);
        }
```

With:
```rust
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
```

- [ ] **Step 7: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/tui/tool.rs
git commit -m "feat: ToolHeader 'e' hotkey to edit config section [#8]

Press 'e' on a ToolHeader to extract its [tools.{key}] section
from config.toml into a temp file, edit it, validate TOML syntax,
and write changes back to the config. Hint bar updated.

Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

---

## Final Verification

- [ ] **Run full build and test suite**

```bash
cargo build && cargo test
```

- [ ] **Manual TUI verification checklist**

Run `cargo run -- source` and verify:
- [ ] Info popup shows `[1/3]` page indicator (not `[1/45]`)
- [ ] Press `/`, type search, Enter, then `/` again — previous query preserved
- [ ] In search display mode, collapse a source — source header stays visible
- [ ] Search input feels responsive (no lag on keystroke)

Run `cargo run -- tool` and verify:
- [ ] On unlinked prompt, press `e` — opens tool's own prompt file, not MASTER.md
- [ ] Unlink skills → items move from `agm_tools/{tool}/` to tool's skills dir
- [ ] Unlink agents → `agents/` dir moves to tool's agents path
- [ ] After unlink, empty `agm_tools/{tool}/` dir is cleaned up
- [ ] Unlinked items show `✗ not linked → ~/.codex/skills`
- [ ] Info popup for unlinked skills shows directory contents
- [ ] Press `e` on ToolHeader → opens editor with tool's config section
- [ ] After editing, config reloads and rows update
