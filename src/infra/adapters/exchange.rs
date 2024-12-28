use std::fmt::Debug;
use async_trait::async_trait;
use log::info;

use crate::common::types::Error;
use crate::common::enums::ExchangeName;
use crate::config::ExchangeConfig;
use crate::domain::interfaces::DataAdapterPort;
/// Trait defining common functionality for exchange adapters
#[async_trait]
pub trait ExchangeAdapter<T: Send + Sync + Debug>: DataAdapterPort<T> {
    /// Returns the API URL for the exchange
    fn get_api_url(&self) -> String;

    /// Returns the trading pairs supported by the exchange
    fn get_pairs(&self) -> Vec<String>;

    /// Returns the exchange name
    fn get_exchange_name(&self) -> ExchangeName;
}

/// Base implementation for exchange adapters
#[derive(Debug, Clone)]
pub struct ExchangeAdapterImpl {
    api_url: String,
    pairs: Vec<String>,
    exchange_name: ExchangeName,
    sink_topics: Vec<String>,
}

impl ExchangeAdapterImpl {
    /// Creates a new ExchangeAdapterImpl instance
    pub fn new(config: ExchangeConfig, name: ExchangeName) -> Result<Self, Error> {
        // Validate configuration
        if config.api_url.is_empty() {
            return Err(Error::ValidationError("API URL cannot be empty".to_string()));
        }
        if config.pairs.is_empty() {
            return Err(Error::ValidationError("Trading pairs cannot be empty".to_string()));
        }

        // Validate trading pairs format
        for pair in &config.pairs {
            if !Self::is_valid_trading_pair(pair) {
                return Err(Error::ValidationError(format!("Invalid trading pair format: {}", pair)));
            }
        }

        let sink_topic = format!("price.feed.{}", name.to_string().to_lowercase());

        Ok(ExchangeAdapterImpl {
            api_url: config.api_url,
            pairs: config.pairs,
            exchange_name: name,
            sink_topics: vec![sink_topic],
        })
    }

    /// Returns the sink topics for this adapter
    pub fn get_sink_topics(&self) -> Vec<String> {
        self.sink_topics.clone()
    }

    /// Validates trading pair format (e.g., "BTC-USD")
    fn is_valid_trading_pair(pair: &str) -> bool {
        let parts: Vec<&str> = pair.split('-').collect();
        if parts.len() != 2 {
            return false;
        }

        parts.iter().all(|p| {
            !p.is_empty() && p.chars().all(|c| c.is_ascii_alphabetic())
        })
    }
}
 #[async_trait]
 impl<T: Send + Sync + Debug> DataAdapterPort<T> for ExchangeAdapterImpl {
     async fn connect(&self) -> Result<(), Error> {
         info!("Connecting to exchange at {}", self.get_api_url());
         // Base implementation just logs - specific adapters will override this
         Ok(())
     }

     async fn process_message(&self) -> Result<Option<T>, Error> {
         // Base implementation returns None - specific adapters will override this
         Ok(None)
     }

     async fn start(&self, _callback: fn(T)) -> Result<(), Error> {
         info!("Starting exchange adapter for {}", self.get_api_url());
         // Base implementation just logs - specific adapters will override this
         Ok(())
     }

     async fn stop(&self) -> Result<(), Error> {
         info!("Stopping exchange adapter for {}", self.get_api_url());
         // Base implementation just logs - specific adapters will override this
         Ok(())
     }

     fn get_sink_topics(&self) -> Vec<String> {
         self.sink_topics.clone()
     }
}

 impl<T: Send + Sync + Debug> ExchangeAdapter<T> for ExchangeAdapterImpl {
     fn get_api_url(&self) -> String {
         self.api_url.clone()
     }

     fn get_pairs(&self) -> Vec<String> {
         self.pairs.clone()
     }

     fn get_exchange_name(&self) -> ExchangeName {
         self.exchange_name
     }
 }

 #[cfg(test)]
 mod tests {
     use super::*;

     #[test]
     fn test_valid_trading_pair() {
         assert!(ExchangeAdapterImpl::is_valid_trading_pair("BTC-USD"));
         assert!(ExchangeAdapterImpl::is_valid_trading_pair("ETH-EUR"));

         // Invalid cases
         assert!(!ExchangeAdapterImpl::is_valid_trading_pair(""));
         assert!(!ExchangeAdapterImpl::is_valid_trading_pair("BTC"));
         assert!(!ExchangeAdapterImpl::is_valid_trading_pair("BTC-"));
         assert!(!ExchangeAdapterImpl::is_valid_trading_pair("-USD"));
         assert!(!ExchangeAdapterImpl::is_valid_trading_pair("BTC-USD-EUR"));
         assert!(!ExchangeAdapterImpl::is_valid_trading_pair("BTC123-USD"));
     }

     #[test]
     fn test_exchange_adapter_creation() {
         let config = ExchangeConfig {
             api_url: "wss://example.com".to_string(),
             pairs: vec!["BTC-USD".to_string()],
         };

         let adapter = ExchangeAdapterImpl::new(config, ExchangeName::Coinbase);
         assert!(adapter.is_ok());

         let config_empty_url = ExchangeConfig {
             api_url: "".to_string(),
             pairs: vec!["BTC-USD".to_string()],
         };
         assert!(ExchangeAdapterImpl::new(config_empty_url, ExchangeName::Coinbase).is_err());

         let config_empty_pairs = ExchangeConfig {
             api_url: "wss://example.com".to_string(),
             pairs: vec![],
         };
         assert!(ExchangeAdapterImpl::new(config_empty_pairs, ExchangeName::Coinbase).is_err());
     }
}
