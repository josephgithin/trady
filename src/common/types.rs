use thiserror::Error;

/// Result type alias using our custom error type
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Operation error: {0}")]
    OperationError(String),

    #[error("Publish error: {0}")]
    PublishError(String),

    #[error("Subscription error: {0}")]
    SubscriptionError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Exchange error: {0}")]
    ExchangeError(String),

    #[error("Strategy error: {0}")]
    StrategyError(String),

    #[error("Broker error: {0}")]
    BrokerError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}
