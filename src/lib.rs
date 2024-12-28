//! Trading platform core library
//! 
//! This library provides the core functionality for the trading platform,
//! including common types, enums, and constants used throughout the system.

pub mod common {
    use thiserror::Error;

    /// Common error types for the trading platform
    pub mod types {
        use super::Error as CommonError;

        /// Result type alias using our custom error type
        pub type Result<T> = std::result::Result<T, CommonError>;

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
    }

    /// Common enums used throughout the platform
    pub mod enums {
        use serde::{Deserialize, Serialize};
        use std::fmt;

        /// Supported exchanges
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
        pub enum ExchangeName {
            Kraken,
            Coinbase,
            Binance,
            Bitfinex,
            FTX,
        }

        impl fmt::Display for ExchangeName {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:?}", self)
            }
        }

        /// Available trading strategies
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
        pub enum StrategyName {
            Arbitrage,
            MeanReversion,
            Momentum,
            MarketMaking,
            StatisticalArbitrage,
        }

        impl fmt::Display for StrategyName {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:?}", self)
            }
        }

        /// Supported brokers
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
        pub enum BrokerName {
            InteractiveBrokers,
            AlpacaMarkets,
            TDAmeritrade,
        }

        impl fmt::Display for BrokerName {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:?}", self)
            }
        }

        /// Order types
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
        pub enum OrderType {
            Market,
            Limit,
            StopLoss,
            StopLimit,
            TrailingStop,
        }

        /// Trading side
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
        pub enum TradeSide {
            Buy,
            Sell,
        }
    }

    /// Platform-wide constants
    pub mod constants {
        /// Topic names for the event bus
        pub mod topics {
            pub const DATA: &str = "data";
            pub const TRADES: &str = "trades";
            pub const ORDERS: &str = "orders";
            pub const LOG: &str = "log";
            pub const METRICS: &str = "metrics";
            pub const AUDIT: &str = "audit";
        }

        /// System-wide timeouts
        pub mod timeouts {
            pub const DEFAULT_TIMEOUT_MS: u64 = 5000;
            pub const NETWORK_TIMEOUT_MS: u64 = 10000;
            pub const ORDER_TIMEOUT_MS: u64 = 30000;
        }

        /// Rate limiting constants
        pub mod rate_limits {
            pub const MAX_REQUESTS_PER_SECOND: u32 = 10;
            pub const MAX_ORDERS_PER_SECOND: u32 = 5;
        }
    }
}

// Re-export commonly used types
pub use common::types::{Error, Result};
pub use common::enums::{ExchangeName, StrategyName, BrokerName, OrderType, TradeSide};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enum_display() {
        assert_eq!(ExchangeName::Kraken.to_string(), "Kraken");
        assert_eq!(StrategyName::Arbitrage.to_string(), "Arbitrage");
        assert_eq!(BrokerName::InteractiveBrokers.to_string(), "InteractiveBrokers");
    }

    #[test]
    fn test_error_display() {
        let err = Error::ValidationError("test error".to_string());
        assert_eq!(err.to_string(), "Validation error: test error");
    }
}
