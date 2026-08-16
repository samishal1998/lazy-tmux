use std::path::PathBuf;

use anyhow::{anyhow, Result};
use lazytmux_core::{Session, Tmux, TmuxContext, Window};

use crate::commands::sessions;
use crate::{interactive, output};

/// Session scope for windows/panes commands: explicit flag, else the session
/// we are inside, else an interactive picker.
pub fn resolve_session(
    tmux: &Tmux,
    ctx: &TmuxContext,
    flag: Option<String>,
    prompt: &str,
) -> Result<Session> {
    match flag {
        Some(name) => sessions::resolve(tmux, &name),
        None => match sessions::current(tmux, ctx)? {
            Some(s) => Ok(s),
            None => interactive::pick_session(tmux, prompt),
        },
    }
}

pub fn resolve_window(
    tmux: &Tmux,
    session: &Session,
    target: Option<String>,
    prompt: &str,
) -> Result<Window> {
    match target {
        Some(t) => tmux
            .list_windows(&session.id)?
            .into_iter()
            .find(|w| w.index.to_string() == t || w.name == t)
            .ok_or_else(|| {
                anyhow!("no window '{t}' in session '{}'", session.name)
            }),
        None => interactive::pick_window(tmux, session, prompt),
    }
}

/// tmux target for a window, safe against odd session names.
pub fn target_of(session: &Session, window: &Window) -> String {
    format!("{}:{}", session.id, window.index)
}

pub fn list(tmux: &Tmux, ctx: &TmuxContext, session_flag: Option<String>) -> Result<()> {
    let session = resolve_session(tmux, ctx, session_flag, "List windows of:")?;
    let windows = tmux.list_windows(&session.id)?;
    println!("session '{}':", session.name);
    let rows: Vec<Vec<String>> = windows
        .iter()
        .map(|w| {
            let marker = if w.active { "*" } else { " " };
            vec![
                format!("{marker} {}", w.index),
                w.name.clone(),
                w.panes.to_string(),
            ]
        })
        .collect();
    output::table(&["  INDEX", "NAME", "PANES"], &rows);
    Ok(())
}

pub fn new(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    name: Option<String>,
    dir: Option<PathBuf>,
) -> Result<()> {
    let session = resolve_session(tmux, ctx, session_flag, "Create a window in:")?;
    let name = match name {
        Some(n) => Some(n),
        None => interactive::optional_name("Window name:")?,
    };
    let window = tmux.new_window(&session.id, name.as_deref(), dir.as_deref())?;
    println!(
        "created window '{}: {}' in session '{}'",
        window.index, window.name, session.name
    );
    Ok(())
}

pub fn kill(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    target: Option<String>,
    yes: bool,
) -> Result<()> {
    let session = resolve_session(tmux, ctx, session_flag, "Kill a window in:")?;
    let window = resolve_window(tmux, &session, target, "Kill window:")?;
    if !yes
        && !interactive::confirm(&format!(
            "Kill window '{}: {}' in session '{}'?",
            window.index, window.name, session.name
        ))?
    {
        println!("aborted");
        return Ok(());
    }
    tmux.kill_window(&window.id)?;
    println!("killed window '{}: {}'", window.index, window.name);
    Ok(())
}

pub fn rename(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    target: Option<String>,
    to: Option<String>,
) -> Result<()> {
    let session = resolve_session(tmux, ctx, session_flag, "Rename a window in:")?;
    let window = resolve_window(tmux, &session, target, "Rename window:")?;
    let to = match to {
        Some(t) => t,
        None => interactive::required_name(
            &format!("New name for '{}: {}':", window.index, window.name),
            "the new name",
        )?,
    };
    tmux.rename_window(&window.id, &to)?;
    println!("renamed window '{}' -> '{to}'", window.name);
    Ok(())
}

pub fn select(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    target: Option<String>,
) -> Result<()> {
    let session = resolve_session(tmux, ctx, session_flag, "Select a window in:")?;
    let window = resolve_window(tmux, &session, target, "Select window:")?;
    tmux.select_window(&target_of(&session, &window))?;
    println!(
        "selected window '{}: {}' in session '{}'",
        window.index, window.name, session.name
    );
    Ok(())
}
