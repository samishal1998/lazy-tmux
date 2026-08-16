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
    // Rust ignores SIGPIPE by default, which turns `ltm ... | head` into a
    // panic on a closed pipe. Restore the default: die quietly instead.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
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
        Some(Cmd::Interactive) => commands::menu::run(&tmux, &ctx),
        Some(Cmd::Completions { shell }) => {
            use clap::CommandFactory;
            use std::io::Write;
            let mut buf = Vec::new();
            clap_complete::generate(shell, &mut Cli::command(), "ltm", &mut buf);
            // Ignore write errors so `ltm completions bash | head` doesn't
            // panic on a closed pipe.
            let _ = std::io::stdout().write_all(&buf);
            Ok(())
        }
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
            SessionsCmd::Detach { name } => commands::sessions::detach(&tmux, name),
        },
        Some(Cmd::KillServer { yes }) => commands::sessions::kill_server(&tmux, yes),
        Some(Cmd::Options { cmd }) => commands::options::dispatch(&tmux, &ctx, cmd),
        Some(Cmd::Macros { cmd }) => commands::macros::dispatch(&tmux, &ctx, cmd),
        Some(Cmd::Doctor { fix, conf }) => commands::doctor::run(&tmux, fix, conf),
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
            WindowsCmd::Swap { a, b } => commands::windows::swap(&tmux, &ctx, session, a, b),
            WindowsCmd::Move { target, to } => {
                commands::windows::move_to_session(&tmux, &ctx, session, target, to)
            }
            WindowsCmd::Layout { target } => {
                commands::windows::layout(&tmux, &ctx, session, target)
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
            PanesCmd::Resize {
                direction,
                amount,
                pane,
            } => commands::panes::resize(
                &tmux,
                &ctx,
                session,
                window,
                pane,
                direction.into(),
                amount,
            ),
            PanesCmd::Zoom { target } => {
                commands::panes::zoom(&tmux, &ctx, session, window, target)
            }
            PanesCmd::Swap { a, b } => {
                commands::panes::swap(&tmux, &ctx, session, window, a, b)
            }
            PanesCmd::Rename { target, title } => {
                commands::panes::rename(&tmux, &ctx, session, window, target, title)
            }
            PanesCmd::Break { target } => {
                commands::panes::break_out(&tmux, &ctx, session, window, target)
            }
            PanesCmd::Select { target } => {
                commands::panes::select(&tmux, &ctx, session, window, target)
            }
            PanesCmd::Join { target, to, right } => {
                commands::panes::join(&tmux, &ctx, session, window, target, to, right)
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
