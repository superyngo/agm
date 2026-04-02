# Central Feature Toggle — Design Spec

## Problem

AGM manages 4 AI features (prompt, skills, agents, commands) that are linked from a central store to each tool's config directory. Currently there is no way to globally disable a feature across all tools. Users may want to disable a feature (e.g., skills) without removing its configuration, and re-enable it later with the same state restored.

## Approach

Add a `disabled` field to `CentralConfig` that lists globally disabled features. The TUI provides an `i` key toggle on central feature items, with a confirmation popup. Disabled features are grayed out everywhere—central items, per-tool link items, and source view categories.

## Config Changes

### CentralConfig struct

Add `disabled: Vec<String>` with `#[serde(default)]` for backward compatibility.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CentralConfig {
    pub prompt_source: String,
    pub skills_source: String,
    #[serde(default = "CentralConfig::default_agents_source")]
    pub agents_source: String,
    #[serde(default = "CentralConfig::default_commands_source")]
    pub commands_source: String,
    pub source_dir: String,
    #[serde(default)]
    pub source_repos: Vec<String>,
    #[serde(default)]
    pub disabled: Vec<String>,  // Valid values: "prompt", "skills", "agents", "commands"
}
```

### Helper methods

```rust
impl CentralConfig {
    pub fn is_disabled(&self, feature: &str) -> bool {
        self.disabled.iter().any(|d| d == feature)
    }

    pub fn feature_name(field: &CentralField) -> Option<&'static str> {
        match field {
            CentralField::Prompt => Some("prompt"),
            CentralField::Skills => Some("skills"),
            CentralField::Agents => Some("agents"),
            CentralField::Commands => Some("commands"),
            _ => None, // Config and Source are not toggleable
        }
    }
}
```

### Example config.toml

```toml
[central]
prompt_source = "~/.local/share/agm/prompts/MASTER.md"
skills_source = "~/.local/share/agm/skills"
agents_source = "~/.local/share/agm/agents"
commands_source = "~/.local/share/agm/commands"
source_dir = "~/.local/share/agm/source"
disabled = ["skills"]
```

## TUI Tool View Changes

### Central item ordering

Reorder from: config → prompt → skills → agents → commands → source

To: **config → source → prompt → skills → agents → commands**

This groups meta items (config, source) at top and toggleable AI features at bottom.

### Central item rendering

Each AI feature item (prompt/skills/agents/commands) shows an enable/disable indicator:

- **Enabled**: white text + green `✓` icon
- **Disabled**: DarkGray text + red `✗` icon  
- **Cursor on item**: Yellow text (with icon preserved)

Config and Source items render as before (no icon, not toggleable).

### `i` key on central AI feature items

When the user presses `i` on a central prompt/skills/agents/commands item:

1. **Determine action**: if currently enabled → disable; if disabled → enable
2. **Count affected tools**: count installed tools (`is_installed() == true`)
3. **Show confirmation popup**:
   - Disable: `"Disable {feature} for {N} installed tools? (y/n)"`
   - Enable: `"Enable {feature} for {N} installed tools? (y/n)"`
4. **On confirm (y)**:
   - **Disable flow**:
     1. For each installed tool: unlink the feature via `remove_link_quiet()` + `recover_after_unlink()`
     2. Add feature name to `config.central.disabled`
     3. Save config
     4. Log results, show status message
   - **Enable flow**:
     1. Remove feature name from `config.central.disabled`
     2. For each installed tool: link the feature via `create_link_quiet()` (with blocked/migration handling)
     3. Save config
     4. Log results, show status message
5. **Rebuild rows** to update visual state

### `i` key on non-toggleable central items

Config and Source items: `i` key does nothing (no-op).

### Per-tool LinkItem rendering when feature disabled

When a feature is globally disabled:

- **Rendering**: DarkGray text, status shows `disabled` instead of ✓/✗/⚠
- **`i` key**: no-op (cannot toggle individual tool link when globally disabled)
- **Space/Enter (info popup)**: still viewable, shows "Feature globally disabled in central config"

### Per-tool toggle_all_links behavior

`toggle_all_links()` (press `i` on StatusHeader) skips disabled features:

- Only considers enabled features when deciding toggle direction
- Only links/unlinks enabled features

## TUI Source View Changes

Source view categories (Skills / Agents / Commands) check `central.disabled`:

- **Disabled category**: header rendered in DarkGray with `(disabled)` suffix
- **Items under disabled category**: all rendered in DarkGray
- **`i` key (install/uninstall)**: no-op on disabled category items
- **Search results**: disabled items still appear but grayed out

Note: Source view has no "prompt" category (prompt is a single file, not managed via source view), so only skills/agents/commands are affected.

## CLI Operation Guards

All non-TUI CLI operations check `config.central.disabled`:

### `agm tool --link` (link_all)

- Skip disabled features when creating links
- Log: `"Skipping {feature} (disabled)"`

### `agm tool --unlink` (unlink_all)

- Skip disabled features (already unlinked when disabled)
- Log: `"Skipping {feature} (disabled)"`

### `agm tool --status` (status display)

- Show disabled features with `disabled` status indicator
- Example: `skills: disabled`

### `agm source --update`

- No change. Source repo updates are independent of feature enable/disable.
- Disabled features are still updated in the source store; they just aren't linked to tools.

## Scope

### In scope

- `CentralConfig.disabled` field with serde support
- `is_disabled()` helper method
- Central item reordering (config → source → prompt → skills → agents → commands)
- `i` key toggle on central AI feature items with confirmation popup
- Unlink-all / link-all flow for the toggled feature
- Grayed-out rendering in tool view (central items + per-tool link items)
- Grayed-out rendering in source view (category headers + items)
- CLI guard in link_all, unlink_all, status
- Unit tests for config serialization with disabled field
- Unit tests for is_disabled helper

### Out of scope

- Per-tool feature disable (only global central-level)
- Disabling source_dir or config (not AI features)
- Any breaking config format changes
