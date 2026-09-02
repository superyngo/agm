# 0003 — Windows support via junctions and hardlinks behind one abstraction

Date: 2026-03-20

## Context

AGM was Unix-only: `std::os::unix::fs::symlink` was called directly from `linker.rs`,
`skills.rs`, and `main.rs`. Windows support was wanted, but Windows symlinks require either
Developer Mode or elevation — unacceptable for a tool a user installs and runs immediately.

## Decision

Introduce `src/platform.rs` as the single `#[cfg]` boundary in the codebase and express every
link operation through it: `link_dir`, `link_file`, `remove_link`, `is_dir_link`,
`read_dir_link_target`, `same_file`, plus `default_editor`.

Windows uses the two mechanisms that need no privileges:

- **directories → NTFS junctions**
- **files → hardlinks**

Unix uses symlinks for both. Callers pass an `is_dir: bool` and never branch on the OS.

## Alternatives rejected

- **Windows symlinks.** Cleanest semantics, but they demand Developer Mode or an elevated
  process. Requiring either would make the first run fail for most users.
- **Copy files on Windows.** Would fork the product's central invariant
  ([0002](0002-links-not-copies.md)) along a platform line — one platform live-shared, the other
  needing sync.
- **`#[cfg]` at each call site.** Was the status quo. Platform logic would keep leaking into
  domain code, and each new operation is a new place to get Windows wrong.

## Consequences

- A hardlink is indistinguishable from a regular file by path inspection, so file-link checks
  cannot rely on `read_link`. They fall back to file-identity comparison (`same_file`: inode on
  Unix, file index on Windows), and on Windows a non-symlink at a managed path is treated as the
  hardlink and removed.
- Hardlinks cannot span volumes and do not track renames. A **Central store** and a **Config
  dir** on different drives will fail to link on Windows.
- On Windows the target must exist before the link is created; creation order matters.
- Every future OS-specific behavior belongs in `platform.rs`. A `#[cfg]` anywhere else is a
  regression of this decision.
