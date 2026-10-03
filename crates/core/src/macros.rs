//! User-defined macros: named sequences of tmux commands, declared in
//! `~/.config/lazy-tmux/macros.toml` and run with `ltm macros run <name>`.
//!
//! ```toml
//! [macros.dev]
//! description = "Editor + server layout for my project"
//! steps = [
//!   "new-session -d -s dev -c ~/projects/app",
//!   "rename-window -t dev: editor",
//!   "send-keys -t dev: 'nvim .' Enter",
//!   "split-window -h -t dev: -c ~/projects/app",
//!   "new-window -t dev -n server -c ~/projects/app",
//!   "send-keys -t dev:server 'npm run dev' Enter",
//! ]
//! attach = "dev"   # optional: session to attach to afterwards
//! ```
//!
//! Steps are tmux commands (no `tmux` prefix), split shell-style so quoted
//! arguments survive. `~/` at the start of an argument expands to $HOME.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::tmux::Tmux;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MacroFile {
    #[serde(default)]
    pub macros: BTreeMap<String, Macro>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Macro {
    #[serde(default)]
    pub description: String,
    pub steps: Vec<String>,
    /// Session to attach to (or switch to) after the steps have run.
    #[serde(default)]
    pub attach: Option<String>,
}

/// `$LAZY_TMUX_MACROS`, else `$XDG_CONFIG_HOME/lazy-tmux/macros.toml`,
/// else `~/.config/lazy-tmux/macros.toml`.
pub fn config_path() -> PathBuf {
    if let Ok(path) = env::var("LAZY_TMUX_MACROS") {
        return PathBuf::from(path);
    }
    let config_dir = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env::var("HOME").unwrap_or_default()).join(".config"));
    config_dir.join("lazy-tmux").join("macros.toml")
}

pub fn load() -> Result<BTreeMap<String, Macro>> {
    let path = config_path();
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(e.into()),
    };
    let file: MacroFile =
        toml::from_str(&text).map_err(|e| Error::Parse(format!("{}: {e}", path.display())))?;
    Ok(file.macros)
}

/// Run a macro's steps in order. Returns the session to attach to, if the
/// macro asks for one. Fails fast on the first failing step, reporting it.
pub fn run(tmux: &Tmux, name: &str, mac: &Macro) -> Result<Option<String>> {
    for (i, step) in mac.steps.iter().enumerate() {
        let args = shlex::split(step).ok_or_else(|| {
            Error::Parse(format!(
                "macro '{name}' step {}: unbalanced quotes: {step:?}",
                i + 1
            ))
        })?;
        if args.is_empty() {
            continue;
        }
        let args: Vec<String> = args.into_iter().map(expand_home).collect();
        tmux.run(&args)
            .map_err(|e| Error::Tmux(format!("macro '{name}' step {} ({step}): {e}", i + 1)))?;
    }
    Ok(mac.attach.clone())
}

fn expand_home(arg: String) -> String {
    if let Some(rest) = arg.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    arg
}

/// Starter file written by `ltm macros edit` when none exists yet.
pub const TEMPLATE: &str = r#"# lazy-tmux macros — run with `ltm macros run <name>`
# Steps are tmux commands (without the `tmux` prefix), executed in order.
# `attach = "<session>"` attaches/switches there when the steps are done.

# [macros.dev]
# description = "Editor + server layout"
# steps = [
#   "new-session -d -s dev -c ~/projects/app",
#   "rename-window -t dev: editor",
#   "send-keys -t dev: 'nvim .' Enter",
#   "split-window -h -t dev:",
#   "new-window -t dev -n server",
#   "send-keys -t dev:server 'npm run dev' Enter",
# ]
# attach = "dev"
"#;
