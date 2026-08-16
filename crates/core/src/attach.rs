//! Decide *how* to get the user's terminal onto a target session, based on
//! the detected context. Executing the plan (possibly `exec`-ing tmux) is
//! left to the frontends so the TUI can restore the terminal first.

use crate::context::{Location, TmuxContext};
use crate::tmux::Tmux;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachPlan {
    /// We are a tmux client already: switch it in place.
    SwitchClient {
        target: String,
        /// Explicit client tty, needed when `$TMUX` is absent so tmux
        /// cannot infer which client we mean.
        client_tty: Option<String>,
    },
    /// Replace this process with `tmux attach -t target`.
    Exec {
        target: String,
        /// Strip the inherited `$TMUX` first, otherwise tmux refuses to
        /// nest ("sessions should be nested with care").
        clear_tmux_env: bool,
    },
}

pub fn plan_attach(tmux: &Tmux, ctx: &TmuxContext, target: &str) -> AttachPlan {
    match ctx {
        TmuxContext::Inside {
            location,
            session_name,
            env_present,
            ..
        } => match location {
            Location::DirectPane | Location::NestedDescendant { .. } => {
                // Without $TMUX, tmux can't tell which client issued the
                // command — point it at a client attached to our session.
                let client_tty = if *env_present {
                    None
                } else {
                    tmux.clients_of_session(session_name)
                        .ok()
                        .and_then(|c| c.into_iter().next())
                };
                if !env_present && client_tty.is_none() {
                    // Can't identify our client; fall back to a fresh attach.
                    return AttachPlan::Exec {
                        target: target.into(),
                        clear_tmux_env: true,
                    };
                }
                AttachPlan::SwitchClient {
                    target: target.into(),
                    client_tty,
                }
            }
            // The env came along for the ride, but this terminal is not a
            // tmux client — attach normally, shedding the stale env.
            Location::EnvInherited => AttachPlan::Exec {
                target: target.into(),
                clear_tmux_env: true,
            },
        },
        TmuxContext::StaleEnv { .. } => AttachPlan::Exec {
            target: target.into(),
            clear_tmux_env: true,
        },
        TmuxContext::Outside { .. } => AttachPlan::Exec {
            target: target.into(),
            clear_tmux_env: false,
        },
    }
}
