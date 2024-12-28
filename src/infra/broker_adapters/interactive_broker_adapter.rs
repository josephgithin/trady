use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use async_trait::async_trait;
use log::{error, info};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::common::types::Error;
use crate::config::BrokerConfig;
use crate::domain::events::{OrderStatusEvent, OrderStatus};
use crate::domain::interfaces::BrokerPort;
use crate::domain::models::Order;
use crate::infra::broker_adapters::base::{BaseBrokerAdapter, BrokerAdapterImpl};
use crate::common::enums::OrderType;

/// Interactive Broker adapter implementation
#[derive(Debug)]
pub struct InteractiveBrokerAdapter {
    base: BrokerAdapterImpl,
    callback: Arc<Mutex<Option<Box<dyn FnMut(OrderStatusEvent) + Send + 'static>>>>,
    is_running: Arc<AtomicBool>,
}

impl InteractiveBrokerAdapter {
    /// Creates a new InteractiveBrokerAdapter instance
    pub fn new(config: BrokerConfig) -> Result<Self, Error> {
        Ok(InteractiveBrokerAdapter {
            base: BrokerAdapterImpl::new(config)?,
            callback: Arc::new(Mutex::new(None)),
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

    async fn notify_status(&self, event: OrderStatusEvent) {
        if let Some(mut callback) = self.callback.lock().await.as_mut() {
            callback(event);
        }
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

        self.notify_status(accepted_event).await;

        // Simulate order fill after delay
        let self_clone = Arc::new(self.clone());
        let order_clone = order.clone();
        tokio::spawn(async move {
            sleep(Duration::from_secs(2)).await;

            let filled_event = OrderStatusEvent {
                order_id,
                status: OrderStatus::Filled,
                filled_size: order_clone.size,
                remaining_size: 0.0,
                fill_price: match order_clone.order_type {
                    OrderType::Limit => order_clone.price,
                    _ => 9999.99
                },
                reason: String::new(),
            };

            info!("Order filled: {:?}", filled_event);
            self_clone.notify_status(filled_event).await;
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

    async fn start<F>(&self, callback: F) -> Result<(), Error>
    where
        F: FnMut(OrderStatusEvent) + Send + 'static,
    {
        info!("Starting Interactive Broker adapter");
        self.is_running.store(true, Ordering::SeqCst);
        *self.callback.lock().await = Some(Box::new(callback));
        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        info!("Stopping Interactive Broker adapter");
        self.is_running.store(false, Ordering::SeqCst);
        *self.callback.lock().await = None;
        Ok(())
    }

    fn get_sink_topics(&self) -> Vec<String> {
        self.base.get_sink_topics()
    }
}

impl BaseBrokerAdapter for InteractiveBrokerAdapter {
    type OrderType = Order;
    type OrderStatusType = OrderStatusEvent;

    fn get_broker_api(&self) -> String {
        self.base.get_broker_api()
    }
}

// Add Clone implementation to support Arc
impl Clone for InteractiveBrokerAdapter {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            callback: Arc::clone(&self.callback),
            is_running: Arc::clone(&self.is_running),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::enums::TradeSide;

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
