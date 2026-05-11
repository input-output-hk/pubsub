use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecureCyclonError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("gossip exchange timed out")]
    ExchangeTimeout,
    #[error("bootstrap source returned no seeds")]
    EmptyBootstrap,
    #[error("invalid gossip message: {0}")]
    InvalidMessage(String),
}

pub type Result<T> = std::result::Result<T, SecureCyclonError>;
