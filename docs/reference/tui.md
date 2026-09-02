# TUI

Structure and behavior of the two ratatui interfaces. Keys live in [KEYMAP.md](KEYMAP.md);
terms in [glossary.md](glossary.md).

## Common shape

Both managers render: a title bar (`agm — <surface> Manager` left, `v<version>` right), a
flat-rendered tree list, and a three-row footer whose top line is a context-sensitive key hint
built from the row under the cursor and whose bottom line shows, in priority order, background
task progress (`⟳ …`), a status message, a selection count, or nothing. Status messages expire
after 3 seconds.

The list is a `Vec` of row enums rebuilt from config/disk state plus an `expanded` set — there is
no persistent tree object. Fold arrows are `▼` / `▶`; the cursor row is prefixed `▸`.

Popups render after the list, and the Help / About panel renders after everything else, so it is
always on top. Scrollable popups cap content at 5000 lines and append
`[truncated — content too large]` beyond that.

## Tool Manager (`agm tool`)

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

## Source Manager (`agm source`)

Rows: three category headers — Skills, Agents, Commands — each expanding into **Source** headers,
each expanding into **Item** rows. Headers carry an `[installed/total]` count; source headers
carry a kind icon and label (`Repo`, `Local`, `Migrated`). Item rows show a selection marker, the
cursor caret, an **Install status** icon (`✓` installed, `○` not installed, `✗` conflict), the
name padded to 30 columns, and the status word.

Fuzzy search (`/`) filters the list and highlights the matched characters in the name.

### Selection

Two sets, per the two-set model:

- **Committed** — items toggled with `s` or `Ctrl+A`.
- **Preview** — the live `Shift+↑`/`Shift+↓` range from the anchor to the cursor.

An action or a plain cursor move commits the preview range into the committed set. The effective
selection is the union of both.

Selection **survives refresh**: before rescanning, the committed set is snapshotted as
`(category, source name, item name)` triples; index-based state is cleared; after the rescan the
triples are re-resolved to new indices, and anything that no longer exists is dropped. Every
refresh path — `F5`, background add, background update, rename, delete — goes through this.

### Deletion

Deleting a `Repo` or `Local` **Source** takes a `y`/`Y` confirmation. Deleting a `Migrated`
**Source** requires typing the literal word `delete`, because that content exists nowhere else —
it was moved out of a **Tool**.

## Background work

`git` work never runs on the UI thread. `spawn_update` and `spawn_add` run
`skills::update_all_with_progress`, `clone_or_pull`, or `add_local_copy` on a thread and send
`TaskEvent`s over an `mpsc` channel: per-repo start/complete, streamed git output lines, and a
done event carrying the new **Item** counts. The main loop drains the channel non-blockingly
every tick, appends to the log, updates footer progress, and calls `refresh()` on completion.

## Log

An in-memory ring buffer of the last 500 entries, each timestamped `%H:%M:%S` at one of four
levels — info, success, warning, error. Opened with `o`, always scrolled to the newest entry. It
is not persisted to disk.

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

None. The TUIs have no automated coverage; behavior here was read from `src/tui/`.
