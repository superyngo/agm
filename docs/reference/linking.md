# Linking

How AGM decides what a **Link** is, checks it, creates it, and removes it. Defined in
`src/linker.rs` and `src/platform.rs`. Terms are from [glossary.md](glossary.md).

## Link kinds per platform

| | Directory link | File link |
|---|---|---|
| Unix | symlink | symlink |
| Windows | NTFS junction | hardlink |

`platform.rs` is the only module that knows the difference; everything above it passes an
`is_dir: bool` and calls `platform::link_dir` / `link_file` / `remove_link` / `is_dir_link` /
`read_dir_link_target` / `same_file`. On Windows both link kinds require the target to exist
first. `platform::check_link_capability` can probe the system's ability to create links.

Consequence of the Windows file strategy: a hardlinked **Prompt** is indistinguishable from a
regular file by path inspection, so file-link checks fall back to comparing file identity
(`same_file`, inode on Unix / file index on Windows).

## Link status

`LinkStatus` is computed by comparing the canonicalized actual target against the canonicalized
expected target.

| Variant | Condition | Rendered as (CLI) |
|---|---|---|
| `Linked` | Link exists, points at the expected target, and the target exists | `✓ linked` |
| `Wrong(actual)` | Link exists but points somewhere else | `✗ wrong → <actual>` |
| `Blocked` | Path exists but is not a link of the expected kind | `✗ not linked` |
| `Missing` | Nothing exists at the path (`symlink_metadata` fails) | `✗ missing` |
| `Broken` | Link exists and points at the expected target, but the target is gone | `✗ broken` |

`Missing` is decided first, before the directory/file split. A relative link target is resolved
against the link's parent directory before comparison.

## `create_link`

Dispatches on the current **Link status**:

| Status | Action | Returns |
|---|---|---|
| `Linked` | nothing — prints `skip <label> already linked` | `false` |
| `Missing` | create the link | `true` |
| `Broken` | remove the stale link, create it again, print `(repaired broken link)` | `true` |
| `Wrong` | **refuse**, print a warning naming both targets | `false` |
| `Blocked` | **refuse**, print `exists but is not a link, skipping` | `false` |

`create_link` never deletes real content. Anything destructive — migrating a populated skills
directory, deleting an empty one, backing up a non-empty **Prompt** — happens in the caller
(`link_all`, see [cli.md](cli.md#agm-tool-link)), not here.

## `remove_link`

- Nothing at the path → `skip <label> not found`, returns `false`.
- Directory: removed only if `platform::is_dir_link` says it is a directory link; otherwise
  warned and skipped.
- File: removed if it is a symlink. On Windows, a non-symlink at a managed path is also removed
  (it is assumed to be the hardlink); on Unix a non-symlink is warned and skipped.

## Quiet variants

`create_link_quiet` and `remove_link_quiet` implement the same decisions but return
`(changed, message)` instead of printing, so the TUIs can render outcomes into the log popup
instead of writing to a terminal they own. See [tui.md](tui.md).

## Machine-checked claims

`src/linker.rs` unit tests cover every `LinkStatus` branch and the create/remove decision table
against temp directories. `src/platform.rs` unit tests cover the link primitives for the current
platform. Run with `cargo test`.
