use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use async_trait::async_trait;
use log::{error, info};
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::common::types::Error;
use crate::config::BrokerConfig;
use crate::domain::events::{OrderStatusEvent, OrderStatus};
use crate::domain::interfaces::BrokerPort;
use crate::domain::models::Order;
use crate::infra::broker_adapters::base::{BaseBrokerAdapter, BrokerAdapterImpl};
use crate::common::enums::OrderType;
use crate::common::enums::TradeSide;
/// Interactive Broker adapter implementation
#[derive(Debug)]
pub struct InteractiveBrokerAdapter {
    base: BrokerAdapterImpl,
    callback: Arc<Box<dyn Fn(OrderStatusEvent) + Send + Sync>>,
    is_running: Arc<AtomicBool>,
}

impl InteractiveBrokerAdapter {
    /// Creates a new InteractiveBrokerAdapter instance
    pub fn new(config: BrokerConfig) -> Result<Self, Error> {
        Ok(InteractiveBrokerAdapter {
            base: BrokerAdapterImpl::new(config)?,
            callback: Arc::new(Box::new(|_| {})),
            is_running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Validates an order before submission
    fn validate_order(order: &Order) -> Result<(), Error> {
        if order.size <= 0.0 {
            return Err(Error::ValidationError("Order size must be positive".to_string()));
        }
         if let OrderType::Limit = order.order_type {
            if order.price <= 0.0 {
                return Err(Error::ValidationError("Limit price must be positive".to_string()));
            }
        }
        if order.symbol.is_empty() {
            return Err(Error::ValidationError("Symbol cannot be empty".to_string()));
        }
        Ok(())
    }
}

#[async_trait]
impl BrokerPort for InteractiveBrokerAdapter {
    type OrderStatusType = OrderStatusEvent;
     type OrderType = Order;

    async fn place_order(&self, order: Self::OrderType) -> Result<(), Error> {
        if !self.is_running.load(Ordering::SeqCst) {
            return Err(Error::OperationError("Broker adapter not started".to_string()));
        }

        Self::validate_order(&order)?;

        let order_id = Uuid::new_v4().to_string();
        info!(
            "Placing order {:?} {} of {} (ID: {})",
            order.side, order.size, order.symbol, order_id
        );

         let accepted_event = OrderStatusEvent {
            order_id: order_id.clone(),
            status: OrderStatus::New,
            filled_size: 0.0,
            remaining_size: order.size,
            fill_price: 0.0,
            reason: String::new(),
        };

        let callback = Arc::clone(&self.callback);
        (callback)(accepted_event);

        // Simulate order fill after delay
        let callback = Arc::clone(&self.callback);
        tokio::spawn(async move {
            sleep(Duration::from_secs(2)).await;

             let filled_event = OrderStatusEvent {
                order_id,
                status: OrderStatus::Filled,
                filled_size: order.size,
                remaining_size: 0.0,
                 fill_price: match order.order_type {
                    OrderType::Limit => order.price,
                   _ =>  9999.99
                 },
                reason: String::new(),
            };

            info!("Order filled: {:?}", filled_event);
            (callback)(filled_event);
        });

        Ok(())
    }

    async fn get_order_status(&self) -> Result<Option<OrderStatusEvent>, Error> {
         if !self.is_running.load(Ordering::SeqCst) {
            return Err(Error::OperationError("Broker adapter not started".to_string()));
        }
        // Mock implementation - could be extended to track actual orders
        Ok(None)
    }

    async fn start(&self, callback: fn(OrderStatusEvent)) -> Result<(), Error> {
        info!("Starting Interactive Broker adapter");
        self.is_running.store(true, Ordering::SeqCst);
        self.callback = Arc::new(Box::new(callback));
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        info!("Stopping Interactive Broker adapter");
        self.is_running.store(false, Ordering::SeqCst);
        Ok(())
    }
     fn get_sink_topics(&self) -> Vec<String> {
        self.base.get_sink_topics()
    }
}
 impl BaseBrokerAdapter<Order, OrderStatusEvent> for InteractiveBrokerAdapter {
    fn get_broker_api(&self) -> String {
        self.base.get_broker_api()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
         let config = BrokerConfig {
             host: "api.interactivebrokers.com".to_string(),
             port: 8080,
             client_id: 1,
             account_id: "".to_string(),
             paper_trading: false,
             max_retries: 3,
             timeout: 5000
         };
        let adapter = InteractiveBrokerAdapter::new(config);
        assert!(adapter.is_ok());
    }

    #[test]
    fn test_order_validation() {
        // Valid order
        let valid_order = Order {
            symbol: "BTC-USD".to_string(),
            size: 1.0,
            side: TradeSide::Buy,
             order_type: OrderType::Limit,
            price: 50000.0,
        };
        assert!(InteractiveBrokerAdapter::validate_order(&valid_order).is_ok());

        // Invalid size
        let invalid_size = Order {
            symbol: "BTC-USD".to_string(),
            size: 0.0,
            side: TradeSide::Buy,
             order_type: OrderType::Limit,
            price: 50000.0,
        };
        assert!(InteractiveBrokerAdapter::validate_order(&invalid_size).is_err());

        // Invalid price
          let invalid_price = Order {
            symbol: "BTC-USD".to_string(),
            size: 1.0,
            side: TradeSide::Buy,
             order_type: OrderType::Limit,
            price: 0.0,
        };
        assert!(InteractiveBrokerAdapter::validate_order(&invalid_price).is_err());
    }
}
