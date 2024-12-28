use std::fmt::Debug;
use async_trait::async_trait;
use log::info;

use crate::common::types::Error;
use crate::config::BrokerConfig;
use crate::domain::interfaces::BrokerPort;
use crate::domain::models::Order;
use crate::domain::events::OrderStatusEvent;

/// Base trait for broker adapters that defines common functionality
#[async_trait]
pub trait BaseBrokerAdapter: Send + Sync + Debug {
    type OrderType: Send + Sync + Debug;
    type OrderStatusType: Send + Sync + Debug;

    /// Returns the broker API endpoint
    fn get_broker_api(&self) -> String;

    /// Validates the broker configuration
    fn validate_config(config: &BrokerConfig) -> Result<(), Error> {
        if config.host.is_empty() {
            return Err(Error::ValidationError("Broker host cannot be empty".to_string()));
        }
        if config.port == 0 {
            return Err(Error::ValidationError("Broker port cannot be zero".to_string()));
        }
        Ok(())
    }
}

/// Base implementation for broker adapters
#[derive(Debug, Clone)]
pub struct BrokerAdapterImpl {
    broker_api: String,
    sink_topics: Vec<String>
}

impl BrokerAdapterImpl {
    /// Creates a new BrokerAdapterImpl instance
    pub fn new(config: BrokerConfig) -> Result<Self, Error> {
        Self::validate_config(&config)?;

        Ok(Self {
            broker_api: format!(
                "{}://{}:{}", 
                if config.paper_trading { "https" } else { "http" }, 
                config.host, 
                config.port
            ),
            sink_topics: vec![]
        })
    }

    pub fn get_sink_topics(&self) -> Vec<String> {
        self.sink_topics.clone()
    }
}

#[async_trait]
impl BrokerPort for BrokerAdapterImpl {
    type OrderType = Order;
    type OrderStatusType = OrderStatusEvent;

    async fn place_order(&self, _order: Self::OrderType) -> Result<(), Error> {
        info!("Base implementation - place_order not implemented");
        Ok(())
    }

    async fn get_order_status(&self) -> Result<Option<Self::OrderStatusType>, Error> {
        info!("Base implementation - get_order_status not implemented");
        Ok(None)
    }

    async fn start<F>(&self, _callback: F) -> Result<(), Error>
    where
        F: FnMut(Self::OrderStatusType) + Send + 'static,
    {
        info!("Starting broker adapter for {}", self.broker_api);
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        info!("Stopping broker adapter for {}", self.broker_api);
        Ok(())
    }

    fn get_sink_topics(&self) -> Vec<String> {
        self.sink_topics.clone()
    }
}

impl BaseBrokerAdapter for BrokerAdapterImpl {
    type OrderType = Order;
    type OrderStatusType = OrderStatusEvent;

    fn get_broker_api(&self) -> String {
        self.broker_api.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broker_adapter_creation() {
        // Valid configuration
        let config = BrokerConfig {
            host: "api.broker.com".to_string(),
            port: 8080,
            client_id: 1,
            account_id: "".to_string(),
            paper_trading: false,
            max_retries: 3,
            timeout: 5000
        };
        let adapter = BrokerAdapterImpl::new(config);
        assert!(adapter.is_ok());

        // Invalid configuration - empty host
        let config = BrokerConfig {
            host: "".to_string(),
            port: 0,
            client_id: 1,
            account_id: "".to_string(),
            paper_trading: false,
            max_retries: 3,
            timeout: 5000
        };
        let adapter = BrokerAdapterImpl::new(config);
        assert!(adapter.is_err());

        // Invalid configuration - zero port
        let config = BrokerConfig {
            host: "invalid-url".to_string(),
            port: 0,
            client_id: 1,
            account_id: "".to_string(),
            paper_trading: false,
            max_retries: 3,
            timeout: 5000
        };
        let adapter = BrokerAdapterImpl::new(config);
        assert!(adapter.is_err());
    }

    #[test]
    fn test_broker_api_getter() {
        let config = BrokerConfig {
            host: "api.broker.com".to_string(),
            port: 8080,
            client_id: 1,
            account_id: "".to_string(),
            paper_trading: false,
            max_retries: 3,
            timeout: 5000
        };
        let adapter = BrokerAdapterImpl::new(config).unwrap();
        assert_eq!(adapter.get_broker_api(), "http://api.broker.com:8080");
    }
}
