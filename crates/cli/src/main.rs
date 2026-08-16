mod args;
mod commands;
mod exec;
mod interactive;
mod output;

use std::io::IsTerminal;

use anyhow::{bail, Result};
use clap::Parser;
use inquire::InquireError;
use lazytmux_core::{Tmux, TmuxContext};

use args::{Cli, Cmd, PanesCmd, SessionsCmd, WindowsCmd};

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        if matches!(
            err.downcast_ref::<InquireError>(),
            Some(InquireError::OperationCanceled | InquireError::OperationInterrupted)
        ) {
            std::process::exit(130);
        }
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let ctx = lazytmux_core::detect();
    let tmux = ctx.tmux();

    match cli.command {
        None | Some(Cmd::Ui) => ui(&tmux, &ctx),
        Some(Cmd::Context) => commands::context::show(&ctx),
        Some(Cmd::Attach { name }) => commands::sessions::attach(&tmux, &ctx, name),
        Some(Cmd::Sessions { cmd }) => match cmd.unwrap_or(SessionsCmd::List) {
            SessionsCmd::List => commands::sessions::list(&tmux, &ctx),
            SessionsCmd::New { name, dir, detach } => {
                commands::sessions::new(&tmux, &ctx, name, dir, detach)
            }
            SessionsCmd::Attach { name } => commands::sessions::attach(&tmux, &ctx, name),
            SessionsCmd::Kill { name, yes } => commands::sessions::kill(&tmux, name, yes),
            SessionsCmd::Rename { from, to } => {
                commands::sessions::rename(&tmux, &ctx, from, to)
            }
        },
        Some(Cmd::Windows { session, cmd }) => match cmd.unwrap_or(WindowsCmd::List) {
            WindowsCmd::List => commands::windows::list(&tmux, &ctx, session),
            WindowsCmd::New { name, dir } => {
                commands::windows::new(&tmux, &ctx, session, name, dir)
            }
            WindowsCmd::Kill { target, yes } => {
                commands::windows::kill(&tmux, &ctx, session, target, yes)
            }
            WindowsCmd::Rename { target, to } => {
                commands::windows::rename(&tmux, &ctx, session, target, to)
            }
            WindowsCmd::Select { target } => {
                commands::windows::select(&tmux, &ctx, session, target)
            }
        },
        Some(Cmd::Panes {
            session,
            window,
            cmd,
        }) => match cmd.unwrap_or(PanesCmd::List) {
            PanesCmd::List => commands::panes::list(&tmux, &ctx, session, window),
            PanesCmd::Split { target, right } => {
                commands::panes::split(&tmux, &ctx, session, window, target, right)
            }
            PanesCmd::Kill { target, yes } => {
                commands::panes::kill(&tmux, &ctx, session, window, target, yes)
            }
        },
    }
}

fn ui(tmux: &Tmux, ctx: &TmuxContext) -> Result<()> {
    if !std::io::stdout().is_terminal() {
        bail!("the TUI needs a terminal (try `ltm sessions list` instead)");
    }
    if let Some(target) = lazytmux_tui::run(tmux.clone(), ctx.clone())? {
        exec::attach(tmux, ctx, &target)?;
    }
    Ok(())
}
