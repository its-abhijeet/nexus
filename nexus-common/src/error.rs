use bincode;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum NexusError {
    #[error("I/O Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization Error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Configuration Error: {0}")]
    Config(String),

    #[error("Consensus Error: {0}")]
    Consensus(String),
}

pub type Result<T> = std::result::Result<T, NexusError>;

impl From<bincode::Error> for NexusError {
    fn from(err: bincode::Error) -> Self {
        NexusError::Config(format!("Bincode Error: {}", err))
    }
}
