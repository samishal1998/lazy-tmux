use anyhow::Result;
use lazytmux_core::context::ancestry;
use lazytmux_core::TmuxContext;

pub fn show(ctx: &TmuxContext) -> Result<()> {
    println!("where : {}", ctx.describe());
    if let TmuxContext::Inside {
        socket,
        session_id,
        session_name,
        pane_id,
        env_present,
        ..
    } = ctx
    {
        println!("session: {session_name} ({session_id})");
        println!("pane   : {pane_id}");
        if let Some(socket) = socket {
            println!("socket : {}", socket.display());
        }
        println!("$TMUX  : {}", if *env_present { "set" } else { "not set" });
    }

    let panes = ctx.tmux().global_panes().unwrap_or_default();
    println!("\nprocess ancestry (self first):");
    let mut past_pane = false;
    for (pid, comm) in ancestry() {
        let pane = panes.iter().find(|p| p.pid == pid);
        match pane {
            Some(p) => {
                println!(
                    "  {comm} ({pid})  <- pane {} in session '{}'",
                    p.pane_id, p.session_name
                );
                past_pane = true;
            }
            None => println!("  {comm} ({pid})"),
        }
        // One entry past the pane (the tmux server) is enough context.
        if past_pane && comm.starts_with("tmux") {
            break;
        }
    }
    Ok(())
}
