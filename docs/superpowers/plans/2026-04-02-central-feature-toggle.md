# Central Feature Toggle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add global enable/disable toggles for AI features (prompt, skills, agents, commands) in the central config, with TUI visual feedback and CLI guards.

**Architecture:** A `disabled: Vec<String>` field on `CentralConfig` persists the toggle state. The TUI's `i` key on central AI feature items triggers a confirmation popup, then batch-unlinks/links all installed tools. Disabled features render grayed out across tool view, source view, and CLI status.

**Tech Stack:** Rust, ratatui, serde/toml, crossterm

---

### Task 1: Add `disabled` field to CentralConfig

**Files:**
- Modify: `src/config.rs:16-27` (CentralConfig struct)
- Modify: `src/config.rs:29-37` (impl CentralConfig)
- Test: `src/config.rs` (#[cfg(test)] module, lines 262+)

- [ ] **Step 1: Write failing test for `disabled` field serialization roundtrip**

Add to the existing `#[cfg(test)]` module in `src/config.rs`:

```rust
#[test]
fn test_disabled_field_roundtrip() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let mut config = Config::default_config();
    config.central.disabled = vec!["skills".to_string(), "agents".to_string()];
    config.save_to(&config_path).unwrap();

    let loaded = Config::load_from(Some(config_path)).unwrap();
    assert_eq!(loaded.central.disabled, vec!["skills", "agents"]);
}

#[test]
fn test_disabled_field_default_empty() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");

    let config = Config::default_config();
    config.save_to(&config_path).unwrap();

    let loaded = Config::load_from(Some(config_path)).unwrap();
    assert!(loaded.central.disabled.is_empty());
}

#[test]
fn test_is_disabled() {
    let mut config = Config::default_config();
    assert!(!config.central.is_disabled("skills"));

    config.central.disabled = vec!["skills".to_string()];
    assert!(config.central.is_disabled("skills"));
    assert!(!config.central.is_disabled("prompt"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test test_disabled_field_roundtrip test_disabled_field_default_empty test_is_disabled -- --nocapture`
Expected: FAIL — `disabled` field and `is_disabled()` not defined

- [ ] **Step 3: Add `disabled` field and `is_disabled` method**

In `src/config.rs`, add to the `CentralConfig` struct (after `source_repos`):

```rust
#[serde(default)]
pub disabled: Vec<String>,
```

Add to `impl CentralConfig` block:

```rust
pub fn is_disabled(&self, feature: &str) -> bool {
    self.disabled.iter().any(|d| d == feature)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test test_disabled_field_roundtrip test_disabled_field_default_empty test_is_disabled -- --nocapture`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: All tests pass (existing tests should be unaffected since `disabled` defaults to empty vec)

- [ ] **Step 6: Commit**

```bash
git add src/config.rs
git commit -m "feat: add disabled field to CentralConfig for global feature toggle"
```

---

### Task 2: Add `CentralConfig::feature_name` helper

**Files:**
- Modify: `src/config.rs:29-37` (impl CentralConfig)

This helper maps `CentralField` to the feature name string used in `disabled`. Since `CentralField` lives in `tui::tool`, we implement this as a standalone function in config to avoid circular deps.

- [ ] **Step 1: Write failing test**

Add to `src/config.rs` test module:

```rust
#[test]
fn test_feature_name_for_field() {
    assert_eq!(CentralConfig::feature_name_str("prompt"), true);
    assert_eq!(CentralConfig::feature_name_str("skills"), true);
    assert_eq!(CentralConfig::feature_name_str("agents"), true);
    assert_eq!(CentralConfig::feature_name_str("commands"), true);
    assert_eq!(CentralConfig::feature_name_str("config"), false);
    assert_eq!(CentralConfig::feature_name_str("source"), false);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_feature_name_for_field`
Expected: FAIL

- [ ] **Step 3: Implement `is_toggleable_feature`**

Add to `impl CentralConfig` in `src/config.rs`:

```rust
pub const TOGGLEABLE_FEATURES: &'static [&'static str] = &["prompt", "skills", "agents", "commands"];

pub fn feature_name_str(name: &str) -> bool {
    Self::TOGGLEABLE_FEATURES.contains(&name)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_feature_name_for_field`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat: add TOGGLEABLE_FEATURES constant and feature_name_str helper"
```

---

### Task 3: Reorder central items in TUI tool view

**Files:**
- Modify: `src/tui/tool.rs:109-115` (build_rows central items)
- Modify: `src/tui/tool.rs:2276-2299` (test assertions for row ordering)

- [ ] **Step 1: Update `build_rows` ordering**

In `src/tui/tool.rs`, change lines 109-115 from:

```rust
        rows.push(ToolRow::CentralItem(CentralField::Config));
        rows.push(ToolRow::CentralItem(CentralField::Prompt));
        rows.push(ToolRow::CentralItem(CentralField::Skills));
        rows.push(ToolRow::CentralItem(CentralField::Agents));
        rows.push(ToolRow::CentralItem(CentralField::Commands));
        rows.push(ToolRow::CentralItem(CentralField::Source));
```

to:

```rust
        rows.push(ToolRow::CentralItem(CentralField::Config));
        rows.push(ToolRow::CentralItem(CentralField::Source));
        rows.push(ToolRow::CentralItem(CentralField::Prompt));
        rows.push(ToolRow::CentralItem(CentralField::Skills));
        rows.push(ToolRow::CentralItem(CentralField::Agents));
        rows.push(ToolRow::CentralItem(CentralField::Commands));
```

- [ ] **Step 2: Update test assertions for new ordering**

In `test_build_rows_central_expanded` (around line 2276), update the assertions:

```rust
assert!(matches!(rows[1], ToolRow::CentralItem(CentralField::Config)));
assert!(matches!(rows[2], ToolRow::CentralItem(CentralField::Source)));
assert!(matches!(rows[3], ToolRow::CentralItem(CentralField::Prompt)));
assert!(matches!(rows[4], ToolRow::CentralItem(CentralField::Skills)));
assert!(matches!(rows[5], ToolRow::CentralItem(CentralField::Agents)));
assert!(matches!(rows[6], ToolRow::CentralItem(CentralField::Commands)));
```

- [ ] **Step 3: Run tests**

Run: `cargo test test_build_rows`
Expected: All `test_build_rows_*` tests pass

- [ ] **Step 4: Commit**

```bash
git add src/tui/tool.rs
git commit -m "feat: reorder central items — config, source, then AI features"
```

---

### Task 4: Add `ConfirmToggleFeature` popup variant

**Files:**
- Modify: `src/tui/tool.rs:240-255` (PopupState enum)
- Modify: `src/tui/tool.rs:504-508` (popup type detection in handle_popup_key)
- Modify: `src/tui/tool.rs:558-579` (add handling after ConfirmCreate)

- [ ] **Step 1: Add PopupState variant**

In `src/tui/tool.rs`, add a new variant to `PopupState` (after `ConfirmCreate`):

```rust
ConfirmToggleFeature {
    feature: String,
    enabling: bool,
    tool_count: usize,
},
```

- [ ] **Step 2: Add popup type detection**

In `handle_popup_key` (around line 508), add after the `is_confirm` line:

```rust
let is_confirm_toggle = matches!(&self.popup, Some(PopupState::ConfirmToggleFeature { .. }));
```

- [ ] **Step 3: Add key handling for the confirmation popup**

After the `} else if is_confirm {` block (line 579), add:

```rust
} else if is_confirm_toggle {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(PopupState::ConfirmToggleFeature { feature, enabling, .. }) = self.popup.take() {
                self.execute_toggle_feature(&feature, enabling);
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            self.popup = None;
        }
        _ => {}
    }
}
```

- [ ] **Step 4: Add stub `execute_toggle_feature` method**

Add a placeholder method to `impl ToolApp` (will be implemented in Task 5):

```rust
fn execute_toggle_feature(&mut self, feature: &str, enabling: bool) {
    // TODO: implement in Task 5
    self.set_status(format!("{} {} — not yet implemented", if enabling { "Enable" } else { "Disable" }, feature));
}
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build`
Expected: Compiles without errors

- [ ] **Step 6: Commit**

```bash
git add src/tui/tool.rs
git commit -m "feat: add ConfirmToggleFeature popup variant with y/n handling"
```

---

### Task 5: Implement `execute_toggle_feature` — the core toggle logic

**Files:**
- Modify: `src/tui/tool.rs` (replace stub from Task 4)

- [ ] **Step 1: Implement `execute_toggle_feature`**

Replace the stub method with the full implementation:

```rust
fn execute_toggle_feature(&mut self, feature: &str, enabling: bool) {
    use super::log::LogLevel;

    let link_field = match feature {
        "prompt" => LinkField::Prompt,
        "skills" => LinkField::Skills,
        "agents" => LinkField::Agents,
        "commands" => LinkField::Commands,
        _ => return,
    };

    let tool_keys: Vec<String> = self
        .config
        .tools
        .iter()
        .filter(|(_, tc)| tc.is_installed())
        .map(|(k, _)| k.clone())
        .collect();

    let mut success_count = 0;
    let mut fail_count = 0;

    if enabling {
        // Remove from disabled list first
        self.config.central.disabled.retain(|d| d != feature);

        // Link for all installed tools
        for key in &tool_keys {
            if let Some((link_path, target, is_dir, _)) = self.get_link_paths(key, &link_field) {
                let status = linker::check_link(&link_path, &target, is_dir);
                match status {
                    LinkStatus::Linked => {
                        success_count += 1; // already linked
                    }
                    LinkStatus::Missing | LinkStatus::Broken => {
                        match linker::create_link_quiet(&link_path, &target, feature, is_dir) {
                            Ok((true, msg)) => {
                                self.log.push(LogLevel::Success, format!("[{}] {}", key, msg));
                                success_count += 1;
                            }
                            Ok((false, msg)) => {
                                self.log.push(LogLevel::Info, format!("[{}] {}", key, msg));
                                success_count += 1;
                            }
                            Err(e) => {
                                self.log.push(LogLevel::Error, format!("[{}] Link {} failed: {}", key, feature, e));
                                fail_count += 1;
                            }
                        }
                    }
                    LinkStatus::Blocked => {
                        self.handle_blocked_link(key, &link_field, &link_path, &target, is_dir, feature);
                        success_count += 1;
                    }
                    LinkStatus::Wrong(_) => {
                        if let Err(e) = crate::platform::remove_link(&link_path) {
                            self.log.push(LogLevel::Error, format!("[{}] Failed to remove wrong link: {}", key, e));
                            fail_count += 1;
                            continue;
                        }
                        match linker::create_link_quiet(&link_path, &target, feature, is_dir) {
                            Ok((true, msg)) => {
                                self.log.push(LogLevel::Success, format!("[{}] Repaired: {}", key, msg));
                                success_count += 1;
                            }
                            Ok((false, msg)) => {
                                self.log.push(LogLevel::Info, format!("[{}] {}", key, msg));
                                success_count += 1;
                            }
                            Err(e) => {
                                self.log.push(LogLevel::Error, format!("[{}] Repair failed: {}", key, e));
                                fail_count += 1;
                            }
                        }
                    }
                }
            }
        }
    } else {
        // Unlink for all installed tools first
        for key in &tool_keys {
            if let Some((link_path, target, is_dir, _)) = self.get_link_paths(key, &link_field) {
                let status = linker::check_link(&link_path, &target, is_dir);
                match status {
                    LinkStatus::Linked => {
                        match linker::remove_link_quiet(&link_path, feature, is_dir) {
                            Ok((true, msg)) => {
                                self.log.push(LogLevel::Success, format!("[{}] {}", key, msg));
                                self.recover_after_unlink(key, &link_field, &link_path);
                                success_count += 1;
                            }
                            Ok((false, msg)) => {
                                self.log.push(LogLevel::Info, format!("[{}] {}", key, msg));
                                success_count += 1;
                            }
                            Err(e) => {
                                self.log.push(LogLevel::Error, format!("[{}] Unlink {} failed: {}", key, feature, e));
                                fail_count += 1;
                            }
                        }
                    }
                    _ => {
                        success_count += 1; // already not linked
                    }
                }
            }
        }

        // Add to disabled list
        if !self.config.central.disabled.contains(&feature.to_string()) {
            self.config.central.disabled.push(feature.to_string());
        }
    }

    // Save config
    let config_path = self.config_path.clone().unwrap_or_else(Config::config_path);
    if let Err(e) = self.config.save_to(&config_path) {
        self.log.push(super::log::LogLevel::Error, format!("Failed to save config: {}", e));
    }

    let action = if enabling { "Enabled" } else { "Disabled" };
    if fail_count == 0 {
        self.set_status(format!("✓ {} {} for {} tool(s)", action, feature, success_count));
    } else {
        self.set_status(format!("⚠ {} {}: {} ok, {} failed", action, feature, success_count, fail_count));
    }

    self.rebuild_rows();
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add src/tui/tool.rs
git commit -m "feat: implement execute_toggle_feature with batch link/unlink"
```

---

### Task 6: Wire `i` key to trigger confirmation popup on central AI features

**Files:**
- Modify: `src/tui/tool.rs:461-474` (KeyCode::Char('i') handler)

- [ ] **Step 1: Add central feature toggle to `i` key handler**

Change the `KeyCode::Char('i')` match arm (lines 462-474) from:

```rust
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
```

to:

```rust
            KeyCode::Char('i') => {
                if let Some(row) = self.current_row().cloned() {
                    match &row {
                        ToolRow::CentralItem(ref cf @ (CentralField::Prompt | CentralField::Skills | CentralField::Agents | CentralField::Commands)) => {
                            self.show_toggle_feature_confirm(cf);
                        }
                        ToolRow::StatusHeader { tool_key } => {
                            self.toggle_all_links(&tool_key.clone());
                        }
                        ToolRow::LinkItem { tool_key, field } => {
                            let feature = match field {
                                LinkField::Prompt => "prompt",
                                LinkField::Skills => "skills",
                                LinkField::Agents => "agents",
                                LinkField::Commands => "commands",
                            };
                            if self.config.central.is_disabled(feature) {
                                self.set_status(format!("{} is globally disabled", feature));
                            } else {
                                self.toggle_link(&tool_key.clone(), &field.clone());
                            }
                        }
                        _ => {}
                    }
                }
            }
```

- [ ] **Step 2: Implement `show_toggle_feature_confirm`**

Add this method to `impl ToolApp`:

```rust
fn show_toggle_feature_confirm(&mut self, field: &CentralField) {
    let feature = match field {
        CentralField::Prompt => "prompt",
        CentralField::Skills => "skills",
        CentralField::Agents => "agents",
        CentralField::Commands => "commands",
        _ => return,
    };

    let enabling = self.config.central.is_disabled(feature);
    let tool_count = self.config.tools.values().filter(|t| t.is_installed()).count();

    self.popup = Some(PopupState::ConfirmToggleFeature {
        feature: feature.to_string(),
        enabling,
        tool_count,
    });
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add src/tui/tool.rs
git commit -m "feat: wire i key on central AI features to confirmation popup"
```

---

### Task 7: Render the `ConfirmToggleFeature` popup

**Files:**
- Modify: `src/tui/tool.rs` (rendering section, near `render_confirm_create`)

- [ ] **Step 1: Find `render_confirm_create` and add parallel renderer**

Find `fn render_confirm_create` (around line 2116) and add a new function after it:

```rust
fn render_confirm_toggle_feature(app: &ToolApp, frame: &mut Frame, area: Rect) {
    if let Some(PopupState::ConfirmToggleFeature { ref feature, enabling, tool_count }) = app.popup {
        let popup_area = super::dialog_area(area, 3);
        frame.render_widget(Clear, popup_area);

        let action = if *enabling { "Enable" } else { "Disable" };
        let title = format!(" {} Feature ", action);
        let block = Block::default()
            .title(title.as_str())
            .title_bottom(" y:confirm  n/Esc:cancel ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if *enabling { Color::Green } else { Color::Red }));
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let text = format!(
            "{} {} for {} installed tool(s)?",
            action, feature, tool_count
        );
        let paragraph = Paragraph::new(text).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner);
    }
}
```

- [ ] **Step 2: Call the renderer from the main draw function**

Find where `render_confirm_create` is called in the main `draw` / `ui` function and add a call to `render_confirm_toggle_feature` right after it. Search for:

```rust
render_confirm_create(&app, frame, area);
```

Add after it:

```rust
render_confirm_toggle_feature(&app, frame, area);
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add src/tui/tool.rs
git commit -m "feat: render ConfirmToggleFeature popup with y/n dialog"
```

---

### Task 8: Gray out disabled central items in tool view rendering

**Files:**
- Modify: `src/tui/tool.rs:1830-1870` (render_row for CentralItem)

- [ ] **Step 1: Update CentralItem rendering to show enable/disable indicators**

Replace the `ToolRow::CentralItem(field)` match arm in `render_row` (lines 1830-1870). The key change: for AI features (prompt/skills/agents/commands), add a status indicator and gray out if disabled.

```rust
ToolRow::CentralItem(field) => {
    let (label, value) = match field {
        CentralField::Config => (
            "config".to_string(),
            "~/.config/agm/config.toml".to_string(),
        ),
        CentralField::Source => (
            "source".to_string(),
            contract_tilde(&expand_tilde(&config.central.source_dir)),
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
    };

    let is_feature = matches!(
        field,
        CentralField::Prompt | CentralField::Skills | CentralField::Agents | CentralField::Commands
    );
    let is_disabled = is_feature && config.central.is_disabled(&label);

    let (indicator, indicator_style) = if !is_feature {
        ("  ".to_string(), Style::default())
    } else if is_disabled {
        ("✗ ".to_string(), Style::default().fg(Color::Red))
    } else {
        ("✓ ".to_string(), Style::default().fg(Color::Green))
    };

    let label_style = if is_disabled {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let value_style = if is_cursor {
        Style::default().fg(Color::Yellow)
    } else if is_disabled {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };

    Line::from(vec![
        Span::raw(format!("{}    ", cursor_prefix)),
        Span::styled(indicator, indicator_style),
        Span::styled(format!("{:<8}", label), label_style),
        Span::styled(value, value_style),
    ])
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add src/tui/tool.rs
git commit -m "feat: render enable/disable indicators on central AI features"
```

---

### Task 9: Gray out disabled LinkItems in tool view rendering

**Files:**
- Modify: `src/tui/tool.rs:1919-1962` (render_row for LinkItem)

- [ ] **Step 1: Update LinkItem rendering to check disabled state**

In the `ToolRow::LinkItem` match arm, after getting the label (around line 1946), add a disabled check and override the rendering:

After line `let status = linker::check_link(&link_path, &target, is_dir);` (line 1947), add a disabled check:

```rust
let feature_disabled = config.central.is_disabled(label);
```

Then replace the status_spans and rendering logic. Change lines 1948-1961 to:

```rust
let feature_disabled = config.central.is_disabled(label);

if feature_disabled {
    let mut spans = vec![
        Span::raw(format!("{}      ", cursor_prefix)),
        Span::styled(
            format!("{:<8} ", label),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled("disabled", Style::default().fg(Color::DarkGray)),
    ];
    if is_cursor {
        Line::from(spans).style(Style::default().fg(Color::Yellow))
    } else {
        Line::from(spans)
    }
} else {
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
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add src/tui/tool.rs
git commit -m "feat: gray out disabled features in per-tool LinkItem rendering"
```

---

### Task 10: Update `toggle_all_links` to skip disabled features

**Files:**
- Modify: `src/tui/tool.rs:1059-1109` (toggle_all_links method)

- [ ] **Step 1: Add disabled feature filtering**

In `toggle_all_links`, change the `fields` array and iteration to skip disabled features. Replace lines 1069-1108:

```rust
    let all_fields = [LinkField::Prompt, LinkField::Skills, LinkField::Agents, LinkField::Commands];
    let fields: Vec<&LinkField> = all_fields
        .iter()
        .filter(|f| {
            let name = match f {
                LinkField::Prompt => "prompt",
                LinkField::Skills => "skills",
                LinkField::Agents => "agents",
                LinkField::Commands => "commands",
            };
            !self.config.central.is_disabled(name)
        })
        .collect();

    if fields.is_empty() {
        self.set_status("All features are disabled".to_string());
        return;
    }

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
    if linked_count == fields.len() {
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
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add src/tui/tool.rs
git commit -m "feat: toggle_all_links skips globally disabled features"
```

---

### Task 11: Update footer hints for central AI feature items

**Files:**
- Modify: `src/tui/tool.rs:1680-1691` (footer hints for CentralItem)

- [ ] **Step 1: Split CentralItem hint rendering**

Replace the hint section for CentralItem (lines 1680-1691). Currently:

```rust
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
```

Change to:

```rust
Some(ToolRow::CentralItem(CentralField::Config)) => {
    spans.extend([hint_key("␣/⏎"), hint_text(" info  ")]);
    spans.extend([hint_key("e"), hint_text(" edit  ")]);
    spans.extend([hint_key("l"), hint_text(" log  ")]);
    spans.extend([hint_key("q"), hint_text(" quit")]);
}
Some(ToolRow::CentralItem(CentralField::Source)) => {
    spans.extend([hint_key("␣/⏎"), hint_text(" edit path  ")]);
    spans.extend([hint_key("l"), hint_text(" log  ")]);
    spans.extend([hint_key("q"), hint_text(" quit")]);
}
Some(ToolRow::CentralItem(CentralField::Prompt)) => {
    spans.extend([hint_key("␣/⏎"), hint_text(" info  ")]);
    spans.extend([hint_key("e"), hint_text(" edit  ")]);
    spans.extend([hint_key("i"), hint_text(" toggle  ")]);
    spans.extend([hint_key("l"), hint_text(" log  ")]);
    spans.extend([hint_key("q"), hint_text(" quit")]);
}
Some(ToolRow::CentralItem(_)) => {
    // Skills, Agents, Commands
    spans.extend([hint_key("␣/⏎"), hint_text(" edit path  ")]);
    spans.extend([hint_key("i"), hint_text(" toggle  ")]);
    spans.extend([hint_key("l"), hint_text(" log  ")]);
    spans.extend([hint_key("q"), hint_text(" quit")]);
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add src/tui/tool.rs
git commit -m "feat: add i:toggle hint to central AI feature footer"
```

---

### Task 12: Block Space/Enter path editing on disabled features

**Files:**
- Modify: `src/tui/tool.rs:412-417` (Space/Enter handler for CentralItem Skills/Agents/Commands/Source)

- [ ] **Step 1: Guard Space/Enter on disabled features**

In the `KeyCode::Char(' ') | KeyCode::Enter` handler, the match arm for `CentralField::Skills | Agents | Commands | Source` (around line 412-431) opens a PathEditor. Add a disabled guard for the AI features:

```rust
ToolRow::CentralItem(
    ref cf @ (CentralField::Skills
    | CentralField::Agents
    | CentralField::Commands
    | CentralField::Source),
) => {
    // Block path editing for disabled features
    let feature_name = match cf {
        CentralField::Skills => Some("skills"),
        CentralField::Agents => Some("agents"),
        CentralField::Commands => Some("commands"),
        _ => None,
    };
    if let Some(name) = feature_name {
        if self.config.central.is_disabled(name) {
            self.set_status(format!("{} is disabled — press i to enable", name));
            return;
        }
    }

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
```

- [ ] **Step 2: Also guard Prompt info when disabled**

For the Prompt item (around line 435), add a similar guard:

```rust
ToolRow::CentralItem(CentralField::Prompt) => {
    if self.config.central.is_disabled("prompt") {
        self.set_status("prompt is disabled — press i to enable".to_string());
    } else {
        self.show_central_info(&CentralField::Prompt);
    }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add src/tui/tool.rs
git commit -m "feat: block Space/Enter on disabled central features"
```

---

### Task 13: Gray out disabled categories in TUI source view

**Files:**
- Modify: `src/tui/source.rs:1677-1732` (CategoryHeader rendering)
- Modify: `src/tui/source.rs:1298-1327` (i key handler)

- [ ] **Step 1: Update CategoryHeader rendering to check disabled**

In the `ListRow::CategoryHeader` render arm (line 1677), after computing `label` and `expanded` (around line 1718), add disabled check:

```rust
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
```

- [ ] **Step 2: Gray out individual items under disabled categories**

Find the rendering for `ListRow::SkillItem`, `ListRow::AgentItem`, and `ListRow::CommandItem`. Each needs a disabled check. For example, in the SkillItem render arm, after building the `spans`, add:

```rust
let disabled = app.config.central.is_disabled("skills");
// ... if disabled, override style to DarkGray
```

Apply the same pattern for AgentItem (check "agents") and CommandItem (check "commands").

- [ ] **Step 3: Guard `i` key in source view for disabled categories**

In the `KeyCode::Char('i')` handler (line 1298), wrap each toggle call with a disabled check:

```rust
KeyCode::Char('i') => {
    let row = self.current_row().cloned();
    match row {
        Some(ListRow::SkillItem { group_index, skill_index }) => {
            if self.config.central.is_disabled("skills") {
                self.set_status("Skills feature is disabled");
            } else {
                self.toggle_skill(group_index, skill_index);
            }
        }
        Some(ListRow::AgentItem { group_index, agent_index }) => {
            if self.config.central.is_disabled("agents") {
                self.set_status("Agents feature is disabled");
            } else {
                self.toggle_agent(group_index, agent_index);
            }
        }
        Some(ListRow::CommandItem { group_index, command_index }) => {
            if self.config.central.is_disabled("commands") {
                self.set_status("Commands feature is disabled");
            } else {
                self.toggle_command(group_index, command_index);
            }
        }
        Some(ListRow::SourceHeader { group_index, category }) => {
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
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 5: Commit**

```bash
git add src/tui/source.rs
git commit -m "feat: gray out disabled categories in source view with i key guard"
```

---

### Task 14: Add CLI guards for `link_all` and `unlink_all`

**Files:**
- Modify: `src/main.rs:102-361` (link_all function)
- Modify: `src/main.rs:363-413` (unlink_all function)

- [ ] **Step 1: Add disabled check to link_all**

At the start of `link_all`, after the central path expansions (line 106), add:

```rust
let disabled = &config.central.disabled;
```

Then wrap each linking section with a disabled check. Before the skills linking block (around line 179 `if !tool.skills_dir.is_empty()`):

```rust
if !tool.skills_dir.is_empty() && !disabled.iter().any(|d| d == "skills") {
```

Before the agents block (around line 242 `if !tool.agents_dir.is_empty()`):

```rust
if !tool.agents_dir.is_empty() && !disabled.iter().any(|d| d == "agents") {
```

Before the prompt block (around line 296 `if !tool.prompt_filename.is_empty()`):

```rust
if !tool.prompt_filename.is_empty() && !disabled.iter().any(|d| d == "prompt") {
```

Add a disabled message for each skipped feature after the tool header print:

```rust
for d in disabled {
    println!("  {} {} (disabled)", "skip".yellow(), d);
}
```

- [ ] **Step 2: Add disabled check to unlink_all**

In `unlink_all`, wrap each unlink section similarly:

```rust
if !tool_config.skills_dir.is_empty() && !config.central.disabled.iter().any(|d| d == "skills") {
```

```rust
if !tool_config.agents_dir.is_empty() && !config.central.disabled.iter().any(|d| d == "agents") {
```

```rust
if !tool_config.prompt_filename.is_empty() && !config.central.disabled.iter().any(|d| d == "prompt") {
```

Note: `unlink_all` does not currently handle `commands`. If it should, add it; otherwise just guard the existing 3 features.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: CLI link_all and unlink_all skip disabled features"
```

---

### Task 15: Add disabled indicator to CLI status display

**Files:**
- Modify: `src/status.rs:9-213`

- [ ] **Step 1: Add disabled check to status output**

In `src/status.rs`, after loading config (line 10), add:

```rust
let disabled = &config.central.disabled;
```

For each feature's status display block, add a disabled check. For example, before the prompt status block (line 65):

```rust
if let Some(ls) = prompt_ls {
    if disabled.iter().any(|d| d == "prompt") {
        println!("{}{:<8}{}", INDENT, "prompt", "disabled".dimmed());
    } else {
        // existing prompt status rendering
    }
}
```

Apply the same pattern for skills (line 90), agents (line 115), and commands (line 140).

Also update the central summary section at the bottom (lines 191-209) to show disabled status:

```rust
if disabled.iter().any(|d| d == "prompt") {
    println!("Central prompt : {} (disabled)", contract_tilde(&central_prompt).dimmed());
} else {
    println!("Central prompt : {}", contract_tilde(&central_prompt));
}
```

Apply similar patterns for skills, agents, commands.

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles

- [ ] **Step 3: Commit**

```bash
git add src/status.rs
git commit -m "feat: show disabled status in CLI status display"
```

---

### Task 16: Final integration test and cleanup

**Files:**
- All modified files

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 2: Run clippy for lint checks**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Build release**

Run: `cargo build --release`
Expected: Builds successfully

- [ ] **Step 4: Final commit if any remaining changes**

```bash
git add -A
git commit -m "chore: cleanup and final adjustments for central feature toggle"
```
