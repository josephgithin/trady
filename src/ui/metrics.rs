use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;
use log::{error, info, warn};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};

use crate::common::types::Error;
use crate::domain::events::DomainEvent;
use crate::domain::interfaces::Subscriber;

const METRICS_CHANNEL_SIZE: usize = 1000;
const METRICS_UPDATE_MS: u64 = 100;
const METRICS_REPORT_MS: u64 = 5000; // Report every 5 seconds

/// Metrics collector and reporter
#[derive(Debug)]
pub struct MetricsSubscriber {
    topics: Vec<String>,
    callback: Arc<Mutex<Option<fn(DomainEvent)>>>,
    tx: Arc<Mutex<Option<mpsc::Sender<DomainEvent>>>>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
    metrics: Arc<Mutex<MetricsData>>,
}

#[derive(Debug, Default)]
struct MetricsData {
    event_counts: HashMap<String, u64>,
    order_counts: HashMap<String, u64>,
    trade_volume: f64,
    last_report_time: std::time::Instant,
}

impl MetricsSubscriber {
    /// Creates a new MetricsSubscriber instance
    pub fn new(topics: Vec<String>) -> Result<Self, Error> {
        if topics.is_empty() {
            return Err(Error::ValidationError("Topics list cannot be empty".to_string()));
        }

        for topic in &topics {
            if topic.is_empty() {
                return Err(Error::ValidationError("Topic cannot be empty".to_string()));
            }
        }

        Ok(MetricsSubscriber {
            topics,
            callback: Arc::new(Mutex::new(None)),
            tx: Arc::new(Mutex::new(None)),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            metrics: Arc::new(Mutex::new(MetricsData::default())),
        })
    }

    /// Updates metrics based on an event
    async fn update_metrics(&self, event: &DomainEvent) {
        let mut metrics = self.metrics.lock().await;

        // Update event counts
        let event_type = format!("{:?}", event);
        *metrics.event_counts.entry(event_type).or_insert(0) += 1;

        // Update specific metrics based on event type
         match event {
            DomainEvent::OrderStatus(e) => {
               *metrics.order_counts.entry(format!("{:?}", e.status)).or_insert(0) += 1;
            }
            DomainEvent::TradeSignal(e) => {
                metrics.trade_volume += e.size;
            }
             _ => {}
        }

         // Report metrics periodically
         if metrics.last_report_time.elapsed() >= Duration::from_millis(METRICS_REPORT_MS) {
            self.report_metrics(&metrics).await;
            metrics.last_report_time = std::time::Instant::now();
         }
    }

    /// Reports current metrics
    async fn report_metrics(&self, metrics: &MetricsData) {
        info!("=== Metrics Report ===");
        info!("Event Counts:");
        for (event_type, count) in &metrics.event_counts {
            info!("  {}: {}", event_type, count);
        }
       info!("Order Status Counts:");
        for (status, count) in &metrics.order_counts {
            info!("  {}: {}", status, count);
        }
        info!("Total Trade Volume: {}", metrics.trade_volume);
        info!("====================");
    }

    /// Processes a single event
    async fn process_event(&self, event: DomainEvent) -> Result<(), Error> {
         self.update_metrics(&event).await;

         if let Some(callback) = *self.callback.lock().await {
           callback(event);
           Ok(())
        } else {
           Err(Error::OperationError("Callback not set".to_string()))
         }
     }
}

#[async_trait]
impl Subscriber<DomainEvent> for MetricsSubscriber {
    /// Handles incoming events
    async fn on_event(&self, event: DomainEvent) {
         if !self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
             warn!("Metrics subscriber not running, dropping event");
             return;
         }

        if let Some(tx) = self.tx.lock().await.as_ref() {
            if let Err(e) = tx.send(event).await {
                error!("Failed to queue event: {}", e);
             }
        }
    }

    /// Subscribes to events with a callback
    async fn subscribe(&self, callback: fn(DomainEvent)) -> Result<(), Error> {
         if self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(Error::OperationError("Cannot subscribe while running".to_string()));
        }

         let mut cb = self.callback.lock().await;
          *cb = Some(callback);

       info!("Metrics subscriber subscribed with callback");
        Ok(())
    }

    /// Returns the list of topics this subscriber is interested in
    fn get_source_topics(&self) -> Vec<String> {
        self.topics.clone()
    }

    /// Starts the metrics subscriber
    async fn start(&self) -> Result<(), Error> {
       info!("Starting metrics subscriber");

       if self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(Error::OperationError("Metrics subscriber already running".to_string()));
       }

        if self.callback.lock().await.is_none() {
            return Err(Error::OperationError("Callback not set".to_string()));
        }

        let (tx, mut rx) = mpsc::channel(METRICS_CHANNEL_SIZE);
        *self.tx.lock().await = Some(tx);

        self.is_running.store(true, std::sync::atomic::Ordering::SeqCst);

          // Reset metrics
        let mut metrics = self.metrics.lock().await;
        *metrics = MetricsData::default();
         metrics.last_report_time = std::time::Instant::now();
         drop(metrics);
         let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
           while is_running.load(std::sync::atomic::Ordering::SeqCst) {
                if let Ok(event) = rx.try_recv() {
                    if let Err(e) = self.process_event(event).await {
                         error!("Failed to process event: {}", e);
                     }
                 }
               sleep(Duration::from_millis(METRICS_UPDATE_MS)).await;
              }
             info!("Metrics processing loop stopped");
         });

        Ok(())
    }

    /// Stops the metrics subscriber
    async fn stop(&self) -> Result<(), Error> {
         info!("Stopping metrics subscriber");

        if !self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
           return Ok(());
       }

        self.is_running.store(false, std::sync::atomic::Ordering::SeqCst);

       // Final metrics report
        let metrics = self.metrics.lock().await;
         self.report_metrics(&metrics).await;

        *self.tx.lock().await = None;

         Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_subscriber_creation() {
        // Valid topics
        let topics = vec!["topic1".to_string(), "topic2".to_string()];
        assert!(MetricsSubscriber::new(topics).is_ok());

        // Empty topics list
        let empty_topics: Vec<String> = vec![];
        assert!(MetricsSubscriber::new(empty_topics).is_err());

        // Topics with empty string
        let invalid_topics = vec!["topic1".to_string(), "".to_string()];
        assert!(MetricsSubscriber::new(invalid_topics).is_err());
    }

    #[tokio::test]
    async fn test_metrics_lifecycle() {
        let topics = vec!["test".to_string()];
        let metrics = MetricsSubscriber::new(topics).unwrap();

        // Should fail without callback
        assert!(metrics.start().await.is_err());

        // Set callback
          metrics.subscribe(|_| {}).await.unwrap();

        // Start
        assert!(metrics.start().await.is_ok());

        // Should fail to subscribe while running
        assert!(metrics.subscribe(|_| {}).await.is_err());

        // Stop
        assert!(metrics.stop().await.is_ok());
    }
}
