use anyhow::{anyhow, Result};
use lazytmux_core::{Pane, ResizeDir, Session, Tmux, TmuxContext, Window};

use crate::commands::windows;
use crate::{interactive, output};

/// Resolve the (session, window) scope: flags first, then the active window
/// of the current/chosen session.
fn resolve_scope(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    window_flag: Option<String>,
) -> Result<(Session, Window)> {
    let session = windows::resolve_session(tmux, ctx, session_flag, "Which session:")?;
    let window = match window_flag {
        Some(t) => windows::resolve_window(tmux, &session, Some(t), "")?,
        None => tmux
            .list_windows(&session.id)?
            .into_iter()
            .find(|w| w.active)
            .ok_or_else(|| anyhow!("session '{}' has no windows", session.name))?,
    };
    Ok((session, window))
}

fn resolve_pane(
    tmux: &Tmux,
    session: &Session,
    window: &Window,
    target: Option<String>,
    prompt: &str,
) -> Result<Pane> {
    let panes = tmux.list_panes(&windows::target_of(session, window))?;
    match target {
        Some(t) => panes
            .into_iter()
            .find(|p| p.index.to_string() == t || p.id == t)
            .ok_or_else(|| {
                anyhow!(
                    "no pane '{t}' in window '{}: {}'",
                    window.index,
                    window.name
                )
            }),
        None => interactive::pick_pane(panes, prompt),
    }
}

pub fn list(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    window_flag: Option<String>,
) -> Result<()> {
    let (session, window) = resolve_scope(tmux, ctx, session_flag, window_flag)?;
    let panes = tmux.list_panes(&windows::target_of(&session, &window))?;
    println!(
        "session '{}', window '{}: {}':",
        session.name, window.index, window.name
    );
    let rows: Vec<Vec<String>> = panes
        .iter()
        .map(|p| {
            let marker = if p.active { "*" } else { " " };
            vec![
                format!("{marker} {}", p.index),
                p.id.clone(),
                p.command.clone(),
                format!("{}x{}", p.width, p.height),
                p.path.clone(),
            ]
        })
        .collect();
    output::table(&["  INDEX", "ID", "COMMAND", "SIZE", "PATH"], &rows);
    Ok(())
}

pub fn split(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    window_flag: Option<String>,
    target: Option<String>,
    right: bool,
) -> Result<()> {
    let (session, window) = resolve_scope(tmux, ctx, session_flag, window_flag)?;
    let pane = match target {
        Some(t) => resolve_pane(tmux, &session, &window, Some(t), "")?,
        None => tmux
            .list_panes(&windows::target_of(&session, &window))?
            .into_iter()
            .find(|p| p.active)
            .ok_or_else(|| anyhow!("window has no panes"))?,
    };
    tmux.split_pane(&pane.id, right)?;
    println!(
        "split pane {} {}",
        pane.id,
        if right { "to the right" } else { "below" }
    );
    Ok(())
}

fn active_pane(tmux: &Tmux, session: &Session, window: &Window) -> Result<Pane> {
    tmux.list_panes(&windows::target_of(session, window))?
        .into_iter()
        .find(|p| p.active)
        .ok_or_else(|| anyhow!("window has no panes"))
}

#[allow(clippy::too_many_arguments)]
pub fn resize(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    window_flag: Option<String>,
    pane: Option<String>,
    dir: ResizeDir,
    amount: u16,
) -> Result<()> {
    let (session, window) = resolve_scope(tmux, ctx, session_flag, window_flag)?;
    let pane = match pane {
        Some(t) => resolve_pane(tmux, &session, &window, Some(t), "")?,
        None => active_pane(tmux, &session, &window)?,
    };
    tmux.resize_pane(&pane.id, dir, amount)?;
    println!("resized pane {} by {amount}", pane.id);
    Ok(())
}

pub fn zoom(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    window_flag: Option<String>,
    target: Option<String>,
) -> Result<()> {
    let (session, window) = resolve_scope(tmux, ctx, session_flag, window_flag)?;
    let pane = match target {
        Some(t) => resolve_pane(tmux, &session, &window, Some(t), "")?,
        None => active_pane(tmux, &session, &window)?,
    };
    tmux.zoom_pane(&pane.id)?;
    println!("toggled zoom on pane {}", pane.id);
    Ok(())
}

pub fn swap(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    window_flag: Option<String>,
    a: Option<String>,
    b: Option<String>,
) -> Result<()> {
    let (session, window) = resolve_scope(tmux, ctx, session_flag, window_flag)?;
    let first = resolve_pane(tmux, &session, &window, a, "Swap pane:")?;
    let second = resolve_pane(tmux, &session, &window, b, "...with pane:")?;
    if first.id == second.id {
        println!("that's the same pane");
        return Ok(());
    }
    tmux.swap_panes(&first.id, &second.id)?;
    println!("swapped panes {} and {}", first.id, second.id);
    Ok(())
}

pub fn rename(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    window_flag: Option<String>,
    target: Option<String>,
    title: Option<String>,
) -> Result<()> {
    let (session, window) = resolve_scope(tmux, ctx, session_flag, window_flag)?;
    let pane = resolve_pane(tmux, &session, &window, target, "Rename pane:")?;
    let title = match title {
        Some(t) => t,
        None => interactive::required_name(
            &format!("New title for pane {}:", pane.id),
            "the new title",
        )?,
    };
    tmux.set_pane_title(&pane.id, &title)?;
    println!("set title of pane {} to '{title}'", pane.id);
    Ok(())
}

pub fn break_out(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    window_flag: Option<String>,
    target: Option<String>,
) -> Result<()> {
    let (session, window) = resolve_scope(tmux, ctx, session_flag, window_flag)?;
    let pane = resolve_pane(tmux, &session, &window, target, "Break out pane:")?;
    tmux.break_pane(&pane.id)?;
    println!("broke pane {} out into its own window", pane.id);
    Ok(())
}

pub fn select(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    window_flag: Option<String>,
    target: Option<String>,
) -> Result<()> {
    let (session, window) = resolve_scope(tmux, ctx, session_flag, window_flag)?;
    let pane = resolve_pane(tmux, &session, &window, target, "Select pane:")?;
    tmux.select_pane(&pane.id)?;
    println!("selected pane {} ({})", pane.id, pane.command);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn join(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    window_flag: Option<String>,
    target: Option<String>,
    to: Option<String>,
    right: bool,
) -> Result<()> {
    let (session, window) = resolve_scope(tmux, ctx, session_flag, window_flag)?;
    let pane = resolve_pane(tmux, &session, &window, target, "Join pane:")?;
    let dest = windows::resolve_window(tmux, &session, to, "...into window:")?;
    if dest.id == window.id {
        return Err(anyhow!("pane {} is already in that window", pane.id));
    }
    tmux.join_pane(&pane.id, &windows::target_of(&session, &dest), right)?;
    println!(
        "joined pane {} into window '{}: {}'",
        pane.id, dest.index, dest.name
    );
    Ok(())
}

pub fn kill(
    tmux: &Tmux,
    ctx: &TmuxContext,
    session_flag: Option<String>,
    window_flag: Option<String>,
    target: Option<String>,
    yes: bool,
) -> Result<()> {
    let (session, window) = resolve_scope(tmux, ctx, session_flag, window_flag)?;
    let pane = resolve_pane(tmux, &session, &window, target, "Kill pane:")?;
    if !yes
        && !interactive::confirm(&format!(
            "Kill pane {} ({}) in window '{}: {}'?",
            pane.id, pane.command, window.index, window.name
        ))?
    {
        println!("aborted");
        return Ok(());
    }
    tmux.kill_pane(&pane.id)?;
    println!("killed pane {}", pane.id);
    Ok(())
}
