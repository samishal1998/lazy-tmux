//! Thin, typed wrapper around the `tmux` binary.
//!
//! All queries go through format strings delimited by an ASCII unit
//! separator, so names containing spaces, colons, or quotes round-trip
//! safely. Mutations target tmux *ids* wherever possible.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};
use crate::model::{GlobalPane, Pane, Session, Window};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDir {
    Up,
    Down,
    Left,
    Right,
}

impl ResizeDir {
    fn flag(self) -> &'static str {
        match self {
            ResizeDir::Up => "-U",
            ResizeDir::Down => "-D",
            ResizeDir::Left => "-L",
            ResizeDir::Right => "-R",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Tmux {
    socket: Option<PathBuf>,
}

impl Tmux {
    /// Talk to tmux with ambient defaults ($TMUX env, or the default socket).
    pub fn new() -> Self {
        Self { socket: None }
    }

    /// Talk to a specific server socket, regardless of $TMUX.
    pub fn with_socket(socket: impl Into<PathBuf>) -> Self {
        Self {
            socket: Some(socket.into()),
        }
    }

    pub fn socket(&self) -> Option<&Path> {
        self.socket.as_deref()
    }

    /// Base `tmux` command with the socket flag applied (for callers that
    /// need to `exec` tmux themselves, e.g. attach).
    pub fn command(&self) -> Command {
        let mut cmd = Command::new("tmux");
        if let Some(sock) = &self.socket {
            cmd.arg("-S").arg(sock);
        }
        cmd
    }

    pub fn run<I, S>(&self, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.command().args(args).output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::TmuxNotFound
            } else {
                Error::Io(e)
            }
        })?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if stderr.contains("no server running") || stderr.contains("error connecting to") {
                Err(Error::NoServer)
            } else {
                Err(Error::Tmux(stderr))
            }
        }
    }

    pub fn server_running(&self) -> bool {
        !matches!(
            self.run(["list-sessions", "-F", ""]),
            Err(Error::NoServer) | Err(Error::TmuxNotFound)
        )
    }

    // ---- queries ----------------------------------------------------------

    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let out = match self.run(["list-sessions", "-F", &Session::format()]) {
            Ok(out) => out,
            Err(Error::NoServer) => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        out.lines().map(Session::parse).collect()
    }

    pub fn session_by_name(&self, name: &str) -> Result<Option<Session>> {
        Ok(self.list_sessions()?.into_iter().find(|s| s.name == name))
    }

    pub fn list_windows(&self, session: &str) -> Result<Vec<Window>> {
        let out = self.run(["list-windows", "-t", session, "-F", &Window::format()])?;
        out.lines().map(Window::parse).collect()
    }

    pub fn list_panes(&self, window: &str) -> Result<Vec<Pane>> {
        let out = self.run(["list-panes", "-t", window, "-F", &Pane::format()])?;
        out.lines().map(Pane::parse).collect()
    }

    /// Every pane on the server, with its root process pid.
    pub fn global_panes(&self) -> Result<Vec<GlobalPane>> {
        let out = self.run(["list-panes", "-a", "-F", &GlobalPane::format()])?;
        out.lines().map(GlobalPane::parse).collect()
    }

    /// Visible contents of a pane, plain text.
    pub fn capture_pane(&self, pane: &str) -> Result<String> {
        self.run(["capture-pane", "-p", "-t", pane])
    }

    /// Ttys of clients attached to the given session name.
    pub fn clients_of_session(&self, session_name: &str) -> Result<Vec<String>> {
        let out = self.run([
            "list-clients",
            "-F",
            &format!(
                "#{{client_tty}}{}#{{client_session}}",
                crate::model::SEP_STR
            ),
        ])?;
        Ok(out
            .lines()
            .filter_map(|l| l.split_once(crate::model::SEP))
            .filter(|(_, sess)| *sess == session_name)
            .map(|(tty, _)| tty.to_string())
            .collect())
    }

    // ---- session mutations ------------------------------------------------

    /// Create a detached session and return it. `name: None` lets tmux pick
    /// the next numeric name.
    pub fn new_session(&self, name: Option<&str>, dir: Option<&Path>) -> Result<Session> {
        let fmt = Session::format();
        let mut args: Vec<&OsStr> = vec![
            "new-session".as_ref(),
            "-d".as_ref(),
            "-P".as_ref(),
            "-F".as_ref(),
            fmt.as_ref(),
        ];
        if let Some(name) = name {
            args.push("-s".as_ref());
            args.push(name.as_ref());
        }
        if let Some(dir) = dir {
            args.push("-c".as_ref());
            args.push(dir.as_ref());
        }
        let out = self.run(args)?;
        Session::parse(out.trim_end_matches('\n'))
    }

    pub fn kill_session(&self, target: &str) -> Result<()> {
        self.run(["kill-session", "-t", target]).map(|_| ())
    }

    pub fn rename_session(&self, target: &str, new_name: &str) -> Result<()> {
        self.run(["rename-session", "-t", target, new_name])
            .map(|_| ())
    }

    /// Switch the current (or given) client to another session/window.
    /// Only valid when a client exists — i.e. we are inside tmux.
    pub fn switch_client(&self, target: &str, client_tty: Option<&str>) -> Result<()> {
        let mut args = vec!["switch-client", "-t", target];
        if let Some(tty) = client_tty {
            args.push("-c");
            args.push(tty);
        }
        self.run(args).map(|_| ())
    }

    // ---- window mutations -------------------------------------------------

    pub fn new_window(
        &self,
        session: &str,
        name: Option<&str>,
        dir: Option<&Path>,
    ) -> Result<Window> {
        let fmt = Window::format();
        let mut args: Vec<&OsStr> = vec![
            "new-window".as_ref(),
            "-d".as_ref(),
            "-P".as_ref(),
            "-F".as_ref(),
            fmt.as_ref(),
            "-t".as_ref(),
            session.as_ref(),
        ];
        if let Some(name) = name {
            args.push("-n".as_ref());
            args.push(name.as_ref());
        }
        if let Some(dir) = dir {
            args.push("-c".as_ref());
            args.push(dir.as_ref());
        }
        let out = self.run(args)?;
        Window::parse(out.trim_end_matches('\n'))
    }

    pub fn kill_window(&self, target: &str) -> Result<()> {
        self.run(["kill-window", "-t", target]).map(|_| ())
    }

    pub fn rename_window(&self, target: &str, new_name: &str) -> Result<()> {
        self.run(["rename-window", "-t", target, new_name])
            .map(|_| ())
    }

    pub fn select_window(&self, target: &str) -> Result<()> {
        self.run(["select-window", "-t", target]).map(|_| ())
    }

    // ---- pane mutations ---------------------------------------------------

    /// Split a pane. `right = true` puts the new pane beside the old one
    /// (tmux `-h`), otherwise below it (tmux `-v`).
    pub fn split_pane(&self, target: &str, right: bool) -> Result<()> {
        let dir = if right { "-h" } else { "-v" };
        self.run(["split-window", dir, "-d", "-t", target])
            .map(|_| ())
    }

    pub fn select_pane(&self, target: &str) -> Result<()> {
        self.run(["select-pane", "-t", target]).map(|_| ())
    }

    pub fn kill_pane(&self, target: &str) -> Result<()> {
        self.run(["kill-pane", "-t", target]).map(|_| ())
    }

    pub fn resize_pane(&self, target: &str, dir: ResizeDir, amount: u16) -> Result<()> {
        self.run(["resize-pane", dir.flag(), "-t", target, &amount.to_string()])
            .map(|_| ())
    }

    /// Toggle zoom for a pane.
    pub fn zoom_pane(&self, target: &str) -> Result<()> {
        self.run(["resize-pane", "-Z", "-t", target]).map(|_| ())
    }

    pub fn swap_panes(&self, a: &str, b: &str) -> Result<()> {
        self.run(["swap-pane", "-d", "-s", a, "-t", b]).map(|_| ())
    }

    /// Swap a pane with the previous (`up = true`) or next one in the window.
    pub fn swap_pane_step(&self, target: &str, up: bool) -> Result<()> {
        let dir = if up { "-U" } else { "-D" };
        self.run(["swap-pane", "-d", dir, "-t", target]).map(|_| ())
    }

    /// Set a pane's title (shown in borders / `#{pane_title}`).
    pub fn set_pane_title(&self, target: &str, title: &str) -> Result<()> {
        self.run(["select-pane", "-t", target, "-T", title])
            .map(|_| ())
    }

    /// Break a pane out into its own window (stays in the background).
    pub fn break_pane(&self, target: &str) -> Result<()> {
        self.run(["break-pane", "-d", "-s", target]).map(|_| ())
    }

    /// Join a pane into a target window/pane as a new split.
    pub fn join_pane(&self, src: &str, dst: &str, right: bool) -> Result<()> {
        let dir = if right { "-h" } else { "-v" };
        self.run(["join-pane", dir, "-d", "-s", src, "-t", dst])
            .map(|_| ())
    }

    /// Cycle the window to the next preset layout.
    pub fn next_layout(&self, window: &str) -> Result<()> {
        self.run(["next-layout", "-t", window]).map(|_| ())
    }

    pub fn swap_windows(&self, a: &str, b: &str) -> Result<()> {
        self.run(["swap-window", "-d", "-s", a, "-t", b])
            .map(|_| ())
    }

    /// Move a window to another session, appending after its last window.
    pub fn move_window_to_session(&self, window: &str, session: &str) -> Result<()> {
        let next_index = self
            .list_windows(session)?
            .iter()
            .map(|w| w.index)
            .max()
            .map_or(0, |i| i + 1);
        self.run([
            "move-window",
            "-d",
            "-s",
            window,
            "-t",
            &format!("{session}:{next_index}"),
        ])
        .map(|_| ())
    }

    /// Detach every client attached to a session.
    pub fn detach_clients(&self, session: &str) -> Result<()> {
        self.run(["detach-client", "-s", session]).map(|_| ())
    }

    /// Kill the whole server (all sessions).
    pub fn kill_server(&self) -> Result<()> {
        self.run(["kill-server"]).map(|_| ())
    }

    // ---- options ----------------------------------------------------------

    pub fn set_global_option(&self, name: &str, value: &str) -> Result<()> {
        self.run(["set-option", "-g", name, value]).map(|_| ())
    }

    /// Read a server option (`-s`), e.g. escape-time or extended-keys.
    /// Array options come back as one value per line.
    pub fn show_server_option(&self, name: &str) -> Result<String> {
        Ok(self
            .run(["show-options", "-s", "-v", name])?
            .trim()
            .to_string())
    }

    pub fn set_server_option(&self, name: &str, value: &str) -> Result<()> {
        self.run(["set-option", "-s", name, value]).map(|_| ())
    }

    /// Append to a server array option (e.g. terminal-features).
    pub fn append_server_option(&self, name: &str, value: &str) -> Result<()> {
        self.run(["set-option", "-s", "-a", name, value])
            .map(|_| ())
    }

    pub fn show_global_option(&self, name: &str) -> Result<String> {
        Ok(self
            .run(["show-options", "-g", "-v", name])?
            .trim()
            .to_string())
    }

    pub fn global_flag(&self, name: &str) -> Result<bool> {
        Ok(self.show_global_option(name)? == "on")
    }

    /// Toggle an on/off global option (tmux toggles when no value is given)
    /// and return the new state.
    pub fn toggle_global_flag(&self, name: &str) -> Result<bool> {
        self.run(["set-option", "-g", name])?;
        self.global_flag(name)
    }

    pub fn set_window_option(&self, window: &str, name: &str, value: &str) -> Result<()> {
        self.run(["set-option", "-w", "-t", window, name, value])
            .map(|_| ())
    }

    pub fn window_flag(&self, window: &str, name: &str) -> Result<bool> {
        Ok(self
            .run(["show-options", "-w", "-t", window, "-v", name])?
            .trim()
            == "on")
    }

    pub fn toggle_window_flag(&self, window: &str, name: &str) -> Result<bool> {
        self.run(["set-option", "-w", "-t", window, name])?;
        self.window_flag(window, name)
    }
}
