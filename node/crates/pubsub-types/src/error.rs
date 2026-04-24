use thiserror::Error;

#[derive(Error, Debug)]
pub enum PubSubError {
    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    #[error("Unauthorized publisher for topic")]
    Unauthorized,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Topic not found: {0}")]
    TopicNotFound(String),

    #[error("Codec error: {0}")]
    Codec(String),

    #[error("Store error: {0}")]
    Store(String),

    #[error("Chain state error: {0}")]
    ChainState(String),

    #[error("Peer not found: {0}")]
    PeerNotFound(String),

    #[error("{0}")]
    Other(String),
}
