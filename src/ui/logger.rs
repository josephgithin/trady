use std::sync::Arc;
use async_trait::async_trait;
use log::{error, info, warn};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};

use crate::common::types::Error;
use crate::domain::events::DomainEvent;
use crate::domain::interfaces::Subscriber;

const EVENT_CHANNEL_SIZE: usize = 1000;
const PROCESSING_INTERVAL_MS: u64 = 100;

/// Event logger implementation
#[derive(Debug)]
pub struct Logger {
    topics: Vec<String>,
    callback: Arc<Mutex<Option<fn(DomainEvent)>>>,
    tx: Arc<Mutex<Option<mpsc::Sender<DomainEvent>>>>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl Logger {
    /// Creates a new Logger instance
    pub fn new(topics: Vec<String>) -> Result<Self, Error> {
        if topics.is_empty() {
            return Err(Error::ValidationError("Topics list cannot be empty".to_string()));
        }

        for topic in &topics {
            if topic.is_empty() {
                return Err(Error::ValidationError("Topic cannot be empty".to_string()));
            }
        }

        Ok(Logger {
            topics,
            callback: Arc::new(Mutex::new(None)),
            tx: Arc::new(Mutex::new(None)),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// Processes a single event
    async fn process_event(&self, event: DomainEvent) -> Result<(), Error> {
        if let Some(callback) = *self.callback.lock().await {
            callback(event);
            Ok(())
        } else {
            Err(Error::OperationError("Callback not set".to_string()))
        }
    }

    /// Formats an event for logging
    fn format_event(&self, event: &DomainEvent) -> String {
        match event {
            DomainEvent::UserInput(e) => format!("User Input - Action: {}, Value: {:?}", e.action, e.value),
            DomainEvent::OrderStatus(e) => format!("Order Status - ID: {:?}, Status: {}", e.order_id, e.status),
            DomainEvent::TradeSignal(e) => format!("Trade Signal - Symbol: {}, Side: {}, Size: {}", e.symbol, e.side, e.size),
            // Add other event types as needed
        }
    }
}

#[async_trait]
impl Subscriber<DomainEvent> for Logger {
 /// Handles incoming events
 async fn on_event(&self, event: DomainEvent) {
     if !self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
         warn!("Logger not running, dropping event");
         return;
     }
 
     let formatted = self.format_event(&event);
     info!("Event: {}", formatted);
 
     if let Some(tx) = self.tx.lock().await.as_ref() {
         if let Err(e) = tx.send(event).await {
             error!("Failed to queue event: {}", e);
         }
     }
 }
 
 /// Subscribes to events with a callback
 fn subscribe(&self, callback: fn(DomainEvent)) -> Result<(), Error> {
     if self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
         return Err(Error::OperationError("Cannot subscribe while running".to_string()));
     }
 
     let mut cb = self.callback.blocking_lock();
     *cb = Some(callback);
     
     info!("Logger subscribed with callback");
     Ok(())
 }
 
 /// Returns the list of topics this subscriber is interested in
 fn get_source_topics(&self) -> Vec<String> {
     self.topics.clone()
 }
 
 /// Starts the logger
 async fn start(&self) -> Result<(), Error> {
     info!("Starting Logger");
 
     if self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
         return Err(Error::OperationError("Logger already running".to_string()));
     }
 
     if self.callback.lock().await.is_none() {
         return Err(Error::OperationError("Callback not set".to_string()));
     }
 
     let (tx, mut rx) = mpsc::channel(EVENT_CHANNEL_SIZE);
     *self.tx.lock().await = Some(tx);
 
     self.is_running.store(true, std::sync::atomic::Ordering::SeqCst);
 
     let is_running = Arc::clone(&self.is_running);
     
     tokio::spawn(async move {
         while is_running.load(std::sync::atomic::Ordering::SeqCst) {
             if let Ok(event) = rx.try_recv() {
                 if let Err(e) = self.process_event(event).await {
                     error!("Failed to process event: {}", e);
                 }
             }
             sleep(Duration::from_millis(PROCESSING_INTERVAL_MS)).await;
         }
         info!("Logger processing loop stopped");
     });
 
     Ok(())
 }
 
 /// Stops the logger
 async fn stop(&self) -> Result<(), Error> {
     info!("Stopping Logger");
 
     if !self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
         return Ok(());
     }
 
     self.is_running.store(false, std::sync::atomic::Ordering::SeqCst);
     *self.tx.lock().await = None;
 
     Ok(())
 }
 }
 
 #[cfg(test)]
 mod tests {
 use super::*;
 
 #[test]
 fn test_logger_creation() {
     // Valid topics
     let topics = vec!["topic1".to_string(), "topic2".to_string()];
     assert!(Logger::new(topics).is_ok());
 
     // Empty topics list
     let empty_topics: Vec<String> = vec![];
     assert!(Logger::new(empty_topics).is_err());
 
     // Topics with empty string
     let invalid_topics = vec!["topic1".to_string(), "".to_string()];
     assert!(Logger::new(invalid_topics).is_err());
 }
 
 #[tokio::test]
 async fn test_logger_lifecycle() {
     let topics = vec!["test".to_string()];
     let logger = Logger::new(topics).unwrap();
 
     // Should fail without callback
     assert!(logger.start().await.is_err());
 
     // Set callback
     logger.subscribe(|_| {}).unwrap();
 
     // Start
     assert!(logger.start().await.is_ok());
     
     // Should fail to subscribe while running
     assert!(logger.subscribe(|_| {}).is_err());
 
     // Stop
     assert!(logger.stop().await.is_ok());
 }
}
EOF"
