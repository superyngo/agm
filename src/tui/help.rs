//! Shared Help + About panel (principle 18).
//!
//! One scrollable popup with two tabs:
//!   - `Help`  — keybindings for the current surface (Tool/Source manager)
//!   - `About` — single-sourced app metadata from `CARGO_PKG_*`
//!
//! Open with `?`; switch tabs with `Tab`; close with `?`/`Esc` (principle 14:
//! Esc peels exactly one layer — the popup, not the app).

use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    Frame,
};

use super::popup::PopupAction;

/// Which surface the help is being shown for. The keybinding tables differ
/// between the Tool manager and Source manager, so the caller tells us which
/// set to render.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HelpSurface {
    Tool,
    Source,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Help,
    About,
}

pub struct HelpPopup {
    tab: Tab,
    surface: HelpSurface,
    popup: super::popup::ScrollablePopup,
}

impl HelpPopup {
    pub fn new(surface: HelpSurface) -> Self {
        let tab = Tab::Help;
        let popup = build_popup(tab, surface);
        Self {
            tab,
            surface,
            popup,
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> PopupAction {
        // Tab cycles between Help and About. Each switch rebuilds the
        // underlying ScrollablePopup so the title, hint, and content match.
        if matches!(code, KeyCode::Tab | KeyCode::BackTab) {
            let next = match self.tab {
                Tab::Help => Tab::About,
                Tab::About => Tab::Help,
            };
            self.tab = next;
            self.popup = build_popup(next, self.surface);
            return PopupAction::Consumed;
        }
        // `?` toggles closed (symmetric with the open binding).
        if matches!(code, KeyCode::Char('?')) {
            return PopupAction::Close;
        }
        self.popup.handle_key(code)
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        self.popup.render(frame, area);
    }
}

/// Build the title line that shows BOTH tabs at once. The active tab is bold
/// cyan with `▸`, the inactive tab is dim gray — so the title itself
/// communicates "this panel has two switchable sections" without requiring the
/// user to read the bottom hint.
fn tab_title(active: Tab) -> Line<'static> {
    let active_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let inactive_style = Style::default().fg(Color::DarkGray);
    let sep = Span::styled("  ·  ", Style::default().fg(Color::DarkGray));

    let help_span = Span::styled(
        if active == Tab::Help {
            "▸ Help".to_string()
        } else {
            "Help".to_string()
        },
        if active == Tab::Help {
            active_style
        } else {
            inactive_style
        },
    );
    let about_span = Span::styled(
        if active == Tab::About {
            "▸ About".to_string()
        } else {
            "About".to_string()
        },
        if active == Tab::About {
            active_style
        } else {
            inactive_style
        },
    );

    Line::from(vec![help_span, sep, about_span])
}

fn build_popup(tab: Tab, surface: HelpSurface) -> super::popup::ScrollablePopup {
    let title = tab_title(tab);
    match tab {
        Tab::Help => {
            let lines = build_help_lines(surface);
            super::popup::ScrollablePopup::new("Help", lines)
                .with_title_line(title)
                .with_close_hint("?/Esc:close  Tab:switch")
        }
        Tab::About => {
            let lines = build_about_lines();
            super::popup::ScrollablePopup::new("About", lines)
                .with_title_line(title)
                .with_close_hint("?/Esc:close  Tab:switch")
        }
    }
}

// ---------------------------------------------------------------------------
// Content builders
// ---------------------------------------------------------------------------

fn key_span(s: &str) -> Span<'static> {
    Span::styled(
        s.to_string(),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
}

fn text_span(s: &str) -> Span<'static> {
    Span::raw(s.to_string())
}

/// One `key  description` row, two-space gap, fixed-width key column so the
/// descriptions align.
fn binding_row(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw("  "),
        key_span(&format!("{:<14}", key)),
        text_span(desc),
    ])
}

fn section(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        title.to_string(),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn blank() -> Line<'static> {
    Line::from("")
}

pub fn build_help_lines(surface: HelpSurface) -> Vec<Line<'static>> {
    // Common navigation/fold rows shared by both surfaces.
    let mut lines = vec![
        section("Navigation"),
        binding_row("↑/↓  k/j", "Move cursor"),
        binding_row("PgUp/PgDn", "Scroll by page"),
        binding_row("Home/End", "Jump to top / bottom"),
        blank(),
        section("Fold"),
        binding_row("␣  ⏎", "Toggle fold / open info"),
        binding_row("9", "Expand all"),
        binding_row("0", "Collapse all"),
        blank(),
    ];

    match surface {
        HelpSurface::Tool => {
            lines.push(section("Items"));
            lines.push(binding_row("i", "Info popup for row"));
            lines.push(binding_row("e", "Edit file or path"));
            lines.push(binding_row("l", "Toggle link / feature"));
            lines.push(blank());
        }
        HelpSurface::Source => {
            lines.push(section("Items"));
            lines.push(binding_row("i", "Info popup for row"));
            lines.push(binding_row("l", "Install / uninstall"));
            lines.push(binding_row("e", "Edit source file"));
            lines.push(binding_row("d", "Delete source"));
            lines.push(blank());

            lines.push(section("Selection (multi)"));
            lines.push(binding_row("s", "Toggle item selection"));
            lines.push(binding_row("Shift+↑/↓", "Range select"));
            lines.push(binding_row("Ctrl+A", "Select all in source"));
            lines.push(binding_row("Esc", "Clear selection"));
            lines.push(blank());

            lines.push(section("Sources"));
            lines.push(binding_row("a", "Add source"));
            lines.push(binding_row("r", "Rename source"));
            lines.push(binding_row("u", "Update all (git pull)"));
            lines.push(binding_row("F5", "Refresh list"));
            lines.push(binding_row("/", "Fuzzy search"));
            lines.push(blank());
        }
    }

    lines.push(section("Popups"));
    lines.push(binding_row("o", "Open log"));
    lines.push(binding_row("?", "Open this help / about"));
    lines.push(binding_row("Esc", "Close popup (one layer)"));
    lines.push(blank());

    lines.push(section("Quit"));
    lines.push(binding_row("q", "Quit"));
    if surface == HelpSurface::Source {
        lines.push(binding_row("Ctrl+C", "Quit"));
    }

    lines
}

pub fn build_about_lines() -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");
    let description = env!("CARGO_PKG_DESCRIPTION");
    let authors = env!("CARGO_PKG_AUTHORS");
    let license = env!("CARGO_PKG_LICENSE");
    let repo = env!("CARGO_PKG_REPOSITORY");
    let homepage = env!("CARGO_PKG_HOMEPAGE");

    lines.push(Line::from(vec![
        text_span("  "),
        Span::styled(
            name.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" v{}", version), Style::default().fg(Color::Yellow)),
    ]));
    lines.push(blank());
    lines.push(Line::from(vec![text_span("  "), text_span(description)]));
    lines.push(blank());

    lines.push(section("Details"));
    if !authors.is_empty() {
        lines.push(field_row("Authors", authors));
    }
    if !license.is_empty() {
        lines.push(field_row("License", license));
    }
    if !repo.is_empty() {
        lines.push(field_row("Repository", repo));
    }
    if !homepage.is_empty() {
        lines.push(field_row("Homepage", homepage));
    }
    lines.push(field_row("Config", "~/.config/agm/config.toml"));
    lines.push(field_row("Data", "~/.local/share/agm/"));
    lines.push(blank());

    lines.push(section("Privacy"));
    lines.push(Line::from(vec![
        text_span("  "),
        text_span("No telemetry. All configuration and links stay local."),
    ]));

    lines
}

fn field_row(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        text_span("  "),
        Span::styled(
            format!("{:<11}", label),
            Style::default().fg(Color::DarkGray),
        ),
        text_span(value),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_tool_lists_key_sections() {
        let lines = build_help_lines(HelpSurface::Tool);
        let joined = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("Navigation"));
        assert!(joined.contains("Fold"));
        assert!(joined.contains("Items"));
        assert!(joined.contains("Popups"));
        assert!(joined.contains("Quit"));
        // Tool surface does not have search/sources sections.
        assert!(!joined.contains("Fuzzy search"));
    }

    #[test]
    fn help_source_lists_source_specific_keys() {
        let lines = build_help_lines(HelpSurface::Source);
        let joined = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("Selection (multi)"));
        assert!(joined.contains("Sources"));
        assert!(joined.contains("Ctrl+A"));
        assert!(joined.contains("Ctrl+C"));
    }

    #[test]
    fn about_includes_name_version_description() {
        let lines = build_about_lines();
        let joined = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains(env!("CARGO_PKG_NAME")));
        assert!(joined.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))));
        assert!(joined.contains(env!("CARGO_PKG_DESCRIPTION")));
        // Privacy note is always present.
        assert!(joined.contains("No telemetry"));
    }

    #[test]
    fn about_omits_empty_optional_fields() {
        // Cargo.toml currently has no authors/license/repo/homepage set, so
        // none of those labels should appear.
        let lines = build_about_lines();
        let joined = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        if env!("CARGO_PKG_AUTHORS").is_empty() {
            assert!(!joined.contains("Authors"));
        }
        if env!("CARGO_PKG_LICENSE").is_empty() {
            assert!(!joined.contains("License"));
        }
    }

    #[test]
    fn tab_key_cycles_between_help_and_about() {
        let mut popup = HelpPopup::new(HelpSurface::Tool);
        // Initial tab is Help.
        assert_eq!(popup.tab, Tab::Help);

        // Tab switches to About.
        assert_eq!(popup.handle_key(KeyCode::Tab), PopupAction::Consumed);
        assert_eq!(popup.tab, Tab::About);

        // Tab again switches back to Help.
        assert_eq!(popup.handle_key(KeyCode::Tab), PopupAction::Consumed);
        assert_eq!(popup.tab, Tab::Help);

        // Backtab also cycles.
        assert_eq!(popup.handle_key(KeyCode::BackTab), PopupAction::Consumed);
        assert_eq!(popup.tab, Tab::About);
    }

    #[test]
    fn question_mark_closes_popup() {
        let mut popup = HelpPopup::new(HelpSurface::Tool);
        assert_eq!(popup.handle_key(KeyCode::Char('?')), PopupAction::Close);
    }

    #[test]
    fn escape_closes_popup() {
        let mut popup = HelpPopup::new(HelpSurface::Tool);
        assert_eq!(popup.handle_key(KeyCode::Esc), PopupAction::Close);
    }

    #[test]
    fn scrolling_keys_are_consumed() {
        let mut popup = HelpPopup::new(HelpSurface::Tool);
        assert_eq!(popup.handle_key(KeyCode::Down), PopupAction::Consumed);
        assert_eq!(popup.handle_key(KeyCode::PageDown), PopupAction::Consumed);
    }

    /// The title shows BOTH tabs at once, with a `▸` marker on the active one.
    /// This is what tells the user "Help and About are switchable within this
    /// same panel" without having to read the bottom hint.
    #[test]
    fn tab_title_marks_active_tab_with_arrow() {
        let help_active = tab_title(Tab::Help);
        let help_text: String = help_active
            .spans
            .iter()
            .map(|s| s.content.as_ref().to_string())
            .collect::<String>();
        assert!(help_text.contains("▸ Help"));
        assert!(!help_text.contains("▸ About"));
        assert!(help_text.contains("About"));

        let about_active = tab_title(Tab::About);
        let about_text: String = about_active
            .spans
            .iter()
            .map(|s| s.content.as_ref().to_string())
            .collect::<String>();
        assert!(about_text.contains("▸ About"));
        assert!(!about_text.contains("▸ Help"));
        assert!(about_text.contains("Help"));
    }

    /// The HelpPopup's underlying ScrollablePopup always carries the styled
    /// tab-bar title — so the title is rendered, not just stored as a string.
    #[test]
    fn popup_carries_styled_title_line() {
        let popup = HelpPopup::new(HelpSurface::Tool);
        assert!(
            popup.popup.title_line.is_some(),
            "HelpPopup must set a styled title_line so both tabs render in the title bar"
        );
    }
}
