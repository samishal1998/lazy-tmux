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
            &format!("#{{client_tty}}{}#{{client_session}}", crate::model::SEP_STR),
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
        self.run(["rename-session", "-t", target, new_name]).map(|_| ())
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
        self.run(["rename-window", "-t", target, new_name]).map(|_| ())
    }

    pub fn select_window(&self, target: &str) -> Result<()> {
        self.run(["select-window", "-t", target]).map(|_| ())
    }

    // ---- pane mutations ---------------------------------------------------

    /// Split a pane. `right = true` puts the new pane beside the old one
    /// (tmux `-h`), otherwise below it (tmux `-v`).
    pub fn split_pane(&self, target: &str, right: bool) -> Result<()> {
        let dir = if right { "-h" } else { "-v" };
        self.run(["split-window", dir, "-d", "-t", target]).map(|_| ())
    }

    pub fn select_pane(&self, target: &str) -> Result<()> {
        self.run(["select-pane", "-t", target]).map(|_| ())
    }

    pub fn kill_pane(&self, target: &str) -> Result<()> {
        self.run(["kill-pane", "-t", target]).map(|_| ())
    }
}
