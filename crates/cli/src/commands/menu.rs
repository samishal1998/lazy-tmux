//! Guided interactive mode: nothing but menus, for when you don't remember
//! any commands at all. Esc goes back one level; Esc at the top level (or
//! "quit") exits.

use anyhow::Result;
use inquire::{InquireError, Select};
use lazytmux_core::{Tmux, TmuxContext};

use crate::commands::{context, panes, sessions, windows};

pub fn run(tmux: &Tmux, ctx: &TmuxContext) -> Result<()> {
    println!("{}", ctx.describe());
    loop {
        let Some(choice) = menu(
            "What do you want to do?",
            &[
                "attach to a session",
                "new session",
                "sessions ...",
                "windows ...",
                "panes ...",
                "show context",
                "quit",
            ],
        )?
        else {
            return Ok(());
        };
        let result = match choice {
            // A successful attach either exec'd tmux (never returns) or
            // switched this client to another session — stop menuing then.
            "attach to a session" => match sessions::attach(tmux, ctx, None) {
                Ok(()) => return Ok(()),
                Err(e) => Err(e),
            },
            "new session" => match sessions::new(tmux, ctx, None, None, false) {
                Ok(()) => return Ok(()),
                Err(e) => Err(e),
            },
            "sessions ..." => sessions_menu(tmux, ctx),
            "windows ..." => windows_menu(tmux, ctx),
            "panes ..." => panes_menu(tmux, ctx),
            "show context" => context::show(ctx),
            _ => return Ok(()),
        };
        if let Err(err) = result {
            if is_cancel(&err) {
                continue; // Esc inside a flow: back to the main menu
            }
            eprintln!("error: {err}");
        }
    }
}

fn sessions_menu(tmux: &Tmux, ctx: &TmuxContext) -> Result<()> {
    loop {
        let Some(choice) = menu(
            "Sessions:",
            &["list", "new", "attach", "rename", "kill", "back"],
        )?
        else {
            return Ok(());
        };
        match choice {
            "list" => sessions::list(tmux, ctx)?,
            "new" => return sessions::new(tmux, ctx, None, None, false),
            "attach" => return sessions::attach(tmux, ctx, None),
            "rename" => sessions::rename(tmux, ctx, None, None)?,
            "kill" => sessions::kill(tmux, None, false)?,
            _ => return Ok(()),
        }
    }
}

fn windows_menu(tmux: &Tmux, ctx: &TmuxContext) -> Result<()> {
    loop {
        let Some(choice) = menu(
            "Windows:",
            &["list", "new", "rename", "select", "kill", "back"],
        )?
        else {
            return Ok(());
        };
        match choice {
            "list" => windows::list(tmux, ctx, None)?,
            "new" => windows::new(tmux, ctx, None, None, None)?,
            "rename" => windows::rename(tmux, ctx, None, None, None)?,
            "select" => windows::select(tmux, ctx, None, None)?,
            "kill" => windows::kill(tmux, ctx, None, None, false)?,
            _ => return Ok(()),
        }
    }
}

fn panes_menu(tmux: &Tmux, ctx: &TmuxContext) -> Result<()> {
    loop {
        let Some(choice) = menu(
            "Panes:",
            &["list", "split below", "split right", "kill", "back"],
        )?
        else {
            return Ok(());
        };
        match choice {
            "list" => panes::list(tmux, ctx, None, None)?,
            "split below" => panes::split(tmux, ctx, None, None, None, false)?,
            "split right" => panes::split(tmux, ctx, None, None, None, true)?,
            "kill" => panes::kill(tmux, ctx, None, None, None, false)?,
            _ => return Ok(()),
        }
    }
}

/// A Select where Esc means "back" (None) instead of an error.
fn menu<'a>(prompt: &str, options: &[&'a str]) -> Result<Option<&'a str>> {
    match Select::new(prompt, options.to_vec()).prompt() {
        Ok(choice) => Ok(Some(choice)),
        Err(InquireError::OperationCanceled) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn is_cancel(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<InquireError>(),
        Some(InquireError::OperationCanceled)
    )
}
