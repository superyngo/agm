# Prompt Blocked Handling & Display Fixes

**Date**: 2026-03-20
**Status**: Draft

## Problem Statement

Three related issues in AGM's prompt linking and status display:

1. **Empty prompt files cause silent skip**: When a tool's prompt file exists but is empty, `main.rs` pre-processing falls through without removing it, causing `create_link()` to hit the `Blocked` path and print `"warn prompt exists but is not a link, skipping"` instead of creating the link.

2. **Blocked status lacks path info**: The `agm status` display shows `✗ blocked` with no path, making it unclear *which file* is blocking. Other statuses (linked, missing, wrong) all show a path arrow.

3. **Mixed path separators on Windows**: `contract_tilde()` hardcodes `~/` as the prefix, but on Windows `rest.display()` uses `\`, producing mixed output like `~/.claude\skills` instead of `~\.claude\skills`.

## Changes

### 1. Fix empty-file gap in `main.rs` (lines 550–573)

**Current behavior**: When the prompt file is a regular file (not a link) and its content is empty, the code falls through the `if !content.trim().is_empty()` block without removing the file. `create_link()` then sees the file and returns `Blocked`.

**Fix**: Add an `else` branch that removes the empty file:

```rust
// Current (around line 552)
if !content.trim().is_empty() {
    if yes || prompt_yes_no(...) {
        // backup and rename
    } else {
        continue;
    }
}
// Falls through — file still exists!

// Fixed
if !content.trim().is_empty() {
    if yes || prompt_yes_no(...) {
        // backup and rename
    } else {
        continue;
    }
} else {
    // Empty file — safe to remove without backup
    fs::remove_file(&prompt_link)?;
}
```

**Files changed**: `src/main.rs`

### 2. Show path in Blocked status display (`status.rs` lines 69, 90)

**Current behavior**:
```
prompt   ✗ blocked
skills   ✗ blocked
```

**New behavior** (paths shown with platform-native separators after Issue 3 fix):
```
prompt   ✗ blocked → ~\.claude\CLAUDE.md      (Windows)
prompt   ✗ blocked → ~/.claude/CLAUDE.md       (Linux)
skills   ✗ blocked → ~\.claude\skills          (Windows)
skills   ✗ blocked → ~/.claude/skills          (Linux)
```

The path shown is the existing file/directory that is blocking link creation (`prompt_link` for prompts, `skills_link` for skills).

**Files changed**: `src/status.rs`

### 3. Platform-native path separators in `contract_tilde()` (`paths.rs` line 77)

**Current behavior**: Always uses `~/` prefix regardless of platform. On Windows produces `~/.claude\skills` (mixed).

**New behavior**:
- Use `std::path::MAIN_SEPARATOR` for the `~` prefix separator
- On Windows, normalize any remaining `/` to `\` in the output string
- Linux: `~/.claude/skills` (unchanged)
- Windows: `~\.claude\skills` (all backslashes)

```rust
// Current
return format!("~/{}", rest.display());

// Fixed
let display = format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display());
#[cfg(windows)]
let display = display.replace('/', "\\");
return display;
```

**Test update**: The `test_contract_tilde` test must become platform-aware since the expected output differs by OS.

**Files changed**: `src/paths.rs`

## Scope

- No changes to `linker::create_link()` contract — it stays non-interactive.
- No changes to `expand_tilde()` input parsing — config files continue to use `~/` universally.
- No changes to skills linking flow (already handles empty directories correctly).

## Testing

- Existing tests in `linker.rs` and `paths.rs` continue to pass (with platform-aware updates where needed).
- Manual verification of `agm status` and `agm link` for each tool with blocked prompts.
