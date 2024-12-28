use std::io::{self, BufRead};
use std::sync::Arc;
use async_trait::async_trait;
use log::{error, info, warn};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};

use crate::common::types::Error;
use crate::domain::events::{DomainEvent, UserInputEvent};
use crate::domain::interfaces::Subscriber;

const INPUT_CHANNEL_SIZE: usize = 100;
const DISPLAY_REFRESH_MS: u64 = 100;
const INPUT_CHECK_MS: u64 = 50;

/// Console-based user interface implementation
#[derive(Debug)]
pub struct ConsoleUI {
    topics: Vec<String>,
    callback: Arc<Mutex<Option<fn(DomainEvent)>>>,
    tx: Arc<Mutex<Option<mpsc::Sender<DomainEvent>>>>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl ConsoleUI {
    /// Creates a new ConsoleUI instance
    pub fn new(topics: Vec<String>) -> Result<Self, Error> {
        if topics.is_empty() {
            return Err(Error::ValidationError("Topics list cannot be empty".to_string()));
        }

        Ok(ConsoleUI {
            topics,
            callback: Arc::new(Mutex::new(None)),
            tx: Arc::new(Mutex::new(None)),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// Handles user input from console
    async fn handle_input(&self) -> Result<(), Error> {
        let is_running = Arc::clone(&self.is_running);
        let tx = Arc::clone(&self.tx);

        tokio::task::spawn_blocking(move || {
            let stdin = io::stdin();
            let reader = stdin.lock();

            for line in reader.lines() {
                 if !is_running.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }

                if let Ok(input) = line {
                   let event = match input.trim() {
                        "q" | "quit" => Some(DomainEvent::UserInput(UserInputEvent {
                            action: "quit".to_string(),
                            value: None,
                        })),
                        cmd if cmd.starts_with("buy ") => {
                            let parts: Vec<&str> = cmd.split_whitespace().collect();
                            if parts.len() == 2 {
                                Some(DomainEvent::UserInput(UserInputEvent {
                                    action: "buy".to_string(),
                                    value: Some(parts[1].to_string()),
                                }))
                            } else {
                                warn!("Invalid buy command format");
                                None
                            }
                        }
                        cmd if cmd.starts_with("sell ") => {
                             let parts: Vec<&str> = cmd.split_whitespace().collect();
                            if parts.len() == 2 {
                                Some(DomainEvent::UserInput(UserInputEvent {
                                    action: "sell".to_string(),
                                    value: Some(parts[1].to_string()),
                                }))
                             } else {
                                warn!("Invalid sell command format");
                                None
                            }
                        }
                        _ => {
                            warn!("Unknown command: {}", input);
                            None
                        }
                    };

                    if let Some(event) = event {
                      if let Some(tx) = tx.blocking_lock().as_ref() {
                          if let Err(e) = tx.blocking_send(event) {
                              error!("Failed to send command: {}", e);
                           }
                        }
                     }
                 }
             }
         });

        Ok(())
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
impl Subscriber<DomainEvent> for ConsoleUI {
    /// Handles incoming events
    async fn on_event(&self, event: DomainEvent) {
      if !self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
          warn!("Console UI not running, dropping event");
          return;
      }

      match event {
          DomainEvent::UserInput(_) => {}, // Don't echo back user input
          _ => println!(">> {:?}", event),
      }
    }

    /// Subscribes to events with a callback
    async fn subscribe(&self, callback: fn(DomainEvent)) -> Result<(), Error> {
       if self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
          return Err(Error::OperationError("Cannot subscribe while running".to_string()));
      }

      let mut cb = self.callback.lock().await;
       *cb = Some(callback);

      info!("Console UI subscribed with callback");
        Ok(())
    }

    /// Returns the list of topics this subscriber is interested in
    fn get_source_topics(&self) -> Vec<String> {
        self.topics.clone()
    }

    /// Starts the console UI
    async fn start(&self) -> Result<(), Error> {
         info!("Starting Console UI");

        if self.is_running.load(std::sync::atomic::Ordering::SeqCst) {
           return Err(Error::OperationError("Console UI already running".to_string()));
         }

         if self.callback.lock().await.is_none() {
             return Err(Error::OperationError("Callback not set".to_string()));
        }

         let (tx, mut rx) = mpsc::channel(INPUT_CHANNEL_SIZE);
        *self.tx.lock().await = Some(tx);

         self.is_running.store(true, std::sync::atomic::Ordering::SeqCst);

         // Start input handler
        self.handle_input().await?;

        // Start event processing loop
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            while is_running.load(std::sync::atomic::Ordering::SeqCst) {
                if let Ok(event) = rx.try_recv() {
                  if let Err(e) = self.process_event(event).await {
                       error!("Failed to process event: {}", e);
                    }
                }
              sleep(Duration::from_millis(DISPLAY_REFRESH_MS)).await;
             }
             info!("Console UI processing loop stopped");
         });

        println!("Console UI started. Available commands:");
        println!("  buy <amount>  - Place a buy order");
        println!("  sell <amount> - Place a sell order");
        println!("  q or quit     - Exit the application");

        Ok(())
    }

    /// Stops the console UI
    async fn stop(&self) -> Result<(), Error> {
        info!("Stopping Console UI");

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
    fn test_console_ui_creation() {
        // Valid topics
        let topics = vec!["topic1".to_string()];
        assert!(ConsoleUI::new(topics).is_ok());

        // Empty topics list
        let empty_topics: Vec<String> = vec![];
        assert!(ConsoleUI::new(empty_topics).is_err());
    }

    #[tokio::test]
    async fn test_ui_lifecycle() {
        let topics = vec!["test".to_string()];
        let ui = ConsoleUI::new(topics).unwrap();

        // Should fail without callback
         assert!(ui.start().await.is_err());

        // Set callback
          ui.subscribe(|_| {}).await.unwrap();

        // Start
        assert!(ui.start().await.is_ok());

        // Should fail to subscribe while running
       assert!(ui.subscribe(|_| {}).await.is_err());

        // Stop
        assert!(ui.stop().await.is_ok());
    }
}
