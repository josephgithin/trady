use std::fmt::Debug;
use async_trait::async_trait;

use crate::common::types::Error;
use crate::config::BrokerConfig;
use crate::domain::interfaces::BrokerPort;
use crate::domain::models::Order;
use crate::domain::events::OrderStatusEvent;

/// Base trait for broker adapters that defines common functionality
#[async_trait]
pub trait BaseBrokerAdapter<OrderType, OrderStatusType>: 
    BrokerPort<OrderType = OrderType, OrderStatusType = OrderStatusType> + Send + Sync 
where
    OrderType: Send + Sync + Debug,
    OrderStatusType: Send + Sync + Debug,
{
    /// Returns the broker API endpoint
    fn get_broker_api(&self) -> String;
    
    /// Validates the broker configuration
    fn validate_config(config: &BrokerConfig) -> Result<(), Error> {
        if config.broker_api.is_empty() {
            return Err(Error::ValidationError("Broker API URL cannot be empty".to_string()));
        }
        if !config.broker_api.starts_with("http") && !config.broker_api.starts_with("ws") {
            return Err(Error::ValidationError("Invalid broker API URL format".to_string()));
        }
        Ok(())
    }
}

/// Base implementation for broker adapters
#[derive(Debug, Clone)]
pub struct BrokerAdapterImpl {
    broker_api: String,
}

impl BrokerAdapterImpl {
    /// Creates a new BrokerAdapterImpl instance
    pub fn new(config: BrokerConfig) -> Result<Self, Error> {
        Self::validate_config(&config)?;
        
        Ok(Self {
            broker_api: config.broker_api,
        })
    }

    /// Validates the broker configuration
    fn validate_config(config: &BrokerConfig) -> Result<(), Error> {
        if config.broker_api.is_empty() {
            return Err(Error::ValidationError("Broker API URL cannot be empty".to_string()));
        }
        if !config.broker_api.starts_with("http") && !config.broker_api.starts_with("ws") {
            return Err(Error::ValidationError("Invalid broker API URL format".to_string()));
        }
        Ok(())
    }
}
 #[async_trait]
 impl BrokerPort for BrokerAdapterImpl {
     type OrderType = Order;
     type OrderStatusType = OrderStatusEvent;
 
     /// Places a new order with the broker
     async fn place_order(&self, order: Self::OrderType) -> Result<(), Error> {
         // Base implementation - should be overridden by specific adapters
         info!("Base implementation - place_order not implemented");
         Ok(())
     }
 
     /// Gets the status of an existing order
     async fn get_order_status(&self) -> Result<Option<Self::OrderStatusType>, Error> {
         // Base implementation - should be overridden by specific adapters
         info!("Base implementation - get_order_status not implemented");
         Ok(None)
     }
 
     /// Starts the broker adapter
     async fn start(&self, _callback: fn(Self::OrderStatusType)) -> Result<(), Error> {
         info!("Starting broker adapter for {}", self.broker_api);
         Ok(())
     }
 
     /// Stops the broker adapter
     async fn stop(&self) -> Result<(), Error> {
         info!("Stopping broker adapter for {}", self.broker_api);
         Ok(())
     }
}
 impl BaseBrokerAdapter<Order, OrderStatusEvent> for BrokerAdapterImpl {
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
             broker_api: "https://api.broker.com".to_string(),
         };
         let adapter = BrokerAdapterImpl::new(config);
         assert!(adapter.is_ok());
 
         // Invalid configuration - empty URL
         let config = BrokerConfig {
             broker_api: "".to_string(),
         };
         let adapter = BrokerAdapterImpl::new(config);
         assert!(adapter.is_err());
 
         // Invalid configuration - invalid URL format
         let config = BrokerConfig {
             broker_api: "invalid-url".to_string(),
         };
         let adapter = BrokerAdapterImpl::new(config);
         assert!(adapter.is_err());
     }
 
     #[test]
     fn test_broker_api_getter() {
         let config = BrokerConfig {
             broker_api: "https://api.broker.com".to_string(),
         };
         let adapter = BrokerAdapterImpl::new(config).unwrap();
         assert_eq!(adapter.get_broker_api(), "https://api.broker.com");
     }
 }
