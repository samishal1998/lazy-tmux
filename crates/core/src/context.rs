//! Where is this process running, relative to tmux?
//!
//! `$TMUX` alone is not a reliable answer:
//!  - it is *inherited*, so a terminal inside nvim inside tmux still has it
//!    set (that is a *nested descendant*, not a direct pane shell);
//!  - a GUI terminal launched from inside tmux inherits it too, while not
//!    actually displaying inside tmux at all (*inherited env*);
//!  - it can point at a server or pane that no longer exists (*stale env*);
//!  - conversely it can be missing (scrubbed env) while the process really
//!    does live under a tmux pane.
//!
//! So detection cross-checks the environment against the *process tree*:
//! walk our ancestor pids (via /proc) and look for the root process of any
//! pane on the server. Where the ancestry chain and the pane meet — and what
//! sits in between — tells us exactly where we are.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::model::GlobalPane;
use crate::tmux::Tmux;

/// Process names that don't count as "nesting" between us and the pane:
/// running lazy-tmux from a subshell of the pane's shell is still "direct".
const SHELLS: &[&str] = &[
    "sh", "bash", "zsh", "fish", "dash", "ksh", "tcsh", "csh", "nu", "elvish", "xonsh",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    /// Running in the pane's own shell (or a plain subshell of it).
    DirectPane,
    /// Running under the pane, but through intermediate programs —
    /// e.g. a terminal inside nvim. `via` lists them, innermost first.
    NestedDescendant { via: Vec<String> },
    /// `$TMUX` is set and the pane exists, but our process tree does not
    /// lead to it (e.g. a GUI terminal launched from within tmux).
    EnvInherited,
}

#[derive(Debug, Clone)]
pub enum TmuxContext {
    Inside {
        /// Socket path from `$TMUX`, if the env was present.
        socket: Option<PathBuf>,
        session_id: String,
        session_name: String,
        pane_id: String,
        location: Location,
        /// Whether `$TMUX` was actually set (it may not be, if the
        /// environment was scrubbed — the process tree still gave us away).
        env_present: bool,
    },
    /// `$TMUX` is set but the server (or our pane) is gone.
    StaleEnv {
        socket: PathBuf,
    },
    Outside {
        server_running: bool,
    },
}

impl TmuxContext {
    /// The tmux handle appropriate for this context (honours the socket
    /// `$TMUX` pointed at, falling back to the default server).
    pub fn tmux(&self) -> Tmux {
        match self {
            TmuxContext::Inside {
                socket: Some(sock), ..
            } => Tmux::with_socket(sock),
            _ => Tmux::new(),
        }
    }

    pub fn is_inside(&self) -> bool {
        matches!(self, TmuxContext::Inside { .. })
    }

    /// Session we are in, if any (id, name).
    pub fn current_session(&self) -> Option<(&str, &str)> {
        match self {
            TmuxContext::Inside {
                session_id,
                session_name,
                ..
            } => Some((session_id, session_name)),
            _ => None,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            TmuxContext::Inside {
                session_name,
                pane_id,
                location,
                env_present,
                ..
            } => {
                let mut s = match location {
                    Location::DirectPane => {
                        format!("inside tmux: session '{session_name}', pane {pane_id} (direct)")
                    }
                    Location::NestedDescendant { via } => format!(
                        "inside tmux: session '{session_name}', pane {pane_id} (nested via {})",
                        via.join(" > ")
                    ),
                    Location::EnvInherited => format!(
                        "tmux environment inherited from session '{session_name}' \
                         (this terminal is not displayed inside tmux)"
                    ),
                };
                if !env_present {
                    s.push_str(" [$TMUX not set — detected via process tree]");
                }
                s
            }
            TmuxContext::StaleEnv { socket } => format!(
                "stale $TMUX pointing at {} — server or pane is gone",
                socket.display()
            ),
            TmuxContext::Outside { server_running } => {
                if *server_running {
                    "outside tmux (a server is running)".into()
                } else {
                    "outside tmux (no server running)".into()
                }
            }
        }
    }
}

/// Detect the current tmux context. Never fails: degrades to `Outside`.
pub fn detect() -> TmuxContext {
    let env_tmux = env::var("TMUX").ok().filter(|v| !v.is_empty());
    let chain = ancestry();

    if let Some(val) = env_tmux {
        let socket = PathBuf::from(val.split(',').next().unwrap_or_default());
        let tmux = Tmux::with_socket(&socket);
        let panes = match tmux.global_panes() {
            Ok(p) if !p.is_empty() => p,
            _ => return TmuxContext::StaleEnv { socket },
        };

        if let Some((pane, via)) = locate_in_chain(&chain, &panes) {
            return inside(Some(socket), pane, direct_or_nested(via), true);
        }

        // Ancestry did not reach a pane. If we could not read the process
        // tree at all (non-Linux, restricted /proc), trust the env and
        // assume direct; otherwise the env was merely inherited.
        let env_pane = env::var("TMUX_PANE").ok().unwrap_or_default();
        let pane = panes.iter().find(|p| p.pane_id == env_pane).cloned();
        return match pane {
            Some(pane) if chain.len() <= 1 => {
                inside(Some(socket), pane, Location::DirectPane, true)
            }
            Some(pane) => inside(Some(socket), pane, Location::EnvInherited, true),
            None => TmuxContext::StaleEnv { socket },
        };
    }

    // No $TMUX: we may still be under a pane of the default server.
    let tmux = Tmux::new();
    match tmux.global_panes() {
        Ok(panes) if !panes.is_empty() => match locate_in_chain(&chain, &panes) {
            Some((pane, via)) => inside(None, pane, direct_or_nested(via), false),
            None => TmuxContext::Outside {
                server_running: true,
            },
        },
        Ok(_) => TmuxContext::Outside {
            server_running: true,
        },
        Err(_) => TmuxContext::Outside {
            server_running: false,
        },
    }
}

fn inside(
    socket: Option<PathBuf>,
    pane: GlobalPane,
    location: Location,
    env_present: bool,
) -> TmuxContext {
    TmuxContext::Inside {
        socket,
        session_id: pane.session_id,
        session_name: pane.session_name,
        pane_id: pane.pane_id,
        location,
        env_present,
    }
}

fn direct_or_nested(via: Vec<String>) -> Location {
    if via.is_empty() {
        Location::DirectPane
    } else {
        Location::NestedDescendant { via }
    }
}

/// Find the nearest ancestor that is a pane root process. Returns that pane
/// and the non-shell programs sitting between us and it (innermost first).
fn locate_in_chain(
    chain: &[(u32, String)],
    panes: &[GlobalPane],
) -> Option<(GlobalPane, Vec<String>)> {
    let by_pid: HashMap<u32, &GlobalPane> = panes.iter().map(|p| (p.pid, p)).collect();
    for (i, (pid, _)) in chain.iter().enumerate() {
        if let Some(pane) = by_pid.get(pid) {
            let mut via: Vec<String> = Vec::new();
            for (_, comm) in chain.get(1..i).unwrap_or(&[]) {
                if SHELLS.contains(&comm.as_str()) {
                    continue;
                }
                // Collapse runs of the same program (nvim spawns an
                // embedded nvim child, etc.).
                if via.last().map(String::as_str) != Some(comm.as_str()) {
                    via.push(comm.clone());
                }
            }
            return Some(((*pane).clone(), via));
        }
    }
    None
}

/// Our ancestor chain, self first: [(pid, comm), ...]. Uses /proc, so on
/// non-Linux platforms this returns only our own pid.
pub fn ancestry() -> Vec<(u32, String)> {
    let mut chain = Vec::new();
    let mut pid = std::process::id();
    for _ in 0..128 {
        let Some((ppid, comm)) = proc_stat(pid) else {
            if chain.is_empty() {
                chain.push((pid, String::new()));
            }
            break;
        };
        chain.push((pid, comm));
        if ppid == 0 {
            break;
        }
        pid = ppid;
    }
    chain
}

/// Parse `/proc/<pid>/stat` into (ppid, comm). The comm field is enclosed in
/// parentheses and may itself contain spaces or parentheses, so split on the
/// *last* closing paren.
fn proc_stat(pid: u32) -> Option<(u32, String)> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let comm = stat.get(open + 1..close)?.to_string();
    let rest: Vec<&str> = stat.get(close + 1..)?.split_whitespace().collect();
    // fields after comm: state, ppid, ...
    let ppid = rest.get(1)?.parse().ok()?;
    Some((ppid, comm))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(pid: u32) -> GlobalPane {
        GlobalPane {
            pid,
            pane_id: "%1".into(),
            session_id: "$1".into(),
            session_name: "main".into(),
            window_index: 0,
        }
    }

    #[test]
    fn direct_when_parent_is_pane_shell() {
        let chain = vec![(100, "ltm".into()), (50, "zsh".into())];
        let (_, via) = locate_in_chain(&chain, &[pane(50)]).unwrap();
        assert!(via.is_empty());
    }

    #[test]
    fn subshell_still_counts_as_direct() {
        let chain = vec![(100, "ltm".into()), (60, "bash".into()), (50, "zsh".into())];
        let (_, via) = locate_in_chain(&chain, &[pane(50)]).unwrap();
        assert!(via.is_empty());
    }

    #[test]
    fn nested_through_nvim() {
        let chain = vec![
            (100, "ltm".into()),
            (70, "zsh".into()),
            (60, "nvim".into()),
            (50, "zsh".into()),
        ];
        let (_, via) = locate_in_chain(&chain, &[pane(50)]).unwrap();
        assert_eq!(via, vec!["nvim".to_string()]);
    }

    #[test]
    fn not_found_outside() {
        let chain = vec![(100, "ltm".into()), (50, "zsh".into())];
        assert!(locate_in_chain(&chain, &[pane(999)]).is_none());
    }
}
