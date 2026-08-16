use anyhow::Result;
use lazytmux_core::{Tmux, TmuxContext};

use crate::args::{OnOff, OptionsCmd};
use crate::commands::windows;
use crate::output;

pub fn dispatch(tmux: &Tmux, ctx: &TmuxContext, cmd: Option<OptionsCmd>) -> Result<()> {
    match cmd.unwrap_or(OptionsCmd::List) {
        OptionsCmd::List => list(tmux, ctx),
        OptionsCmd::Mouse { state } => global_flag(tmux, "mouse", state),
        OptionsCmd::Status { state } => global_flag(tmux, "status", state),
        OptionsCmd::Sync { state } => sync(tmux, ctx, state),
        OptionsCmd::Get { name } => get(tmux, &name),
        OptionsCmd::Set {
            name,
            value,
            window,
        } => set(tmux, ctx, &name, &value, window),
    }
}

fn list(tmux: &Tmux, ctx: &TmuxContext) -> Result<()> {
    let flag = |on: bool| if on { "on" } else { "off" }.to_string();
    let mut rows = vec![
        vec![
            "mouse".into(),
            flag(tmux.global_flag("mouse")?),
            "scroll, click panes, drag borders".into(),
        ],
        vec![
            "status".into(),
            flag(tmux.global_flag("status")?),
            "the status bar".into(),
        ],
    ];
    if let Ok(session) = windows::resolve_session(tmux, ctx, None, "") {
        if let Ok(window) = windows::active_window(tmux, &session) {
            rows.push(vec![
                "sync".into(),
                flag(tmux.window_flag(
                    &windows::target_of(&session, &window),
                    "synchronize-panes",
                )?),
                format!("type into all panes of window '{}' at once", window.name),
            ]);
        }
    }
    output::table(&["OPTION", "VALUE", "WHAT IT DOES"], &rows);
    println!("\ntoggle with `ltm options <name>`; health checks: `ltm doctor`");
    Ok(())
}

fn global_flag(tmux: &Tmux, name: &str, state: Option<OnOff>) -> Result<()> {
    let new = match state {
        Some(v) => {
            tmux.set_global_option(name, v.as_str())?;
            matches!(v, OnOff::On)
        }
        None => tmux.toggle_global_flag(name)?,
    };
    println!("{name}: {}", if new { "on" } else { "off" });
    Ok(())
}

fn sync(tmux: &Tmux, ctx: &TmuxContext, state: Option<OnOff>) -> Result<()> {
    let session = windows::resolve_session(tmux, ctx, None, "Sync panes in which session:")?;
    let window = windows::active_window(tmux, &session)?;
    let target = windows::target_of(&session, &window);
    let new = match state {
        Some(v) => {
            tmux.set_window_option(&target, "synchronize-panes", v.as_str())?;
            matches!(v, OnOff::On)
        }
        None => tmux.toggle_window_flag(&target, "synchronize-panes")?,
    };
    println!(
        "synchronize-panes on window '{}: {}': {}",
        window.index,
        window.name,
        if new { "on" } else { "off" }
    );
    Ok(())
}

fn get(tmux: &Tmux, name: &str) -> Result<()> {
    let global = tmux.show_global_option(name).unwrap_or_default();
    let server = tmux.show_server_option(name).unwrap_or_default();
    match (global.is_empty(), server.is_empty()) {
        (false, _) => println!("{global}"),
        (true, false) => println!("{server}"),
        _ => println!("(unset)"),
    }
    Ok(())
}

fn set(tmux: &Tmux, ctx: &TmuxContext, name: &str, value: &str, window: bool) -> Result<()> {
    if window {
        let session = windows::resolve_session(tmux, ctx, None, "Set on which session:")?;
        let win = windows::active_window(tmux, &session)?;
        tmux.set_window_option(&windows::target_of(&session, &win), name, value)?;
        println!("{name} = {value} (window '{}')", win.name);
    } else {
        tmux.set_global_option(name, value)?;
        println!("{name} = {value} (global)");
    }
    Ok(())
}
