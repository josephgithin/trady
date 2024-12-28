use async_trait::async_trait;
use std::fmt::Debug;
use crate::common::types::Error;
use crate::domain::models::StrategyConfig;

/// Event bus interface for publishing and subscribing to events
#[async_trait]
pub trait EventBusPort: Send + Sync + Debug {
    type EventType: Send + Sync + Debug;

    async fn publish(&self, event: Self::EventType, topic: String) -> Result<(), Error>;
    
    // Change these to accept boxed closures instead of fn pointers
    async fn subscribe<F>(&self, topic: String, callback: F) -> Result<(), Error>
    where
        F: FnMut(Self::EventType) + Send + 'static;
    
    async fn subscribe_many<F>(&self, topics: Vec<String>, callback: F) -> Result<(), Error>
    where
        F: FnMut(Self::EventType) + Send + 'static;

    async fn shutdown(&self) -> Result<(), Error>;
}

/// Data adapter interface for connecting to and processing data from external sources
#[async_trait]
pub trait DataAdapterPort<T>: Send + Sync + Debug {
    async fn connect(&self) -> Result<(), Error>;
    async fn process_message(&self) -> Result<Option<T>, Error>;
    async fn start<F>(&self, callback: F) -> Result<(), Error>
    where
        F: FnMut(T) + Send + 'static;
    async fn stop(&self) -> Result<(), Error>;
    fn get_sink_topics(&self) -> Vec<String>;
}

/// Trading strategy interface for analyzing market data and generating signals
#[async_trait]
pub trait StrategyPort: Send + Sync + Debug {
    type EventType: Send + Sync + Debug;
    type TradeSignalType: Send + Sync + Debug;

    async fn analyze_market_data(&self, event: Self::EventType) -> Result<Vec<Self::TradeSignalType>, Error>;
    async fn start<F>(&self, callback: F) -> Result<(), Error>
    where
        F: FnMut(Self::TradeSignalType) + Send + 'static;
    async fn stop(&self) -> Result<(), Error>;
    async fn configure_strategy(&mut self, config: StrategyConfig) -> Result<(), Error>;
    async fn get_strategy_config(&self) -> Result<StrategyConfig, Error>;
    fn get_sink_topics(&self) -> Vec<String>;
    fn get_source_topics(&self) -> Vec<String>;
}

/// Broker interface for placing orders and managing order status
#[async_trait]
pub trait BrokerPort: Send + Sync + Debug {
    type OrderStatusType: Send + Sync + Debug;
    type OrderType: Send + Sync + Debug;

    /// Places a new order with the broker
    async fn place_order(&self, order: Self::OrderType) -> Result<(), Error>;

    /// Retrieves the current status of an order
    async fn get_order_status(&self) -> Result<Option<Self::OrderStatusType>, Error>;

    /// Starts the broker connection with status callback
    async fn start(&self, callback: Box<dyn Fn(Self::OrderStatusType) + Send + Sync>) -> Result<(), Error>;

    /// Stops the broker connection
    async fn stop(&self) -> Result<(), Error>;
    /// Returns the list of sink topics
    fn get_sink_topics(&self) -> Vec<String>;
}

/// Persistence interface for storing and retrieving data
#[async_trait]
pub trait PersistenceManager: Send + Sync + Debug {
    /// Saves data with the specified key
    async fn save_data(&self, key: String, data: String) -> Result<(), Error>;

    /// Retrieves data for the specified key
    async fn get_data(&self, key: String) -> Result<Option<String>, Error>;
}

/// Security interface for authentication, authorization and encryption
#[async_trait]
pub trait SecurityLayer: Send + Sync + Debug {
    /// Authenticates a user
    async fn auth_user(&self, user: String) -> Result<bool, Error>;

    /// Authorizes a user for a specific action
    async fn authorize_user(&self, user: String, action: String) -> Result<bool, Error>;

    /// Encrypts the provided data
    async fn encrypt(&self, data: String) -> Result<String, Error>;

    /// Starts the security layer
    async fn start(&self) -> Result<(), Error>;

    /// Stops the security layer
    async fn stop(&self) -> Result<(), Error>;
}

/// Generic subscriber interface for handling events
#[async_trait]
pub trait Subscriber<T: Send + Sync + Debug>: Send + Sync + Debug {
    /// Handles an incoming event
    async fn on_event(&self, event: T);

    /// Subscribes to events with a callback
    async fn subscribe(&self, callback: fn(T)) -> Result<(), Error>;

    /// Returns the list of source topics
    fn get_source_topics(&self) -> Vec<String>;

     /// Starts the subscriber
    async fn start(&self) -> Result<(), Error>;

    /// Stops the subscriber
    async fn stop(&self) -> Result<(), Error>;
}
