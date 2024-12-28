use std::sync::Arc;
use async_trait::async_trait;
use log::{error, info, warn};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};

use crate::common::types::Error;
use crate::domain::events::DomainEvent;
use crate::domain::interfaces::Subscriber;

const CHANNEL_SIZE: usize = 1000;
const PROCESSING_INTERVAL_MS: u64 = 100;

/// Subscriber that handles audit logging of domain events
#[derive(Debug)]
pub struct AuditSubscriber {
    topics: Vec<String>,
    callback: Arc<Mutex<Option<fn(DomainEvent)>>>,
    tx: Arc<Mutex<Option<mpsc::Sender<DomainEvent>>>>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl AuditSubscriber {
    /// Creates a new AuditSubscriber instance
    pub fn new(topics: Vec<String>) -> Result<Self, Error> {
        if topics.is_empty() {
            return Err(Error::ValidationError("Topics list cannot be empty".to_string()));
        }

        for topic in &topics {
            if topic.is_empty() {
                return Err(Error::ValidationError("Topic cannot be empty".to_string()));
            }
        }

        Ok(AuditSubscriber {
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
}

#[async_trait]
impl Subscriber<DomainEvent> for AuditSubscriber {
 /// Handles incoming events
 async fn on_event(&self, event: DomainEvent) {
     if !self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
         warn!("Audit subscriber not running, dropping event");
         return;
     }
 
     if let Some(tx) = self.tx.lock().await.as_ref() {
         match tx.send(event.clone()).await {
             Ok(_) => {
                 info!("Audit received: {:?}", event);
             }
             Err(e) => {
                 error!("Failed to queue event: {}", e);
             }
         }
     }
 }
 
 /// Subscribes to events with a callback
 fn subscribe(&self, callback: fn(DomainEvent)) -> Result<(), Error> {
     if self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
         return Err(Error::OperationError("Cannot subscribe while running".to_string()));
     }
 
     let mut cb = self.callback.lock().blocking_unwrap();
     *cb = Some(callback);
     
     info!("Audit subscribed with callback");
     Ok(())
 }
 
 /// Returns the list of topics this subscriber is interested in
 fn get_source_topics(&self) -> Vec<String> {
     self.topics.clone()
 }
 
 /// Starts the audit subscriber
 async fn start(&self) -> Result<(), Error> {
     info!("Starting audit logger");
 
     if self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
         return Err(Error::OperationError("Audit subscriber already running".to_string()));
     }
 
     if self.callback.lock().await.is_none() {
         return Err(Error::OperationError("Callback not set".to_string()));
     }
 
     let (tx, mut rx) = mpsc::channel(CHANNEL_SIZE);
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
         info!("Audit processing loop stopped");
     });
 
     Ok(())
 }
 
 /// Stops the audit subscriber
 async fn stop(&self) -> Result<(), Error> {
     info!("Stopping audit logger");
 
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
 fn test_audit_subscriber_creation() {
     // Valid topics
     let topics = vec!["topic1".to_string(), "topic2".to_string()];
     assert!(AuditSubscriber::new(topics).is_ok());
 
     // Empty topics list
     let empty_topics: Vec<String> = vec![];
     assert!(AuditSubscriber::new(empty_topics).is_err());
 
     // Topics with empty string
     let invalid_topics = vec!["topic1".to_string(), "".to_string()];
     assert!(AuditSubscriber::new(invalid_topics).is_err());
 }
 
 #[tokio::test]
 async fn test_subscriber_lifecycle() {
     let topics = vec!["test".to_string()];
     let subscriber = AuditSubscriber::new(topics).unwrap();
 
     // Should fail without callback
     assert!(subscriber.start().await.is_err());
 
     // Set callback
     subscriber.subscribe(|_| {}).unwrap();
 
     // Start
     assert!(subscriber.start().await.is_ok());
     
     // Should fail to subscribe while running
     assert!(subscriber.subscribe(|_| {}).is_err());
 
     // Stop
     assert!(subscriber.stop().await.is_ok());
 }
}
EOF"
