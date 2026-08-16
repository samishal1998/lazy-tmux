//! Data models parsed from tmux format-string output.
//!
//! Every model carries the tmux *id* (`$n`, `@n`, `%n`) alongside the
//! human-facing name/index. Ids are what we pass back to tmux as targets:
//! they are unambiguous even when names contain `:` or `.`.

use crate::error::{Error, Result};

/// Field delimiter for tmux format strings: the printable "symbol for unit
/// separator" (U+241F). tmux escapes actual control characters in format
/// output (0x1f becomes the literal text `\037`), so the delimiter must be
/// printable — and this one is about as unlikely in a name as it gets.
pub const SEP: char = '\u{241F}';
pub const SEP_STR: &str = "\u{241F}";

fn fields(line: &str, expected: usize, what: &str) -> Result<Vec<String>> {
    let parts: Vec<String> = line.split(SEP).map(str::to_string).collect();
    if parts.len() != expected {
        return Err(Error::Parse(format!(
            "expected {expected} fields for {what}, got {}: {line:?}",
            parts.len()
        )));
    }
    Ok(parts)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub windows: usize,
    /// Number of clients currently attached to this session.
    pub attached: usize,
    /// Unix timestamp of session creation.
    pub created: u64,
    pub path: String,
}

impl Session {
    pub fn format() -> String {
        [
            "#{session_id}",
            "#{session_name}",
            "#{session_windows}",
            "#{session_attached}",
            "#{session_created}",
            "#{session_path}",
        ]
        .join(SEP_STR)
    }

    pub fn parse(line: &str) -> Result<Self> {
        let f = fields(line, 6, "session")?;
        Ok(Self {
            id: f[0].clone(),
            name: f[1].clone(),
            windows: f[2].parse().unwrap_or(0),
            attached: f[3].parse().unwrap_or(0),
            created: f[4].parse().unwrap_or(0),
            path: f[5].clone(),
        })
    }

    pub fn is_attached(&self) -> bool {
        self.attached > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub id: String,
    pub index: usize,
    pub name: String,
    pub panes: usize,
    pub active: bool,
}

impl Window {
    pub fn format() -> String {
        [
            "#{window_id}",
            "#{window_index}",
            "#{window_name}",
            "#{window_panes}",
            "#{window_active}",
        ]
        .join(SEP_STR)
    }

    pub fn parse(line: &str) -> Result<Self> {
        let f = fields(line, 5, "window")?;
        Ok(Self {
            id: f[0].clone(),
            index: f[1].parse().unwrap_or(0),
            name: f[2].clone(),
            panes: f[3].parse().unwrap_or(0),
            active: f[4] == "1",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    pub id: String,
    pub index: usize,
    pub active: bool,
    pub pid: u32,
    pub command: String,
    pub path: String,
    pub width: u16,
    pub height: u16,
    pub title: String,
}

impl Pane {
    pub fn format() -> String {
        [
            "#{pane_id}",
            "#{pane_index}",
            "#{pane_active}",
            "#{pane_pid}",
            "#{pane_current_command}",
            "#{pane_current_path}",
            "#{pane_width}",
            "#{pane_height}",
            "#{pane_title}",
        ]
        .join(SEP_STR)
    }

    pub fn parse(line: &str) -> Result<Self> {
        let f = fields(line, 9, "pane")?;
        Ok(Self {
            id: f[0].clone(),
            index: f[1].parse().unwrap_or(0),
            active: f[2] == "1",
            pid: f[3].parse().unwrap_or(0),
            command: f[4].clone(),
            path: f[5].clone(),
            width: f[6].parse().unwrap_or(0),
            height: f[7].parse().unwrap_or(0),
            title: f[8].clone(),
        })
    }
}

/// A pane seen server-wide (`list-panes -a`), with enough context to map a
/// process id back to the session/window/pane that hosts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalPane {
    pub pid: u32,
    pub pane_id: String,
    pub session_id: String,
    pub session_name: String,
    pub window_index: usize,
}

impl GlobalPane {
    pub fn format() -> String {
        [
            "#{pane_pid}",
            "#{pane_id}",
            "#{session_id}",
            "#{session_name}",
            "#{window_index}",
        ]
        .join(SEP_STR)
    }

    pub fn parse(line: &str) -> Result<Self> {
        let f = fields(line, 5, "global pane")?;
        Ok(Self {
            pid: f[0].parse().unwrap_or(0),
            pane_id: f[1].clone(),
            session_id: f[2].clone(),
            session_name: f[3].clone(),
            window_index: f[4].parse().unwrap_or(0),
        })
    }
}
