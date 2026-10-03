use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use lazytmux_core::{extract, macros, Tmux};

use crate::interactive;

const HEADER: &str = "\
# Extracted by `ltm extract`. Each [macros.<name>] recreates one session.
# Run:   LAZY_TMUX_MACROS=<this file> ltm macros run <name>
# Keep:  merge into ~/.config/lazy-tmux/macros.toml (or copy it there).
# Running a macro fails if a session with that name already exists.

";

pub fn run(
    tmux: &Tmux,
    names: Vec<String>,
    all: bool,
    with_process: bool,
    output: Option<PathBuf>,
    force: bool,
) -> Result<()> {
    if with_process && !cfg!(target_os = "linux") {
        bail!("--with-running-process reads /proc, so it needs Linux");
    }
    if let Some(path) = &output {
        if path.exists() && !force {
            bail!("{} exists (use --force to overwrite)", path.display());
        }
    }

    let sessions = tmux.list_sessions()?;
    if sessions.is_empty() {
        bail!("no tmux sessions to extract");
    }
    let chosen = if all {
        sessions
    } else if !names.is_empty() {
        names
            .iter()
            .map(|n| {
                sessions
                    .iter()
                    .find(|s| &s.name == n)
                    .cloned()
                    .with_context(|| format!("no session named '{n}' (see `ltm sessions list`)"))
            })
            .collect::<Result<Vec<_>>>()?
    } else if sessions.len() == 1 {
        sessions
    } else if interactive::is_interactive() {
        interactive::pick_sessions(sessions, "Extract which sessions?")?
    } else {
        bail!("pass session names or --all (stdin is not a terminal)");
    };
    if chosen.is_empty() {
        bail!("no sessions selected");
    }

    let mut out = BTreeMap::new();
    for s in &chosen {
        let snap = extract::snapshot(tmux, &s.id, &s.name, with_process)?;
        out.insert(s.name.clone(), extract::to_macro(&snap));
    }
    let text = format!("{HEADER}{}", macros::to_toml(&out)?);

    match output {
        Some(path) => {
            std::fs::write(&path, &text).with_context(|| format!("writing {}", path.display()))?;
            eprintln!("wrote {} macro(s) to {}", out.len(), path.display());
        }
        None => print!("{text}"),
    }
    Ok(())
}
