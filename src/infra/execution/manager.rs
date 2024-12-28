use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use log::{error, info, warn};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::common::types::Error;
use crate::domain::events::{TradeSignalEvent, OrderStatusEvent, OrderStatus};
use crate::domain::interfaces::BrokerPort;
use crate::domain::models::Order;
use crate::common::enums::{OrderType, TradeSide};
const MAX_ORDER_SIZE: f64 = 10.0;
const MIN_ORDER_SIZE: f64 = 0.0001;

/// Manages order execution and broker interactions
#[derive(Debug)]
pub struct ExecutionManager {
    broker: Arc<Mutex<Option<Box<dyn BrokerPort<OrderStatusType = OrderStatusEvent, OrderType = Order> + Send + Sync>>>>,
    active_orders: Arc<Mutex<HashMap<String, Order>>>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl ExecutionManager {
    /// Creates a new ExecutionManager instance
    pub fn new(broker: Box<dyn BrokerPort<OrderStatusType = OrderStatusEvent, OrderType = Order> + Send + Sync>) -> Self {
        ExecutionManager {
            broker: Arc::new(Mutex::new(Some(broker))),
            active_orders: Arc::new(Mutex::new(HashMap::new())),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Validates a trade signal
    fn validate_trade_signal(signal: &TradeSignalEvent) -> Result<(), Error> {
        if signal.symbol.is_empty() {
            return Err(Error::ValidationError("Symbol cannot be empty".to_string()));
        }

        if signal.size <= MIN_ORDER_SIZE {
            return Err(Error::ValidationError(format!("Order size must be greater than {}", MIN_ORDER_SIZE)));
        }

        if signal.size > MAX_ORDER_SIZE {
            return Err(Error::ValidationError(format!("Order size cannot exceed {}", MAX_ORDER_SIZE)));
        }

         if signal.side != TradeSide::Buy && signal.side != TradeSide::Sell {
            return Err(Error::ValidationError("Invalid order side".to_string()));
        }

        Ok(())
    }

    /// Tracks a new order
    async fn track_order(&self, order: Order) -> String {
        let order_id = Uuid::new_v4().to_string();
        self.active_orders.lock().await.insert(order_id.clone(), order);
        order_id
    }

    /// Updates order status
    async fn update_order_status(&self, order_id: &str, status: &OrderStatusEvent) {
        let mut orders = self.active_orders.lock().await;
        if status.status == OrderStatus::Filled || status.status == OrderStatus::Cancelled {
            orders.remove(order_id);
        }
    }
}

impl ExecutionManager {
    /// Handles incoming trade signals
    pub async fn handle_trade_signal(&self, event: TradeSignalEvent) -> Result<(), Error> {
       if !self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(Error::OperationError("Execution manager not running".to_string()));
        }

        Self::validate_trade_signal(&event)?;

        let broker_guard = self.broker.lock().await;
        let broker = broker_guard.as_ref()
            .ok_or_else(|| Error::OperationError("Broker not available".to_string()))?;

        let order = Order {
            symbol: event.symbol,
            side: event.side,
            size: event.size,
            price: 0.0,
            order_type: OrderType::Market,
        };

        let order_id = self.track_order(order.clone()).await;

        match broker.place_order(order).await {
            Ok(_) => {
                info!("Order placed successfully: {}", order_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to place order {}: {}", order_id, e);
                self.active_orders.lock().await.remove(&order_id);
                Err(e)
            }
        }
    }

    /// Starts the execution manager
    pub async fn start<F>(&self, callback: F) -> Result<(), Error> 
    where
        F: Fn(OrderStatusEvent) + Send + Sync + 'static
    {
        info!("Starting execution manager");

        if self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(Error::OperationError("Execution manager already running".to_string()));
        }

        let broker_guard = self.broker.lock().await;
        let broker = broker_guard.as_ref()
            .ok_or_else(|| Error::OperationError("Broker not available".to_string()))?;

         let active_orders = Arc::clone(&self.active_orders);
         let wrapped_callback = move |status: OrderStatusEvent| {
             let active_orders = Arc::clone(&active_orders);
             if !status.order_id.is_empty() {
                 let order_id = status.order_id.clone();  // Clone the order_id before moving
                 tokio::spawn(async move {
                     active_orders.lock().await.remove(&order_id);
                 });
             }
             callback(status);
         };
         
         broker.start(Box::new(wrapped_callback)).await?;
        self.is_running.store(true, std::sync::atomic::Ordering::SeqCst);

        Ok(())
    }

    /// Stops the execution manager
    pub async fn stop(&self) -> Result<(), Error> {
        info!("Stopping execution manager");

        if !self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(());
        }

        let broker_guard = self.broker.lock().await;
        if let Some(broker) = broker_guard.as_ref() {
            broker.stop().await?;
        }

        self.is_running.store(false, std::sync::atomic::Ordering::SeqCst);

         let active_orders = self.active_orders.lock().await.len();
          if active_orders > 0 {
              warn!("{} orders still active during shutdown", active_orders);
           }

        Ok(())
    }

    /// Gets the count of active orders
    pub async fn get_active_orders_count(&self) -> usize {
        self.active_orders.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_trade_signal() {
        // Valid signal
        let valid_signal = TradeSignalEvent {
            symbol: "BTC-USD".to_string(),
            side: TradeSide::Buy,
            size: 1.0,
            strategy: crate::common::enums::StrategyName::Arbitrage
        };
        assert!(ExecutionManager::validate_trade_signal(&valid_signal).is_ok());

        // Invalid size
        let invalid_size = TradeSignalEvent {
            symbol: "BTC-USD".to_string(),
             side: TradeSide::Buy,
            size: 0.0,
             strategy: crate::common::enums::StrategyName::Arbitrage
        };
        assert!(ExecutionManager::validate_trade_signal(&invalid_size).is_err());

        // Invalid side
        let invalid_side = TradeSignalEvent {
            symbol: "BTC-USD".to_string(),
            side: TradeSide::Buy,
            size: 1.0,
             strategy: crate::common::enums::StrategyName::Arbitrage
        };
        assert!(ExecutionManager::validate_trade_signal(&invalid_side).is_ok());
         let invalid_side = TradeSignalEvent {
            symbol: "BTC-USD".to_string(),
            side:  TradeSide::Sell,
            size: 1.0,
             strategy: crate::common::enums::StrategyName::Arbitrage
        };
          assert!(ExecutionManager::validate_trade_signal(&invalid_side).is_ok());
          let invalid_side = TradeSignalEvent {
            symbol: "BTC-USD".to_string(),
             side:  TradeSide::Buy,
            size: 1.0,
             strategy: crate::common::enums::StrategyName::Arbitrage
        };
         assert!(ExecutionManager::validate_trade_signal(&invalid_side).is_ok());
    }
}
