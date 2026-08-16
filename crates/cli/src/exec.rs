//! Execute an [`AttachPlan`]: switch the client in place, or replace this
//! process with `tmux attach`.

use std::os::unix::process::CommandExt;

use anyhow::Result;
use lazytmux_core::{plan_attach, AttachPlan, Error, Tmux, TmuxContext};

pub fn attach(tmux: &Tmux, ctx: &TmuxContext, target: &str) -> Result<()> {
    match plan_attach(tmux, ctx, target) {
        AttachPlan::SwitchClient { target, client_tty } => {
            match tmux.switch_client(&target, client_tty.as_deref()) {
                // We look like a tmux client but aren't one (e.g. detached
                // session driven remotely) — attach fresh instead.
                Err(Error::Tmux(msg)) if msg.contains("no current client") => {
                    exec_attach(tmux, &target, true)
                }
                other => Ok(other?),
            }
        }
        AttachPlan::Exec {
            target,
            clear_tmux_env,
        } => exec_attach(tmux, &target, clear_tmux_env),
    }
}

fn exec_attach(tmux: &Tmux, target: &str, clear_tmux_env: bool) -> Result<()> {
    let mut cmd = tmux.command();
    cmd.args(["attach-session", "-t", target]);
    if clear_tmux_env {
        cmd.env_remove("TMUX");
        cmd.env_remove("TMUX_PANE");
    }
    // On success this never returns.
    Err(cmd.exec().into())
}
