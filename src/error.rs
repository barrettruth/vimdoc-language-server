use thiserror::Error;

#[derive(Debug, Error)]
pub enum LspError {
    #[error("protocol error: {0}")]
    Protocol(String),
}
