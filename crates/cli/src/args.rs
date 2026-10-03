use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Clone, Copy, ValueEnum)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl From<Direction> for lazytmux_core::ResizeDir {
    fn from(d: Direction) -> Self {
        match d {
            Direction::Up => Self::Up,
            Direction::Down => Self::Down,
            Direction::Left => Self::Left,
            Direction::Right => Self::Right,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OnOff {
    On,
    Off,
}

impl OnOff {
    pub fn as_str(self) -> &'static str {
        match self {
            OnOff::On => "on",
            OnOff::Off => "off",
        }
    }
}

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
    /// Tmux options: mouse, status bar, pane sync, or any option by name
    #[command(visible_aliases = ["o", "opt"])]
    Options {
        #[command(subcommand)]
        cmd: Option<OptionsCmd>,
    },
    /// User-defined macros: named tmux command sequences (see `macros edit`)
    #[command(visible_aliases = ["m", "macro"])]
    Macros {
        #[command(subcommand)]
        cmd: Option<MacrosCmd>,
    },
    /// Check for common tmux paper-cuts (Shift-Enter, colors, Esc delay…)
    Doctor {
        /// Apply the recommended values to the running server
        #[arg(long)]
        fix: bool,
        /// Print a ~/.tmux.conf snippet instead of a report
        #[arg(long)]
        conf: bool,
    },
    /// Kill the tmux server and ALL sessions
    KillServer {
        /// Skip the confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },
    /// Show where you are relative to tmux (direct pane, nested, outside…)
    #[command(visible_alias = "ctx")]
    Context,
    /// Guided interactive mode: menus for every action
    #[command(visible_alias = "i")]
    Interactive,
    /// Generate shell completions (bash, zsh, fish, elvish, powershell)
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
        /// Write to the shell's standard completions directory instead of
        /// stdout (rerun after upgrading ltm to pick up new commands)
        #[arg(long)]
        install: bool,
    },
    /// Save live sessions as macros (to stdout or a file) to recreate later
    Extract {
        /// Sessions to extract (omit to pick interactively)
        sessions: Vec<String>,
        /// Extract every session
        #[arg(short, long)]
        all: bool,
        /// Also record the command each pane is running, and re-run it on restore
        #[arg(long)]
        with_running_process: bool,
        /// Write to this file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Overwrite --output if it exists
        #[arg(long)]
        force: bool,
    },
    /// Update ltm to the latest GitHub release
    Update {
        /// Only report whether a newer release exists
        #[arg(long)]
        check: bool,
        /// Install this release (e.g. v0.2.0) instead of the latest
        #[arg(long)]
        version: Option<String>,
        /// Reinstall even if already up to date
        #[arg(long)]
        force: bool,
    },
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
    /// Detach all clients from a session
    #[command(visible_alias = "d")]
    Detach {
        /// Session name (omit for an interactive picker)
        name: Option<String>,
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
    /// Swap two windows (reorder within the session)
    Swap {
        /// First window, by index or name (omit for a picker)
        a: Option<String>,
        /// Second window, by index or name (omit for a picker)
        b: Option<String>,
    },
    /// Move a window to another session
    Move {
        /// Window index or name (omit for an interactive picker)
        target: Option<String>,
        /// Destination session name (omit for a picker)
        #[arg(long)]
        to: Option<String>,
    },
    /// Cycle a window to the next preset layout
    Layout {
        /// Window index or name (default: active window)
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
    /// Resize a pane
    Resize {
        /// Direction to grow the pane in
        direction: Direction,
        /// Cells to resize by
        #[arg(default_value = "5")]
        amount: u16,
        /// Pane index (default: the window's active pane)
        #[arg(short, long)]
        pane: Option<String>,
    },
    /// Toggle zoom on a pane
    Zoom {
        /// Pane index (default: the window's active pane)
        target: Option<String>,
    },
    /// Swap two panes (reorder within the window)
    Swap {
        /// First pane index (omit for a picker)
        a: Option<String>,
        /// Second pane index (omit for a picker)
        b: Option<String>,
    },
    /// Set a pane's title
    #[command(visible_alias = "mv")]
    Rename {
        /// Pane index (omit for an interactive picker)
        target: Option<String>,
        /// New title (omit to be prompted)
        title: Option<String>,
    },
    /// Break a pane out into its own window
    Break {
        /// Pane index (omit for an interactive picker)
        target: Option<String>,
    },
    /// Make a pane the focused one in its window
    #[command(visible_alias = "sel")]
    Select {
        /// Pane index (omit for an interactive picker)
        target: Option<String>,
    },
    /// Join a pane into another window as a new split
    Join {
        /// Pane index to move (omit for an interactive picker)
        target: Option<String>,
        /// Destination window, by index or name (omit for a picker)
        #[arg(long)]
        to: Option<String>,
        /// Join as a horizontal split (side by side) instead of below
        #[arg(short, long)]
        right: bool,
    },
}

#[derive(Subcommand)]
pub enum MacrosCmd {
    /// List defined macros
    #[command(visible_alias = "ls")]
    List,
    /// Run a macro (omit the name for an interactive picker)
    #[command(visible_alias = "r")]
    Run {
        /// Macro name
        name: Option<String>,
    },
    /// Show a macro's steps
    Show {
        /// Macro name (omit for an interactive picker)
        name: Option<String>,
    },
    /// Open the macros file in $EDITOR (creates a starter file if missing)
    Edit,
}

#[derive(Subcommand)]
pub enum OptionsCmd {
    /// Show common options and their current values
    #[command(visible_alias = "ls")]
    List,
    /// Turn mouse support on/off (no value = toggle)
    Mouse { state: Option<OnOff> },
    /// Turn the status bar on/off (no value = toggle)
    Status { state: Option<OnOff> },
    /// Synchronize input across the current window's panes (no value = toggle)
    Sync { state: Option<OnOff> },
    /// Read any tmux option by name
    Get {
        /// Option name (e.g. history-limit)
        name: String,
    },
    /// Set any tmux option by name
    Set {
        /// Option name (e.g. history-limit)
        name: String,
        /// Value to set
        value: String,
        /// Set it on the current window instead of globally
        #[arg(short, long)]
        window: bool,
    },
}
