//! Snapshot live sessions into macros (`ltm extract`).
//!
//! A snapshot becomes the same macro TOML `ltm macros run` already reads, so
//! it can be kept, versioned, or copied to another machine. Generated steps
//! never mention window/pane *indexes*, which differ between machines
//! (`base-index`, `pane-base-index`): the window being built is always
//! `<session>:$` (the last one), and the pane being built is always that
//! window's active pane -- a `split-window` without `-d` makes the new pane
//! active, so `send-keys` right after it reaches the right pane.

use std::fs;

use crate::context::{proc_stat, SHELLS};
use crate::error::Result;
use crate::macros::Macro;
use crate::model::SEP_STR;
use crate::tmux::Tmux;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneSnap {
    pub path: String,
    /// What the pane was running in the foreground, when asked for.
    pub command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowSnap {
    /// `None` when the window is auto-named: restoring the name would
    /// freeze it, so it is left for tmux to pick again.
    pub name: Option<String>,
    pub layout: String,
    pub panes: Vec<PaneSnap>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnap {
    pub name: String,
    pub windows: Vec<WindowSnap>,
}

/// Read one session's windows and panes. `with_process` also records each
/// pane's foreground command (Linux only: it reads /proc).
pub fn snapshot(tmux: &Tmux, id: &str, name: &str, with_process: bool) -> Result<SessionSnap> {
    let format = [
        "#{window_index}",
        "#{window_name}",
        "#{automatic-rename}",
        "#{window_layout}",
        "#{pane_pid}",
        "#{pane_current_path}",
    ]
    .join(SEP_STR);
    let out = tmux.run(["list-panes", "-s", "-t", id, "-F", &format])?;

    let mut windows: Vec<WindowSnap> = Vec::new();
    let mut last_index = String::new();
    for line in out.lines() {
        let f: Vec<&str> = line.split(crate::model::SEP).collect();
        if f.len() != 6 {
            return Err(crate::Error::Parse(format!("pane line: {line:?}")));
        }
        if f[0] != last_index {
            last_index = f[0].to_string();
            windows.push(WindowSnap {
                name: (f[2] == "0").then(|| f[1].to_string()),
                layout: f[3].to_string(),
                panes: Vec::new(),
            });
        }
        let command = if with_process {
            f[4].parse().ok().and_then(foreground_command)
        } else {
            None
        };
        windows.last_mut().unwrap().panes.push(PaneSnap {
            path: tilde(f[5]),
            command,
        });
    }
    Ok(SessionSnap {
        name: name.to_string(),
        windows,
    })
}

/// `$HOME/x` -> `~/x`, so the macro works on a machine with another home.
fn tilde(path: &str) -> String {
    let Ok(home) = std::env::var("HOME") else {
        return path.into();
    };
    match path.strip_prefix(&home) {
        Some("") => "~/".into(),
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => path.into(),
    }
}

/// The command running in a pane's foreground, as a shell-quoted line.
/// None when the pane is just an idle shell.
fn foreground_command(pane_pid: u32) -> Option<String> {
    let root = proc_stat(pane_pid)?;
    let target = if SHELLS.contains(&root.comm.as_str()) {
        // An idle shell is its own terminal's foreground group; a running
        // job's group leader is what `tpgid` points at.
        if root.tpgid <= 0 || root.tpgid as u32 == root.pgrp {
            return None;
        }
        root.tpgid as u32
    } else {
        pane_pid // the pane *is* the program (`new-window htop`)
    };
    // The pane running `ltm extract` itself (also `ltm extract | tee`, which
    // shares a process group): restoring that would re-run the extraction.
    if proc_stat(target)?.pgrp == proc_stat(std::process::id())?.pgrp {
        return None;
    }
    let raw = fs::read(format!("/proc/{target}/cmdline")).ok()?;
    let argv: Vec<String> = raw
        .split(|b| *b == 0)
        .filter(|a| !a.is_empty())
        .map(|a| String::from_utf8_lossy(a).into_owned())
        .collect();
    // A forked subshell keeps its parent's argv (`-bash`): not a command.
    let argv0 = argv.first()?.trim_start_matches('-');
    if SHELLS.contains(&argv0.rsplit('/').next()?) {
        return None;
    }
    shlex::try_join(argv.iter().map(String::as_str)).ok()
}

/// A name (not a path) that the macro runner must not home-expand.
fn literal(name: &str) -> String {
    if name.starts_with("~/") {
        format!("\\{name}")
    } else {
        name.to_string()
    }
}

/// One macro step from its argv, quoted so `shlex::split` gives it back.
fn step(args: &[&str]) -> String {
    shlex::try_join(args.iter().copied()).unwrap_or_else(|_| args.join(" "))
}

/// Build the macro that recreates a snapshot.
pub fn to_macro(snap: &SessionSnap) -> Macro {
    let sess = snap.name.as_str();
    let sess_next = format!("{sess}:"); // appends a window
    let last = format!("{sess}:$"); // the window just created
    let mut steps = Vec::new();

    for (i, w) in snap.windows.iter().enumerate() {
        let Some(first) = w.panes.first() else {
            continue;
        };
        let mut args: Vec<&str> = if i == 0 {
            // Big enough that a many-pane window can be split before
            // select-layout sets the real size; tmux resizes on attach.
            vec!["new-session", "-d", "-x", "300", "-y", "100", "-s", sess]
        } else {
            vec!["new-window", "-d", "-t", &sess_next]
        };
        let wname = w.name.as_deref().map(literal);
        if let Some(name) = &wname {
            args.extend(["-n", name]);
        }
        args.extend(["-c", &first.path]);
        steps.push(step(&args));
        push_command(&mut steps, &last, first);

        for pane in &w.panes[1..] {
            steps.push(step(&["split-window", "-t", &last, "-c", &pane.path]));
            // Each split halves the active pane; rebalancing keeps room for
            // the next one (otherwise ~7 panes fail: "no space for new pane").
            steps.push(step(&["select-layout", "-t", &last, "tiled"]));
            push_command(&mut steps, &last, pane);
        }
        if w.panes.len() > 1 {
            steps.push(step(&["select-layout", "-t", &last, &w.layout]));
        }
    }
    Macro {
        description: format!("session '{sess}', extracted by `ltm extract`"),
        steps,
        attach: Some(snap.name.clone()),
    }
}

fn push_command(steps: &mut Vec<String>, target: &str, pane: &PaneSnap) {
    // ponytail: typed-ahead into a pane whose shell is still starting; a
    // slow rc file can eat it. Add a delay step if that ever bites.
    if let Some(cmd) = &pane.command {
        // -l: the text is typed literally (never read as key names or flags).
        steps.push(step(&["send-keys", "-t", target, "-l", "--", cmd]));
        steps.push(step(&["send-keys", "-t", target, "Enter"]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(path: &str, command: Option<&str>) -> PaneSnap {
        PaneSnap {
            path: path.into(),
            command: command.map(Into::into),
        }
    }

    fn sample() -> SessionSnap {
        SessionSnap {
            name: "dev".into(),
            windows: vec![
                WindowSnap {
                    name: Some("code".into()),
                    layout: "abcd,200x50,0,0{100x50,0,0,1,99x50,101,0,2}".into(),
                    panes: vec![
                        pane("~/my project", Some("nvim 'a b.rs'")),
                        pane("/srv", None),
                    ],
                },
                WindowSnap {
                    name: None,
                    layout: "x".into(),
                    panes: vec![pane("~/", None)],
                },
            ],
        }
    }

    /// Every emitted step must survive the macro runner's `shlex::split`
    /// unchanged -- paths with spaces and commands with quotes included.
    #[test]
    fn steps_round_trip_through_shlex() {
        let m = to_macro(&sample());
        let split: Vec<Vec<String>> = m.steps.iter().map(|s| shlex::split(s).unwrap()).collect();
        assert_eq!(
            split[0],
            [
                "new-session",
                "-d",
                "-x",
                "300",
                "-y",
                "100",
                "-s",
                "dev",
                "-n",
                "code",
                "-c",
                "~/my project"
            ]
        );
        assert_eq!(
            split[1],
            ["send-keys", "-t", "dev:$", "-l", "--", "nvim 'a b.rs'"]
        );
        assert_eq!(split[2], ["send-keys", "-t", "dev:$", "Enter"]);
        assert_eq!(split[3], ["split-window", "-t", "dev:$", "-c", "/srv"]);
        assert_eq!(split[4], ["select-layout", "-t", "dev:$", "tiled"]);
        assert_eq!(split[5][0], "select-layout");
        assert_eq!(split[5][3], "abcd,200x50,0,0{100x50,0,0,1,99x50,101,0,2}");
        // Auto-named second window: no -n, appended after the first.
        assert_eq!(split[6], ["new-window", "-d", "-t", "dev:", "-c", "~/"]);
        assert_eq!(split.len(), 7);
        assert_eq!(m.attach.as_deref(), Some("dev"));
    }

    /// A window literally named "~/notes" must come back as that name, not
    /// as $HOME/notes (the runner expands leading `~/` in every argument).
    #[test]
    fn tilde_names_survive_the_runner() {
        let mut snap = sample();
        snap.windows[0].name = Some("~/notes".into());
        let step = to_macro(&snap).steps[0].clone();
        let args: Vec<String> = shlex::split(&step)
            .unwrap()
            .into_iter()
            .map(crate::macros::expand_home)
            .collect();
        let n = args.iter().position(|a| a == "-n").unwrap();
        assert_eq!(args[n + 1], "~/notes");
    }

    #[test]
    fn many_panes_rebalance_after_every_split() {
        let mut snap = sample();
        snap.windows[0].panes = (0..8).map(|_| pane("/tmp", None)).collect();
        let steps = to_macro(&snap).steps;
        let tiled = steps.iter().filter(|s| s.ends_with(" tiled")).count();
        assert_eq!(tiled, 7); // one per split, so the next split has room
    }

    #[test]
    fn tilde_abbreviates_home_only_at_a_path_boundary() {
        std::env::set_var("HOME", "/home/u");
        assert_eq!(tilde("/home/u"), "~/");
        assert_eq!(tilde("/home/u/p"), "~/p");
        assert_eq!(tilde("/home/user2"), "/home/user2");
    }
}
