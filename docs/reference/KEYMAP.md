# Keymap

Every key handled by the two TUIs, as bound in `src/tui/`. Surfaces are the **Tool Manager**
(`agm tool`) and the **Source Manager** (`agm source`); see [tui.md](tui.md) for what they show.

## Global — both managers, normal mode

| Key | Action |
|---|---|
| `↑` / `k` | Cursor up one row |
| `↓` / `j` | Cursor down one row |
| `PgUp` / `PgDn` | Cursor up/down one page (list height − 5) |
| `Home` / `End` | Cursor to first / last row |
| `␣` / `⏎` | Header row: toggle fold. Item row: open the info popup |
| `9` / `0` | Expand all / collapse all |
| `i` | Info popup for the row under the cursor |
| `e` | Edit — see per-surface rows below |
| `l` | Link action — see per-surface rows below |
| `o` | Open the log popup (scrolled to the newest entry) |
| `?` | Open the Help / About panel |
| `Esc` | Peel exactly one layer (see below) — never quits |
| `q` / `Ctrl+C` | Quit |

## Tool Manager

| Key | Action |
|---|---|
| `e` | On a central path row: open the inline path editor. On a tool row: open the tool's TOML section, prompt, or config file in the editor; offers to create the file if missing |
| `l` | On a central row: confirm enabling/disabling that **Feature** for all installed tools. On the status header: link/unlink everything for that tool. On a single link row: toggle that link |

## Source Manager

| Key | Action |
|---|---|
| `s` | Toggle selection of the item under the cursor |
| `Shift+↑` / `Shift+↓` | Extend the range selection from the anchor |
| `Ctrl+A` | Select every item in the current **Source** group |
| `l` | With a selection: confirm bulk install/uninstall of the selected items. Without: toggle install of the item under the cursor, or bulk-toggle a whole **Source** header |
| `e` | Open the source directory, skill directory, agent file, or command file in the editor |
| `a` | Add a **Source** — inline prompt for a URL or local path |
| `r` | Rename the **Source** under the cursor — inline prompt |
| `d` / `Del` | Delete the **Source** under the cursor (confirmation required) |
| `u` | `git pull` every **Source** in the background |
| `F5` | Rescan sources and prune broken links |
| `/` | Enter fuzzy search; expands all rows |

## Popups

All scrollable popups — info, log, Help / About — share:

| Key | Action |
|---|---|
| `↑` / `k`, `↓` / `j` | Scroll one line |
| `PgUp` / `PgDn` | Scroll one page |
| `Home` / `End` | Scroll to top / bottom |
| `Esc` / `⏎` / `␣` | Close |

Popup-specific:

| Key | Surface | Action |
|---|---|---|
| `o` | Log popup | Close |
| `i` | Info popup | Close |
| `e` | Info popup | Close and open the subject path in the editor |
| `l` | Info popup (Tool Manager) | Toggle that link or **Feature** |
| `Tab` / `Shift+Tab` | Help / About | Switch tab |
| `?` | Help / About | Close |

## Confirmations and inline prompts

| Key | Context | Action |
|---|---|---|
| `y` / `Y` | any confirmation | Confirm. In the Tool Manager's create-file dialog, `⏎` also confirms |
| `n` / `N` / `Esc` | any confirmation | Cancel. In the Source Manager any other key also cancels |
| type `delete` | deleting a `Migrated` **Source** | Required literal confirmation; `Backspace` edits, a diverging character cancels |
| `⏎` | add / rename prompt | Submit |
| `Esc` | add / rename prompt | Cancel |
| `Esc` | search | Cancel search and clear the filter |
| `⏎` | search | Leave the input but keep the filter active |
| `Ctrl+↑` / `Ctrl+k`, `Ctrl+↓` / `Ctrl+j` | search | Move the cursor within filtered results while still typing |

## Text fields

Text fields (add, rename, path editor) accept:

| Key | Action |
|---|---|
| `←` / `→` | Cursor one character |
| `Home` / `End` | Cursor to start / end |
| `Backspace` / `Del` | Delete before / at the cursor |
| printable char | Insert at the cursor |

Movement is per Unicode scalar, so multibyte text is never split; there is no selection or
clipboard support inside a text field. `⏎` and `Esc` are deliberately **not** consumed by the
field — the enclosing mode decides what submit and cancel mean.

## The Esc contract

`Esc` peels exactly one layer per press, in this order, and never quits:

1. Close the Help / About panel, if open.
2. Close the surface popup or dialog.
3. Cancel the active edit / confirm mode.
4. Clear the multi-selection (Source Manager).
5. Clear the search query and filter (Source Manager).
6. Clear the status message.

## Machine-checked claims

None — this table is maintained by hand against `src/tui/`. The in-app Help panel (`?`) is
generated from `src/tui/help.rs` and is the authority if the two ever disagree.
