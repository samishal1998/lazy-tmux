//! Interactive prompts (inquire) used whenever an argument is omitted.

use std::fmt;
use std::io::{stdin, IsTerminal};

use anyhow::{bail, Result};
use inquire::{Confirm, MultiSelect, Select, Text};
use lazytmux_core::{Pane, Session, Tmux, Window};

pub fn is_interactive() -> bool {
    stdin().is_terminal()
}

struct SessionItem(Session);

impl fmt::Display for SessionItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = &self.0;
        let attached = if s.is_attached() { ", attached" } else { "" };
        write!(f, "{} ({} windows{attached})", s.name, s.windows)
    }
}

struct WindowItem(Window);

impl fmt::Display for WindowItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let w = &self.0;
        let active = if w.active { ", active" } else { "" };
        write!(f, "{}: {} ({} panes{active})", w.index, w.name, w.panes)
    }
}

struct PaneItem(Pane);

impl fmt::Display for PaneItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let p = &self.0;
        let active = if p.active { ", active" } else { "" };
        write!(
            f,
            "{}: {} [{}x{}{active}]",
            p.index, p.command, p.width, p.height
        )
    }
}

pub fn pick_session(tmux: &Tmux, prompt: &str) -> Result<Session> {
    let sessions = tmux.list_sessions()?;
    if sessions.is_empty() {
        bail!("no tmux sessions exist (create one with `ltm sessions new`)");
    }
    if sessions.len() == 1 {
        return Ok(sessions.into_iter().next().unwrap());
    }
    require_tty("a session name")?;
    let items: Vec<SessionItem> = sessions.into_iter().map(SessionItem).collect();
    Ok(Select::new(prompt, items).prompt()?.0)
}

/// Pick from a pre-filtered set of sessions.
pub fn pick_from(sessions: Vec<Session>, prompt: &str) -> Result<Session> {
    if sessions.len() == 1 {
        return Ok(sessions.into_iter().next().unwrap());
    }
    require_tty("a session name")?;
    let items: Vec<SessionItem> = sessions.into_iter().map(SessionItem).collect();
    Ok(Select::new(prompt, items).prompt()?.0)
}

/// Pick any number of sessions (space selects, enter confirms).
pub fn pick_sessions(sessions: Vec<Session>, prompt: &str) -> Result<Vec<Session>> {
    let items: Vec<SessionItem> = sessions.into_iter().map(SessionItem).collect();
    let chosen = MultiSelect::new(prompt, items)
        .with_help_message("space selects, enter confirms")
        .prompt()?;
    Ok(chosen.into_iter().map(|i| i.0).collect())
}

pub fn pick_window(tmux: &Tmux, session: &Session, prompt: &str) -> Result<Window> {
    let windows = tmux.list_windows(&session.id)?;
    if windows.is_empty() {
        bail!("session '{}' has no windows", session.name);
    }
    if windows.len() == 1 {
        return Ok(windows.into_iter().next().unwrap());
    }
    require_tty("a window")?;
    let items: Vec<WindowItem> = windows.into_iter().map(WindowItem).collect();
    Ok(Select::new(prompt, items).prompt()?.0)
}

pub fn pick_pane(panes: Vec<Pane>, prompt: &str) -> Result<Pane> {
    if panes.is_empty() {
        bail!("the window has no panes");
    }
    if panes.len() == 1 {
        return Ok(panes.into_iter().next().unwrap());
    }
    require_tty("a pane")?;
    let items: Vec<PaneItem> = panes.into_iter().map(PaneItem).collect();
    Ok(Select::new(prompt, items).prompt()?.0)
}

/// Prompt for an optional name; empty input (or a non-tty stdin) means None.
pub fn optional_name(prompt: &str) -> Result<Option<String>> {
    if !is_interactive() {
        return Ok(None);
    }
    let value = Text::new(prompt)
        .with_help_message("leave empty for a default name")
        .prompt()?;
    let value = value.trim().to_string();
    Ok((!value.is_empty()).then_some(value))
}

/// Prompt for a required value.
pub fn required_name(prompt: &str, what: &str) -> Result<String> {
    require_tty(what)?;
    loop {
        let value = Text::new(prompt).prompt()?;
        let value = value.trim().to_string();
        if !value.is_empty() {
            return Ok(value);
        }
    }
}

pub fn confirm(prompt: &str) -> Result<bool> {
    if !is_interactive() {
        bail!("refusing without confirmation (pass --yes for non-interactive use)");
    }
    Ok(Confirm::new(prompt).with_default(false).prompt()?)
}

fn require_tty(what: &str) -> Result<()> {
    if !is_interactive() {
        bail!("stdin is not a terminal — pass {what} explicitly");
    }
    Ok(())
}
