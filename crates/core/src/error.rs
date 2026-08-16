pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("tmux executable not found in PATH")]
    TmuxNotFound,

    #[error("no tmux server is running")]
    NoServer,

    #[error("tmux: {0}")]
    Tmux(String),

    #[error("failed to parse tmux output: {0}")]
    Parse(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
