//! Shell completion generation. Scripts are produced from the live clap
//! command tree, so they always match the binary they came from — but an
//! installed file is a snapshot, hence `--install` to refresh it in place.

use std::env;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::CommandFactory;
use clap_complete::Shell;

use crate::args::Cli;

pub fn run(shell: Shell, install: bool) -> Result<()> {
    let mut buf = Vec::new();
    clap_complete::generate(shell, &mut Cli::command(), "ltm", &mut buf);

    if !install {
        // Ignore write errors so `ltm completions bash | head` stays quiet.
        let _ = std::io::stdout().write_all(&buf);
        return Ok(());
    }

    let (path, hint) = install_path(shell)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(&path, &buf).with_context(|| format!("writing {}", path.display()))?;
    println!("installed {shell} completions to {}", path.display());
    if let Some(hint) = hint {
        println!("{hint}");
    }
    println!("(rerun `ltm completions {shell} --install` after upgrading ltm)");
    Ok(())
}

fn install_path(shell: Shell) -> Result<(PathBuf, Option<String>)> {
    let home = PathBuf::from(env::var("HOME").context("$HOME is not set")?);
    let xdg = |var: &str, fallback: PathBuf| {
        env::var(var).map(PathBuf::from).unwrap_or(fallback)
    };
    match shell {
        Shell::Bash => Ok((
            xdg("XDG_DATA_HOME", home.join(".local/share"))
                .join("bash-completion/completions/ltm"),
            Some("takes effect in new shells (needs the bash-completion package)".into()),
        )),
        Shell::Zsh => {
            // oh-my-zsh already has $ZSH_CUSTOM/completions on fpath —
            // use it when present so no .zshrc edits are needed.
            let omz_custom = env::var("ZSH_CUSTOM").map(PathBuf::from).unwrap_or_else(|_| {
                env::var("ZSH")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| home.join(".oh-my-zsh"))
                    .join("custom")
            });
            if omz_custom.is_dir() {
                return Ok((
                    omz_custom.join("completions/_ltm"),
                    Some(
                        "oh-my-zsh detected — takes effect in new shells \
                         (or `rm -f ~/.zcompdump* && exec zsh`)"
                            .into(),
                    ),
                ));
            }
            Ok((
                home.join(".zfunc/_ltm"),
                Some(
                    "make sure ~/.zshrc has, before compinit:\n  fpath=(~/.zfunc $fpath)\n\
                     then start a new shell (or `rm -f ~/.zcompdump && compinit`)"
                        .into(),
                ),
            ))
        }
        Shell::Fish => Ok((
            xdg("XDG_CONFIG_HOME", home.join(".config")).join("fish/completions/ltm.fish"),
            Some("takes effect in new fish shells automatically".into()),
        )),
        other => bail!(
            "no standard completions directory for {other}; redirect stdout instead:\n  \
             ltm completions {other} > <file>"
        ),
    }
}
