//! Unified shell hosting the Tool Manager and Source Manager behind Tab.
//!
//! Exactly one of the two screens is ever rendered per frame. Both may be
//! alive at once (the inactive one keeps ticking its background work, e.g. a
//! Source `git pull` in flight while the user is looking at Tool), but only
//! the screen that has actually been visited is constructed — visiting
//! `agm tool` alone never touches the source-scanning / git side effects
//! that `agm source` triggers, and vice versa.

use std::io::stdout;
use std::path::PathBuf;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::config::Config;

use super::{source, tool};

/// Which screen is currently on top.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Tool,
    Source,
}

pub fn run(config_path: Option<PathBuf>, initial: Tab) -> Result<()> {
    let config = Config::load_from(config_path.clone())?;

    let mut tool_app: Option<tool::ToolApp> = None;
    let mut source_app: Option<source::App> = None;
    match initial {
        Tab::Tool => tool_app = Some(tool::ToolApp::new(config.clone(), config_path.clone())),
        Tab::Source => source_app = Some(source::App::build(config.clone())),
    }
    let mut active = initial;

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
        prev_hook(info);
    }));

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        let area_height = terminal.size()?.height;

        // Background work (currently only the Source Manager has any) keeps
        // draining even while its screen is not the active one.
        if let Some(app) = &mut source_app {
            app.tick();
        }

        match active {
            Tab::Tool => tool_app
                .as_mut()
                .expect("tool screen active but not built")
                .ensure_visible(area_height),
            Tab::Source => source_app
                .as_mut()
                .expect("source screen active but not built")
                .ensure_visible(area_height),
        }

        terminal.draw(|frame| {
            match active {
                Tab::Tool => tool::render(
                    tool_app.as_mut().expect("tool screen active but not built"),
                    frame,
                ),
                Tab::Source => source::render(
                    source_app
                        .as_mut()
                        .expect("source screen active but not built"),
                    frame,
                ),
            }
            if super::style::no_color() {
                super::style::strip_colors(frame.buffer_mut());
            }
        })?;

        match active {
            Tab::Tool => tool_app.as_mut().unwrap().clear_expired_status(),
            Tab::Source => source_app.as_mut().unwrap().clear_expired_status(),
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(event::KeyModifiers::CONTROL)
                    {
                        break;
                    }

                    let is_modal = match active {
                        Tab::Tool => tool_app.as_ref().unwrap().is_modal(),
                        Tab::Source => source_app.as_ref().unwrap().is_modal(),
                    };

                    if !is_modal && matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
                        active = match active {
                            Tab::Tool => {
                                let cfg = tool_app.as_ref().unwrap().config().clone();
                                match &mut source_app {
                                    Some(app) => app.sync_config(cfg),
                                    None => source_app = Some(source::App::build(cfg)),
                                }
                                Tab::Source
                            }
                            Tab::Source => {
                                if tool_app.is_none() {
                                    let cfg = source_app.as_ref().unwrap().config().clone();
                                    tool_app = Some(tool::ToolApp::new(cfg, config_path.clone()));
                                }
                                Tab::Tool
                            }
                        };
                    } else {
                        match active {
                            Tab::Tool => {
                                let app = tool_app.as_mut().unwrap();
                                app.handle_key(key.code, &mut terminal, area_height);
                                app.drain_pending_editor(&mut terminal);
                            }
                            Tab::Source => {
                                source_app.as_mut().unwrap().handle_key(
                                    key.code,
                                    key.modifiers,
                                    &mut terminal,
                                    area_height,
                                );
                            }
                        }
                    }
                }
            }
        }

        let should_quit = match active {
            Tab::Tool => tool_app.as_ref().unwrap().should_quit(),
            Tab::Source => source_app.as_ref().unwrap().should_quit(),
        };
        if should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    let _ = std::panic::take_hook();
    Ok(())
}
