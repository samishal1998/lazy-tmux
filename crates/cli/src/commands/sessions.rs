use std::path::PathBuf;

use anyhow::{bail, Result};
use lazytmux_core::{Session, Tmux, TmuxContext};

use crate::{exec, interactive, output};

pub fn resolve(tmux: &Tmux, name: &str) -> Result<Session> {
    match tmux.session_by_name(name)? {
        Some(s) => Ok(s),
        None => bail!("no session named '{name}' (see `ltm sessions list`)"),
    }
}

/// The session the user is currently in, as a full model.
pub fn current(tmux: &Tmux, ctx: &TmuxContext) -> Result<Option<Session>> {
    let Some((id, _)) = ctx.current_session() else {
        return Ok(None);
    };
    Ok(tmux.list_sessions()?.into_iter().find(|s| s.id == id))
}

pub fn list(tmux: &Tmux, ctx: &TmuxContext) -> Result<()> {
    let sessions = tmux.list_sessions()?;
    if sessions.is_empty() {
        println!("no sessions — create one with `ltm sessions new`");
        return Ok(());
    }
    let current = ctx.current_session().map(|(id, _)| id.to_string());
    let rows: Vec<Vec<String>> = sessions
        .iter()
        .map(|s| {
            let marker = if Some(&s.id) == current.as_ref() { "*" } else { " " };
            vec![
                format!("{marker} {}", s.name),
                s.windows.to_string(),
                if s.is_attached() {
                    format!("yes ({})", s.attached)
                } else {
                    "no".into()
                },
                output::ago(s.created),
                s.path.clone(),
            ]
        })
        .collect();
    output::table(&["  NAME", "WINDOWS", "ATTACHED", "CREATED", "PATH"], &rows);
    Ok(())
}

pub fn new(
    tmux: &Tmux,
    ctx: &TmuxContext,
    name: Option<String>,
    dir: Option<PathBuf>,
    detach: bool,
) -> Result<()> {
    let name = match name {
        Some(n) => Some(n),
        None => interactive::optional_name("Session name:")?,
    };
    if let Some(n) = &name {
        if tmux.session_by_name(n)?.is_some() {
            bail!("session '{n}' already exists — attach with `ltm attach {n}`");
        }
    }
    let session = tmux.new_session(name.as_deref(), dir.as_deref())?;
    println!("created session '{}'", session.name);
    if detach {
        return Ok(());
    }
    exec::attach(tmux, ctx, &session.id)
}

pub fn attach(tmux: &Tmux, ctx: &TmuxContext, name: Option<String>) -> Result<()> {
    let session = match name {
        Some(n) => resolve(tmux, &n)?,
        None => {
            if tmux.list_sessions()?.is_empty() {
                if interactive::is_interactive()
                    && interactive::confirm("No sessions exist. Create one?")?
                {
                    return new(tmux, ctx, None, None, false);
                }
                bail!("no sessions to attach to");
            }
            interactive::pick_session(tmux, "Attach to:")?
        }
    };
    if ctx
        .current_session()
        .is_some_and(|(id, _)| id == session.id)
    {
        println!("already in session '{}'", session.name);
        return Ok(());
    }
    exec::attach(tmux, ctx, &session.id)
}

pub fn kill(tmux: &Tmux, name: Option<String>, yes: bool) -> Result<()> {
    let session = match name {
        Some(n) => resolve(tmux, &n)?,
        None => interactive::pick_session(tmux, "Kill session:")?,
    };
    if !yes && !interactive::confirm(&format!("Kill session '{}'?", session.name))? {
        println!("aborted");
        return Ok(());
    }
    tmux.kill_session(&session.id)?;
    println!("killed session '{}'", session.name);
    Ok(())
}

pub fn detach(tmux: &Tmux, name: Option<String>) -> Result<()> {
    let session = match name {
        Some(n) => resolve(tmux, &n)?,
        None => interactive::pick_session(tmux, "Detach clients from:")?,
    };
    if !session.is_attached() {
        println!("session '{}' has no attached clients", session.name);
        return Ok(());
    }
    tmux.detach_clients(&session.id)?;
    println!("detached {} client(s) from '{}'", session.attached, session.name);
    Ok(())
}

pub fn kill_server(tmux: &Tmux, yes: bool) -> Result<()> {
    let count = tmux.list_sessions()?.len();
    if count == 0 {
        println!("no server running");
        return Ok(());
    }
    if !yes
        && !interactive::confirm(&format!(
            "Kill the tmux server and ALL {count} session(s)?"
        ))?
    {
        println!("aborted");
        return Ok(());
    }
    tmux.kill_server()?;
    println!("killed the tmux server ({count} session(s))");
    Ok(())
}

pub fn rename(
    tmux: &Tmux,
    ctx: &TmuxContext,
    from: Option<String>,
    to: Option<String>,
) -> Result<()> {
    let session = match from {
        Some(n) => resolve(tmux, &n)?,
        None => match current(tmux, ctx)? {
            Some(s) => s,
            None => interactive::pick_session(tmux, "Rename session:")?,
        },
    };
    let to = match to {
        Some(t) => t,
        None => interactive::required_name(
            &format!("New name for '{}':", session.name),
            "the new name",
        )?,
    };
    tmux.rename_session(&session.id, &to)?;
    println!("renamed session '{}' -> '{to}'", session.name);
    Ok(())
}
