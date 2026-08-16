use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "ltm",
    version,
    about = "lazy-tmux: modern subcommands, interactive prompts, and a TUI for tmux",
    long_about = "lazy-tmux (ltm) wraps tmux with noun-verb subcommands (`ltm sessions list`),\n\
                  interactive pickers when arguments are omitted, and a lazygit-style TUI.\n\
                  Run with no arguments to open the TUI."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Cmd>,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Manage sessions (default: list)
    #[command(visible_aliases = ["s", "session"])]
    Sessions {
        #[command(subcommand)]
        cmd: Option<SessionsCmd>,
    },
    /// Manage windows (default: list)
    #[command(visible_aliases = ["w", "window"])]
    Windows {
        /// Session to operate on (default: current session, or a picker)
        #[arg(short, long, global = true)]
        session: Option<String>,
        #[command(subcommand)]
        cmd: Option<WindowsCmd>,
    },
    /// Manage panes (default: list)
    #[command(visible_aliases = ["p", "pane"])]
    Panes {
        /// Session to operate on (default: current session, or a picker)
        #[arg(short, long, global = true)]
        session: Option<String>,
        /// Window to operate on, by index or name (default: active window)
        #[arg(short, long, global = true)]
        window: Option<String>,
        #[command(subcommand)]
        cmd: Option<PanesCmd>,
    },
    /// Attach to a session (shortcut for `sessions attach`)
    #[command(visible_alias = "a")]
    Attach {
        /// Session name (omit for an interactive picker)
        name: Option<String>,
    },
    /// Show where you are relative to tmux (direct pane, nested, outside…)
    #[command(visible_alias = "ctx")]
    Context,
    /// Open the TUI session manager (also the default with no arguments)
    Ui,
}

#[derive(Subcommand)]
pub enum SessionsCmd {
    /// List sessions
    #[command(visible_alias = "ls")]
    List,
    /// Create a session (and attach to it, unless --detach)
    #[command(visible_alias = "n")]
    New {
        /// Session name (omit to be prompted; empty lets tmux pick one)
        name: Option<String>,
        /// Working directory for the new session
        #[arg(short = 'c', long)]
        dir: Option<PathBuf>,
        /// Create only; do not attach
        #[arg(short, long)]
        detach: bool,
    },
    /// Attach to a session (switches client when already inside tmux)
    #[command(visible_alias = "a")]
    Attach {
        /// Session name (omit for an interactive picker)
        name: Option<String>,
    },
    /// Kill a session
    #[command(visible_aliases = ["rm", "k"])]
    Kill {
        /// Session name (omit for an interactive picker)
        name: Option<String>,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// Rename a session
    #[command(visible_alias = "mv")]
    Rename {
        /// Session to rename (default: current session, or a picker)
        from: Option<String>,
        /// New name (omit to be prompted)
        to: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum WindowsCmd {
    /// List windows
    #[command(visible_alias = "ls")]
    List,
    /// Create a window
    #[command(visible_alias = "n")]
    New {
        /// Window name (optional)
        name: Option<String>,
        /// Working directory for the new window
        #[arg(short = 'c', long)]
        dir: Option<PathBuf>,
    },
    /// Kill a window
    #[command(visible_aliases = ["rm", "k"])]
    Kill {
        /// Window index or name (omit for an interactive picker)
        target: Option<String>,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// Rename a window
    #[command(visible_alias = "mv")]
    Rename {
        /// Window index or name (omit for an interactive picker)
        target: Option<String>,
        /// New name (omit to be prompted)
        to: Option<String>,
    },
    /// Make a window the active one in its session
    #[command(visible_alias = "sel")]
    Select {
        /// Window index or name (omit for an interactive picker)
        target: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum PanesCmd {
    /// List panes
    #[command(visible_alias = "ls")]
    List,
    /// Split a pane (below by default)
    Split {
        /// Pane index to split (default: the window's active pane)
        target: Option<String>,
        /// Split to the right instead of below
        #[arg(short, long)]
        right: bool,
    },
    /// Kill a pane
    #[command(visible_aliases = ["rm", "k"])]
    Kill {
        /// Pane index (omit for an interactive picker)
        target: Option<String>,
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
}
