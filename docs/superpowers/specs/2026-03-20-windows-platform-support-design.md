# Windows Platform Support — Design Spec

## Problem Statement

AGM currently only supports Unix platforms (Linux, macOS). All symlink operations use `std::os::unix::fs`, all default paths assume Unix conventions, CI/CD only builds Linux/macOS binaries, and the installation script only supports bash. This spec defines the design for adding full Windows platform support.

## Scope

- Add Windows build targets: `x86_64-pc-windows-msvc` and `i686-pc-windows-msvc`
- Implement Windows-compatible link mechanism (junction for directories, hardlink for files)
- Adapt path handling for Windows
- Update CI/CD pipeline for Windows builds
- Create PowerShell installation script
- Update documentation and tests

## Approach

**Platform Abstraction Layer** — introduce a new `src/platform.rs` module that encapsulates all platform-specific behavior behind a unified API. Existing modules (`linker.rs`, `skills.rs`, `files.rs`, `main.rs`, `editor.rs`) will call `platform::*` functions instead of using `std::os::unix::fs` directly. Conditional compilation (`#[cfg(unix)]` / `#[cfg(windows)]`) is isolated inside `platform.rs`.

---

## 1. Platform Abstraction Layer (`platform.rs`)

### 1.1 Link Operations

```rust
/// Create a directory link.
/// - Unix: symlink (target ← link_path)
/// - Windows: NTFS junction (target ← link_path)
pub fn link_dir(target: &Path, link_path: &Path) -> io::Result<()>

/// Create a file link.
/// - Unix: symlink (target ← link_path)
/// - Windows: NTFS hardlink (src → dst; src MUST exist)
pub fn link_file(target: &Path, link_path: &Path) -> io::Result<()>

/// Remove a link (file or directory).
/// - Unix: fs::remove_file (works for symlinks to both files and dirs)
/// - Windows: fs::remove_file for hardlinks, fs::remove_dir for junctions
pub fn remove_link(link_path: &Path) -> io::Result<()>

/// Check if a path is a link (symlink, junction, or hardlink).
/// - Unix: metadata.file_type().is_symlink()
/// - Windows: junction → reparse point check; hardlink → nlink > 1
/// NOTE: nlink > 1 is a heuristic — it cannot prove AGM ownership.
/// Callers should pair with same_file() for authoritative verification.
pub fn is_link(path: &Path) -> bool

/// Read the target of a directory link. Returns None for hardlinks (no direction).
/// - Unix: fs::read_link()
/// - Windows: fs::read_link() for junctions (returns \\?\... path), None for hardlinks
pub fn read_link_target(path: &Path) -> Option<PathBuf>

/// Check if two paths point to the same underlying file (by file index / inode).
/// Used for hardlink equivalence checking on Windows.
pub fn same_file(a: &Path, b: &Path) -> io::Result<bool>
```

### 1.2 Platform Defaults

```rust
/// Default editor when config.editor is empty and $EDITOR is unset.
/// - Unix: "vi"
/// - Windows: "notepad"
pub fn default_editor() -> &'static str
```

### 1.3 Capability Detection

```rust
pub enum LinkCapability {
    /// Full junction + hardlink support
    Full,
    /// Partial support with explanation
    Limited(String),
    /// Cannot create links
    Unavailable(String),
}

/// Probe the system's ability to create junctions and hardlinks.
/// Called during `agm init` to inform the user.
pub fn check_link_capability() -> LinkCapability
```

### 1.4 Windows Implementation Details

**Junctions (directories):**
- Use the `junction` crate (`junction::create(target, link_path)`)
- Junctions do NOT require elevated privileges
- Junctions do NOT work across volumes (same-drive constraint)
- `fs::read_link()` returns `\\?\C:\...` extended-length paths for junctions
- Deletion: `fs::remove_dir(junction_path)` removes the junction, not the target

**Hardlinks (files):**
- Use `std::fs::hard_link(src, dst)` (standard library, cross-platform)
- The source file MUST exist at creation time (unlike symlinks)
- No directional relationship — both paths are equal references to the same inode
- Detection: `std::os::windows::fs::MetadataExt::number_of_links() > 1`
- Equivalence check: compare `volume_serial_number` + `file_index` from `GetFileInformationByHandle`
- Same-volume constraint: hardlinks cannot cross drive letters
- Deletion: `fs::remove_file()` removes one link; file persists until last link is removed

**Same-drive constraint:**
- Both junction and hardlink require target and link on the same NTFS volume
- AGM will validate this during link operations and emit a clear error:
  `"Cannot create link across drives (central: C:, tool: D:). Ensure both are on the same drive."`

### 1.5 New Dependency

```toml
[target.'cfg(windows)'.dependencies]
junction = "1"
```

---

## 2. Existing Module Changes

### 2.1 `linker.rs`

- Remove `use std::os::unix::fs as unix_fs`
- `create_link(link_path, target, label)` → add `is_dir: bool` parameter
  - `is_dir == true`: call `platform::link_dir(target, link_path)`
  - `is_dir == false`: call `platform::link_file(target, link_path)`
- `check_link(link_path, expected_target)` → add `is_dir: bool` parameter
  - On Unix: unchanged (use `read_link` + `canonicalize`)
  - On Windows for junctions: use `platform::read_link_target()` + normalize `\\?\` prefix
  - On Windows for hardlinks: use `platform::same_file(link_path, expected_target)`
- `remove_link()` → call `platform::remove_link()`

### 2.2 `files.rs`

- Remove `use std::os::unix::fs as unix_fs`
- `link_file()`: replace `unix_fs::symlink(central, original)` with `platform::link_file(central, original)`
  - **Important**: on Windows, `hard_link(src, dst)` — `src` is the existing file (central), `dst` is the new link (original). Argument order differs from Unix `symlink(target, link_path)`.
- `check_file_status()`: replace `is_symlink()` check with `platform::is_link()` + `platform::same_file()`
- `centralized_path()`: update to handle Windows path prefixes (see §3)

### 2.3 `skills.rs`

- Remove `use std::os::unix::fs as unix_fs`
- `add_local()`: replace `unix_fs::symlink(&skill_path, &link_path)` with `platform::link_dir(&skill_path, &link_path)`
- `list_skills()`: replace `path.is_symlink()` with `platform::is_link()`
- `remove_skill()`: replace `fs::remove_file(&skill_path)` with `platform::remove_link(&skill_path)`
- `prune_broken_skills()`: adapt broken-link detection for junctions

### 2.4 `main.rs`

- `migrate_skills_dir()`: replace `unix_fs::symlink` with `platform::link_dir()`
- `copy_dir_all()`: replace `std::os::unix::fs::symlink` with `platform::link_dir()` / `platform::link_file()` based on source type

### 2.5 `editor.rs`

- `get_editor()`: change fallback from `"vi"` to `platform::default_editor()`

---

## 3. Path Handling Changes

### 3.1 `paths.rs` — `expand_tilde()`

**No changes needed.** `dirs::home_dir()` returns `C:\Users\<name>` on Windows. The `~/.config/agm` pattern expands correctly to `C:\Users\<name>\.config\agm` because `PathBuf::join()` handles mixed separators.

### 3.2 `config.rs` — `ToolConfig::resolve_path()`

**Current logic:** `path.contains('/')` → treat as absolute.

**New logic:**
```rust
fn is_absolute_or_rooted(path: &str) -> bool {
    path.contains('/')
        || path.contains('\\')
        || path.starts_with('~')
        || (path.len() >= 2 && path.as_bytes()[1] == b':')  // C:\...
}
```

If `is_absolute_or_rooted(path)`, call `expand_path(path)`. Otherwise, treat as relative to `config_dir`.

### 3.3 `files.rs` — `centralized_path()`

**Current logic:** `abs.strip_prefix("/")` to get relative path under `files_base`.

**New logic using `std::path::Component`:**
```rust
let components: PathBuf = abs.components()
    .filter(|c| !matches!(c, Component::Prefix(_) | Component::RootDir))
    .collect();
files_base.join(components)
```

This handles both `/Users/wen/.claude/settings.json` → `Users/wen/.claude/settings.json` and `C:\Users\wen\.claude\settings.json` → `Users\wen\.claude\settings.json`.

### 3.4 `paths.rs` — `contract_tilde()`

**No changes needed.** `Path::strip_prefix(&home)` works regardless of separator. Display format will use native separators (backslash on Windows), which is acceptable for a CLI tool.

### 3.5 Default Paths — No Changes to Values

All AI CLI tools use `~/.toolname` on Windows (verified by research):

| Tool     | config_dir (both platforms) |
|----------|---------------------------|
| Claude   | `~/.claude`               |
| Gemini   | `~/.gemini`               |
| Copilot  | `~/.copilot`              |
| OpenCode | `~/.config/opencode`      |

AGM central paths (both platforms):
- Config: `~/.config/agm/config.toml`
- Prompts: `~/.local/share/agm/prompts/MASTER.md`
- Skills: `~/.local/share/agm/skills/`
- Source: `~/.local/share/agm/source/`
- Files: `~/.local/share/agm/files/`

---

## 4. CI/CD Changes

### 4.1 `release.yml` — New Windows Matrix Entries

```yaml
# Windows builds
- os: windows-latest
  target: x86_64-pc-windows-msvc
  artifact_name: agm.exe
  asset_name: agm-windows-x86_64
- os: windows-latest
  target: i686-pc-windows-msvc
  artifact_name: agm.exe
  asset_name: agm-windows-i686
```

### 4.2 Windows Build Steps

- **Rust toolchain**: `dtolnay/rust-toolchain@stable` with `targets: ${{ matrix.target }}` — works on `windows-latest`
- **No cross/musl needed**: MSVC toolchain is natively available
- **i686 target**: add `i686-pc-windows-msvc` target via rustup (rustup handles MSVC lib paths)
- **No strip step**: MSVC release builds are already optimized; `strip` is not standard on Windows
- **Package as zip** instead of tar.gz:
  ```yaml
  - name: Create zip (Windows)
    if: runner.os == 'Windows'
    run: Compress-Archive -Path "target\${{ matrix.target }}\release\${{ matrix.artifact_name }}" -DestinationPath "${{ matrix.asset_name }}.zip"
    shell: pwsh
  ```
- **Checksum**: `Get-FileHash` in PowerShell or `sha256sum` via Git Bash

### 4.3 Release Artifact Naming

| Platform | Pattern |
|----------|---------|
| Linux    | `agm-linux-{arch}[-musl].tar.gz` |
| macOS    | `agm-macos-{arch}.tar.gz` |
| Windows  | `agm-windows-{arch}.zip` |

### 4.4 Test Step

Add `cargo test` to the Windows build matrix to catch platform-specific test failures.

---

## 5. Installation

### 5.1 PowerShell Install Script

Create a `gpinstall.ps1` script (or update the existing gist) that:
1. Detects architecture (`[System.Environment]::Is64BitOperatingSystem`)
2. Downloads the correct `.zip` from GitHub Releases
3. Extracts to `%USERPROFILE%\.local\bin\`
4. Adds `%USERPROFILE%\.local\bin` to user PATH if not present
5. Supports `Uninstall` parameter

Usage:
```powershell
irm https://gist.githubusercontent.com/.../gpinstall.ps1 | iex
```

### 5.2 README.md Updates

Add Windows installation section:
- PowerShell one-liner
- Manual download instructions
- Note about junction/hardlink behavior
- Supported platforms list update

---

## 6. Testing Strategy

### 6.1 Existing Test Adaptation

All tests in `linker.rs`, `skills.rs`, `files.rs` currently use `std::os::unix::fs::symlink`. Replace with `platform::link_dir()` / `platform::link_file()` so they run correctly on both platforms.

### 6.2 New `platform.rs` Unit Tests

```rust
#[test] fn test_link_dir_and_read_target()
#[test] fn test_link_file_and_same_file()
#[test] fn test_remove_link_dir()
#[test] fn test_remove_link_file()
#[test] fn test_is_link_false_for_regular_file()
#[test] fn test_default_editor()
#[test] fn test_check_link_capability()
```

### 6.3 Windows-Specific Tests

```rust
#[cfg(windows)]
#[test] fn test_junction_read_link_extended_path()  // \\?\ prefix handling
#[test] fn test_hardlink_same_file_check()
#[test] fn test_cross_drive_error_message()  // if testable in CI
```

### 6.4 CI

- The `windows-latest` runner in GitHub Actions supports junction and hardlink creation without elevation.
- Add `cargo test --target x86_64-pc-windows-msvc` to the release pipeline or a dedicated CI workflow.

---

## 7. Additional Considerations

### 7.1 Windows Hardlink Semantics

Unlike Unix symlinks which have a clear source→target direction, Windows hardlinks are **bidirectional** — both paths are equal references to the same file. This affects:

- **`status` display**: cannot show `→ target` for hardlinks. Instead show `↔ central` or verify equivalence and show `✓ linked`.
- **`check_link()`**: must use file index comparison (`same_file()`) rather than `read_link()`.
- **Broken link detection**: hardlinks cannot be "broken" (if one path exists, the file exists). A "missing" hardlink means the tool-side path doesn't exist.

### 7.2 Junction `read_link` Behavior

`fs::read_link()` on a Windows junction returns `\\?\C:\Users\...` (extended-length path). Comparison logic must normalize this:
- `fs::canonicalize()` on both sides handles this correctly
- Alternatively, strip `\\?\` prefix before comparison

### 7.3 Hardlink Deletion Semantics

- Deleting a hardlink (`remove_file`) only removes that one directory entry
- The file data persists until the last hardlink is removed
- This is safe for `agm unlink` — removing the tool-side hardlink leaves the central copy intact

### 7.4 `colored` Crate on Windows

The `colored` crate already handles Windows terminal detection (`ENABLE_VIRTUAL_TERMINAL_PROCESSING` on Windows 10+). No changes needed.

### 7.5 Git Operations

`Command::new("git")` works on Windows assuming Git for Windows is installed and `git.exe` is in PATH. No changes needed.

### 7.6 `dialoguer` Crate on Windows

The `dialoguer` crate supports Windows terminals (cmd.exe, PowerShell, Windows Terminal). No changes needed.

### 7.7 Order of Operations on Windows

Since Windows hardlinks require the source file to exist, the link creation order matters:
1. `agm init` must create central directories and files first
2. `agm link` can then create hardlinks/junctions pointing to existing central files

The current code already follows this order, but Windows makes it a hard requirement rather than a soft one.

---

## Summary of File Changes

| File | Change Type | Description |
|------|-------------|-------------|
| `src/platform.rs` | **NEW** | Platform abstraction layer |
| `src/linker.rs` | Modify | Use `platform::*`, add `is_dir` param |
| `src/files.rs` | Modify | Use `platform::*`, fix `centralized_path()` |
| `src/skills.rs` | Modify | Use `platform::*` |
| `src/main.rs` | Modify | Use `platform::*`, add `mod platform` |
| `src/editor.rs` | Modify | Use `platform::default_editor()` |
| `src/config.rs` | Modify | Fix `resolve_path()` for Windows |
| `src/paths.rs` | Minor | Tests may need path separator awareness |
| `.github/workflows/release.yml` | Modify | Add Windows matrix + zip packaging |
| `Cargo.toml` | Modify | Add `junction` dependency (cfg windows) |
| `README.md` | Modify | Add Windows section |
| `RELEASE.md` | Modify | Add Windows platform info |
