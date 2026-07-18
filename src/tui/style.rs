//! Style helpers shared by every TUI surface.
//!
//! Two concerns live here:
//!
//! 1. **`NO_COLOR`** (https://no-color.org/): when the env var is set *to any
//!    value* (including empty), all TUI rendering drops ANSI colors. The CLI
//!    side already honors it via the `colored` crate; this module extends it to
//!    the ratatui surfaces by stripping `fg`/`bg`/`underline_color` on the
//!    buffer cells right before flush. Bold/italic modifiers are preserved so
//!    headers and the cursor `▸` marker remain visible even without color —
//!    this keeps the focus cursor findable (principle 5) when the user opts
//!    into monochrome.
//!
//! 2. **Shared rendering primitives** that were previously duplicated between
//!    `tool.rs` and `source.rs`: the `hint_key` / `hint_text` span builders for
//!    the key-hint footer, and the `StatusLine` (a `(String, Instant)` buffer
//!    with set / clear / clear_expired semantics, used by every App).

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

// ---------------------------------------------------------------------------
// NO_COLOR
// ---------------------------------------------------------------------------

/// True when the `NO_COLOR` env var is present (regardless of its value, per
/// the spec). Read once at startup and cached as a static for cheap access in
/// the render loop.
pub fn no_color() -> bool {
    static CACHED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *CACHED.get_or_init(|| std::env::var_os("NO_COLOR").is_some())
}

/// Strip color attributes from every cell in `buffer`. Call this at the very
/// end of a render closure (after all widgets are drawn) when [`no_color()`]
/// is true. Modifiers (bold, italic, etc.) are kept so emphasis is still
/// visible without color.
pub fn strip_colors(buffer: &mut Buffer) {
    for cell in buffer.content.iter_mut() {
        if cell.fg != Color::Reset {
            cell.fg = Color::Reset;
        }
        if cell.bg != Color::Reset {
            cell.bg = Color::Reset;
        }
        // underline_color only exists behind the `underline-color` feature; we
        // don't currently use it in styles, so leave the field untouched.
    }
}

// ---------------------------------------------------------------------------
// Footer hint spans
// ---------------------------------------------------------------------------

/// One yellow-bold span for a key in the footer hints (e.g. `␣`, `?`, `q`).
pub fn hint_key(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), Style::default().fg(Color::Yellow))
}

/// One plain-text span for the description following a key in the footer hints.
pub fn hint_text(s: &str) -> Span<'static> {
    Span::raw(s.to_string())
}

/// Convenience constructor for the trailing `? help  q quit` pair that every
/// hint row appends. Centralized so a future change to the suffix (e.g.
/// changing the quit key) only has to touch one place.
pub fn hint_help_quit() -> [Span<'static>; 4] {
    [
        hint_key("?"),
        hint_text(" help  "),
        hint_key("q"),
        hint_text(" quit"),
    ]
}

// ---------------------------------------------------------------------------
// Status line
// ---------------------------------------------------------------------------

/// Default time-to-live for a status message before it auto-expires. Keeps
/// the footer from getting stuck on a stale success/error message.
pub const STATUS_TTL: Duration = Duration::from_secs(3);

/// `(message, set_at)` buffer used by every TUI App for the footer status line.
/// Auto-expires after [`STATUS_TTL`]. Both Apps had identical inline
/// implementations before this was extracted.
#[derive(Debug, Default, Clone)]
pub struct StatusLine {
    inner: Option<(String, Instant)>,
}

impl StatusLine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, msg: impl Into<String>) {
        self.inner = Some((msg.into(), Instant::now()));
    }

    pub fn clear(&mut self) {
        self.inner = None;
    }

    pub fn is_set(&self) -> bool {
        self.inner.is_some()
    }

    /// Drop the message if it has been visible longer than [`STATUS_TTL`].
    pub fn clear_expired(&mut self) {
        if let Some((_, when)) = &self.inner {
            if when.elapsed() >= STATUS_TTL {
                self.inner = None;
            }
        }
    }

    /// Render the message styled as a single `Line` (or an empty line if
    /// nothing is set). Color is green to match the existing convention.
    pub fn to_line(&self) -> Line<'static> {
        match &self.inner {
            Some((msg, _)) => {
                Line::from(Span::styled(msg.clone(), Style::default().fg(Color::Green)))
            }
            None => Line::default(),
        }
    }
}

/// Style used for a highlighted (active cursor) row's foreground. Centralized
/// so the cursor color is consistent across surfaces.
pub fn cursor_fg() -> Style {
    Style::default().fg(Color::Yellow)
}

/// Style for a section header inside a popup body.
pub fn section_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;

    #[test]
    fn strip_resets_fg_and_bg_but_keeps_modifier() {
        use ratatui::layout::Position;

        // Build a buffer with one styled cell, then verify strip_colors resets
        // only fg/bg.
        let mut buf = Buffer::empty(ratatui::layout::Rect::new(0, 0, 1, 1));
        let cell = buf.cell_mut(Position { x: 0, y: 0 }).unwrap();
        cell.set_style(
            Style::default()
                .fg(Color::Red)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        );

        strip_colors(&mut buf);

        let cell = buf.cell(Position { x: 0, y: 0 }).unwrap();
        assert_eq!(cell.fg, Color::Reset, "fg should be reset under NO_COLOR");
        assert_eq!(cell.bg, Color::Reset, "bg should be reset under NO_COLOR");
        assert!(
            cell.modifier.contains(Modifier::BOLD),
            "modifiers must survive so emphasis (and the cursor marker) remain visible"
        );
    }

    #[test]
    fn no_color_reads_env_var() {
        // We can't reliably mutate the process env for a single test (other
        // tests in the same binary would see the change), so we only assert
        // the helper returns a consistent bool across calls. The actual
        // env-read is exercised by setting NO_COLOR before running the binary.
        let a = no_color();
        let b = no_color();
        assert_eq!(a, b, "no_color() must be idempotent (cached via OnceLock)");
    }

    #[test]
    fn hint_helpers_build_spans() {
        let k = hint_key("?");
        let t = hint_text(" help");
        assert_eq!(&*k.content, "?");
        assert_eq!(&*t.content, " help");
    }

    #[test]
    fn status_line_set_clear_expire() {
        let mut s = StatusLine::new();
        assert!(!s.is_set());

        s.set("hello");
        assert!(s.is_set());
        assert_eq!(s.to_line().to_string(), "hello");

        s.clear_expired(); // TTL is 3s, won't fire yet
        assert!(s.is_set());

        s.clear();
        assert!(!s.is_set());
        assert_eq!(s.to_line().to_string(), "");
    }
}
