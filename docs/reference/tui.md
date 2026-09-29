# TUI

Structure and behavior of the unified shell and its two screens. Keys live in
[KEYMAP.md](KEYMAP.md); terms in [glossary.md](glossary.md).

## The Shell

`agm` (bare), `agm tool`, and `agm source` all open one process — the **Shell**
(`src/tui/shell.rs`) — which owns the terminal lifecycle (panic hook, raw mode, alternate
screen) and hosts exactly one of two screens at a time: the **Tool Manager** or the **Source
Manager**. `agm tool` / `agm source` start on the matching screen; bare `agm` starts on the
**Tool Manager**.

`Tab` / `Shift+Tab` switches screens when the active screen has no modal open (no popup, no
search/add/rename input, no confirmation). The title bar shows both screens with the active one
bracketed — `agm — [Tool] · Source` or `agm — Tool · [Source]` — and the footer's hint line ends
with `Tab source` / `Tab tool`.

A screen is only constructed the first time it is visited, so `agm tool` alone never triggers
the **Source Manager**'s startup scan/background update, and vice versa. Once built, both
screens stay alive for the rest of the process — switching away does not drop cursor position,
expanded rows, or the log, and the **Source Manager**'s background `git` work keeps draining
every tick regardless of which screen is on top. Switching *into* the **Source Manager**
reloads its copy of the config (the **Tool Manager** is the only screen that persists config
edits to disk), so a change made on the Tool tab is visible immediately after switching.

## Common shape

Both screens render: a title bar (`agm — [Tool] · Source` / `agm — Tool · [Source]` left,
`v<version>` right), a flat-rendered tree list, and a three-row bordered footer with one content
line. That line shows the status message while one is set (it expires after 3 seconds), otherwise
the context-sensitive key hints for the row under the cursor, then `Tab`. The Source Manager also
builds a background-progress (`⟳ …`) and selection-count line, but it is only drawn when the footer
has two content lines, which the fixed height never gives (tracked in
[BACKLOG](../plan/BACKLOG.md) B12).

## Tool Manager screen (`agm tool`)

Rows: a central `agm` header with one row per configured path (source, prompt, skills, agents,
commands), then one header per **Tool** in **Tool key** order. Under each tool: a status header
with one row per configured **Feature** showing its **Link status** (`✓` linked / enabled, `✗`
not), and group headers for `settings`, `auth`, and `mcp` that expand into one row per file when
the group has more than one.

Actions available from the list: toggle a **Feature** globally, link/unlink one **Feature** or a
whole tool, edit any settings/auth/mcp file, edit the tool's own TOML section, and edit a central
path through an inline path editor. Editing a missing file first asks whether to create it.
Link operations use `linker::create_link_quiet` / `remove_link_quiet` so results land in the log
popup instead of the terminal.

## Source Manager screen (`agm source`)

Rows: three category headers — Skills, Agents, Commands — each expanding into **Source** headers,
each expanding into **Item** rows. Headers carry an `[installed/total]` count; source headers
carry a kind icon and label (`Repo`, `Local`, `Migrated`). Globally disabled **Feature**s are
greyed out and actions on their items are guarded with a warning status message. Item rows show a
selection marker, the cursor caret, an **Install status** icon (`✓` installed, `○` not installed,
`✗` conflict), the name padded to 30 columns, and the status word.

Fuzzy search (`/`) filters the list and highlights the matched characters in the name.

### Selection

Two sets, per the two-set model:

- **Committed** — items toggled with `s` or `Ctrl+A`.
- **Preview** — the live `Shift+↑`/`Shift+↓` range from the anchor to the cursor.

An action or a plain cursor move commits the preview range into the committed set. The effective
selection is the union of both. Pressing `l` with an active selection applies a bulk direction
rule: if any selected item is not installed, install all; otherwise uninstall all.

Duplicate-named items across sources in a category or within a single source are counted and
reported with a conflict warning in category info popups and bulk install confirmation dialogs
(`duplicate_name_count`).

Selection **survives refresh**: before rescanning, the committed set is snapshotted as
`(category, source name, item name)` triples; index-based state is cleared; after the rescan the
triples are re-resolved to new indices, and anything that no longer exists is dropped. Every
refresh path — `F5`, background add, background update, rename, delete — goes through this.

### Deletion

Deleting a `Repo` or `Local` **Source** takes a `y`/`Y` confirmation. Deleting a `Migrated`
**Source** requires typing the literal word `delete`, because that content exists nowhere else —
it was moved out of a **Tool**. A `Migrated` **Source** cannot be renamed.

While a background add or update is running, delete, rename, bulk install/uninstall and `F5`
are refused with a "Busy" status so they cannot race the rescan. Per-source UI state (expanded
sources, an open confirmation, the rename target) follows the **Source** across rescans.

Unlinking a **Feature** from the **Tool Manager** leaves the **Tool** with real copies of the
central content, exactly like `agm tool unlink` (`linker::detach_copy`).

## Background work

`git` work never runs on the UI thread. `spawn_update` and `spawn_add` run
`skills::update_all_with_progress`, `clone_or_pull`, or `add_local_copy` on a thread and send
`TaskEvent`s over an `mpsc` channel: per-repo start/complete, streamed git output lines, and a
done event carrying the new **Item** counts. The main loop drains the channel non-blockingly
every tick, appends to the log, updates footer progress, and calls `refresh()` on completion.

## Log

Each screen owns its own in-memory ring buffer (`LogBuffer`) of the last 500 entries, each
timestamped `%H:%M:%S` at one of four levels — info, success, warning, error. Opened with `o`,
always scrolled to the newest entry. It is not persisted to disk.

## Help / About

`?` opens a two-tab panel: **Help** is the keybinding table for the current surface, **About**
shows the crate's name, version, description, authors, license, repository, and homepage read
from `CARGO_PKG_*`. The title renders both tabs at once with `▸` marking the active one.

## Color

`NO_COLOR` (present at any value, including empty) is read once and cached. When set, the
rendered frame buffer is post-processed: every cell's foreground and background are reset while
`BOLD`, `ITALIC`, and `UNDERLINED` are preserved — so the cursor row and headers stay readable on
a monochrome terminal.

## Machine-checked claims

`shell.rs` has no automated integration harness, but `popup`, `help`, `style`, `text_input`,
`background`, `tool`, and `source` modules have unit tests in `src/tui/`. Run with `cargo test`.
