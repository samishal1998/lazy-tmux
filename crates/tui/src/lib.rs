//! Ratatui session manager: terminal lifecycle and event loop.
//!
//! `run` returns the session target the user chose to attach to (if any);
//! actually attaching is the caller's job, after the terminal is restored.

mod app;
mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use lazytmux_core::{TmuxContext, Tmux};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

pub use app::App;

pub fn run(tmux: Tmux, ctx: TmuxContext) -> Result<Option<String>> {
    let mut app = App::new(tmux, ctx);
    app.refresh_all();

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        orig_hook(info);
    }));

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let result = event_loop(&mut terminal, &mut app);

    restore_terminal();
    let _ = terminal.show_cursor();
    result?;
    Ok(app.attach_target.take())
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.on_key(key),
                _ => {}
            }
        }
        app.on_tick();
    }
    Ok(())
}
