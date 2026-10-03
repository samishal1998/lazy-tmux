//! Core library for lazy-tmux: talking to tmux servers, data models, and
//! detection of where the current process runs relative to tmux.
//!
//! This crate is UI-free on purpose: both the CLI and the TUI (and a future
//! daemon, if one ever becomes necessary) build on top of it.

pub mod attach;
pub mod context;
pub mod error;
pub mod extract;
pub mod macros;
pub mod model;
pub mod tmux;

pub use attach::{plan_attach, AttachPlan};
pub use context::{detect, Location, TmuxContext};
pub use error::{Error, Result};
pub use model::{Pane, Session, Window};
pub use tmux::{ResizeDir, Tmux};
