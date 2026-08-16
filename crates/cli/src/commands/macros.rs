use std::collections::BTreeMap;
use std::process::Command;

use anyhow::{bail, Context, Result};
use inquire::Select;
use lazytmux_core::macros::{self, Macro};
use lazytmux_core::{Tmux, TmuxContext};

use crate::args::MacrosCmd;
use crate::{exec, interactive, output};

pub fn dispatch(tmux: &Tmux, ctx: &TmuxContext, cmd: Option<MacrosCmd>) -> Result<()> {
    match cmd.unwrap_or(MacrosCmd::List) {
        MacrosCmd::List => list(),
        MacrosCmd::Run { name } => run(tmux, ctx, name),
        MacrosCmd::Show { name } => show(name),
        MacrosCmd::Edit => edit(),
    }
}

fn list() -> Result<()> {
    let macros = macros::load()?;
    if macros.is_empty() {
        println!(
            "no macros defined — create some with `ltm macros edit` ({})",
            macros::config_path().display()
        );
        return Ok(());
    }
    let rows: Vec<Vec<String>> = macros
        .iter()
        .map(|(name, m)| {
            vec![
                name.clone(),
                m.steps.len().to_string(),
                m.description.clone(),
            ]
        })
        .collect();
    output::table(&["NAME", "STEPS", "DESCRIPTION"], &rows);
    println!("\ndefined in {}", macros::config_path().display());
    Ok(())
}

fn pick(macros: BTreeMap<String, Macro>, prompt: &str) -> Result<(String, Macro)> {
    if macros.is_empty() {
        bail!(
            "no macros defined — create some with `ltm macros edit` ({})",
            macros::config_path().display()
        );
    }
    if macros.len() == 1 {
        return Ok(macros.into_iter().next().unwrap());
    }
    if !interactive::is_interactive() {
        bail!("stdin is not a terminal — pass a macro name explicitly");
    }
    let labels: Vec<String> = macros
        .iter()
        .map(|(name, m)| {
            if m.description.is_empty() {
                name.clone()
            } else {
                format!("{name} — {}", m.description)
            }
        })
        .collect();
    let choice = Select::new(prompt, labels).raw_prompt()?;
    Ok(macros.into_iter().nth(choice.index).unwrap())
}

fn resolve(name: Option<String>, prompt: &str) -> Result<(String, Macro)> {
    let mut macros = macros::load()?;
    match name {
        Some(n) => match macros.remove(&n) {
            Some(m) => Ok((n, m)),
            None => bail!("no macro named '{n}' (see `ltm macros list`)"),
        },
        None => pick(macros, prompt),
    }
}

fn run(tmux: &Tmux, ctx: &TmuxContext, name: Option<String>) -> Result<()> {
    let (name, mac) = resolve(name, "Run macro:")?;
    let attach_to = macros::run(tmux, &name, &mac)?;
    println!("ran macro '{name}' ({} steps)", mac.steps.len());
    if let Some(session) = attach_to {
        let session = crate::commands::sessions::resolve(tmux, &session)?;
        exec::attach(tmux, ctx, &session.id)?;
    }
    Ok(())
}

fn show(name: Option<String>) -> Result<()> {
    let (name, mac) = resolve(name, "Show macro:")?;
    if !mac.description.is_empty() {
        println!("# {}", mac.description);
    }
    println!("{name}:");
    for step in &mac.steps {
        println!("  tmux {step}");
    }
    if let Some(session) = &mac.attach {
        println!("  -> then attach to '{session}'");
    }
    Ok(())
}

fn edit() -> Result<()> {
    let path = macros::config_path();
    if !path.exists() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::write(&path, macros::TEMPLATE)
            .with_context(|| format!("writing {}", path.display()))?;
        println!("created starter file at {}", path.display());
    }
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());
    let status = Command::new(&editor)
        .arg(&path)
        .status()
        .with_context(|| format!("launching editor '{editor}'"))?;
    if !status.success() {
        bail!("editor exited with {status}");
    }
    // Surface syntax errors immediately rather than at next `run`.
    match macros::load() {
        Ok(macros) => println!("{} macro(s) defined", macros.len()),
        Err(e) => eprintln!("warning: {e}"),
    }
    Ok(())
}
