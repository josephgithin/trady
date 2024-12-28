use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use log::{error, info, warn};
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;
use zmq::{Context, Socket, SocketType};

use crate::common::types::Error;
use crate::domain::events::DomainEvent;
use crate::domain::interfaces::EventBusPort;

const SOCKET_TIMEOUT_MS: i32 = 5000;
const MAX_RECONNECT_ATTEMPTS: u32 = 3;

/// ZMQ implementation of the event bus
#[derive(Debug)]
pub struct ZMQEventBus {
    address: String,
    context: Arc<Context>,
    publisher: Arc<Mutex<Option<Socket>>>,
    subscriber: Arc<Mutex<Option<Socket>>>,
    router: Arc<Mutex<Option<Socket>>>,
    dealer: Arc<Mutex<Option<Socket>>>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug)]
struct SocketConfig {
    socket_type: SocketType,
    timeout_ms: i32,
    is_server: bool,
}

impl ZMQEventBus {
    /// Creates a new ZMQEventBus instance
    pub fn new(address: String) -> Result<Self, Error> {
        if address.is_empty() {
            return Err(Error::ValidationError("Address cannot be empty".to_string()));
        }

        if !address.starts_with("tcp://") && !address.starts_with("ipc://") {
            return Err(Error::ValidationError("Invalid address format".to_string()));
        }

        Ok(ZMQEventBus {
            address,
            context: Arc::new(Context::new()),
            publisher: Arc::new(Mutex::new(None)),
            subscriber: Arc::new(Mutex::new(None)),
            router: Arc::new(Mutex::new(None)),
            dealer: Arc::new(Mutex::new(None)),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        })
    }

    /// Validates a topic string
    fn validate_topic(topic: &str) -> Result<(), Error> {
        if topic.is_empty() {
            return Err(Error::ValidationError("Topic cannot be empty".to_string()));
        }
        if topic.contains(' ') {
            return Err(Error::ValidationError("Topic cannot contain spaces".to_string()));
        }
        Ok(())
    }
    async fn get_socket(&self, config: SocketConfig) -> Result<Socket, Error> {
        let socket_mutex = match config.socket_type {
            SocketType::PUB => &self.publisher,
            SocketType::SUB => &self.subscriber,
            SocketType::ROUTER => &self.router,
            SocketType::DEALER => &self.dealer,
            _ => return Err(Error::ConfigError("Unsupported socket type".to_string())),
        };

        let socket_guard = socket_mutex.lock().await;
        if let Some(socket) = &*socket_guard {
            return Ok(socket.clone());
        }
        drop(socket_guard);

        let mut socket_guard = socket_mutex.lock().await;
        let socket = self.context.socket(config.socket_type)
            .map_err(|e| Error::ConnectionError(format!("Failed to create socket: {}", e)))?;

        socket.set_rcvtimeo(config.timeout_ms)
            .map_err(|e| Error::ConfigError(format!("Failed to set receive timeout: {}", e)))?;

        socket.set_sndtimeo(config.timeout_ms)
            .map_err(|e| Error::ConfigError(format!("Failed to set send timeout: {}", e)))?;

        if config.is_server {
            socket.bind(&self.address)
                .map_err(|e| Error::ConnectionError(format!("Failed to bind socket: {}", e)))?;
        } else {
            socket.connect(&self.address)
                .map_err(|e| Error::ConnectionError(format!("Failed to connect socket: {}", e)))?;
        }

        *socket_guard = Some(socket);
        info!("{:?} socket initialized", config.socket_type);

        Ok(socket_guard.as_ref().unwrap().clone())
    }

    async fn get_publisher(&self) -> Result<Socket, Error> {
        self.get_socket(SocketConfig {
            socket_type: SocketType::PUB,
            timeout_ms: SOCKET_TIMEOUT_MS,
            is_server: true,
        }).await
    }

    async fn get_subscriber(&self) -> Result<Socket, Error> {
        self.get_socket(SocketConfig {
            socket_type: SocketType::SUB,
            timeout_ms: SOCKET_TIMEOUT_MS,
            is_server: false,
        }).await
    }

    /// Attempts to reconnect a socket with retries
    async fn try_reconnect(&self, socket: &Socket) -> Result<(), Error> {
        for attempt in 1..=MAX_RECONNECT_ATTEMPTS {
            match socket.connect(&self.address) {
                Ok(_) => {
                    info!("Successfully reconnected on attempt {}", attempt);
                    return Ok(());
                }
                Err(e) => {
                    if attempt == MAX_RECONNECT_ATTEMPTS {
                        return Err(Error::ConnectionError(format!("Failed to reconnect: {}", e)));
                    }
                    warn!("Reconnect attempt {} failed, retrying...", attempt);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
        Err(Error::ConnectionError("Max reconnection attempts reached".to_string()))
    }


    async fn get_dealer(&self) -> Result<Socket, zmq::Error> {
        let dealer_socket = self.dealer.lock().await;
        if let Some(socket) = &*dealer_socket{
            return Ok(socket.clone());
        }
        drop(dealer_socket);
        let mut dealer_socket_guard = self.dealer.lock().await;
        let context = Context::new();
        let dealer = context.socket(zmq::DEALER)?;
        dealer.connect(&self.address)?;
        *dealer_socket_guard = Some(dealer);
        info!("Dealer socket initialized");
        return Ok(dealer_socket_guard.as_ref().unwrap().clone());

    }


    async fn get_router(&self) -> Result<Socket, zmq::Error> {
        let router_socket = self.router.lock().await;
        if let Some(socket) = &*router_socket{
            return Ok(socket.clone());
        }
        drop(router_socket);
        let mut router_socket_guard = self.router.lock().await;
        let context = Context::new();
        let router = context.socket(zmq::ROUTER)?;
        router.bind(&self.address)?;
        *router_socket_guard = Some(router);
        info!("Router socket initialized");
        return Ok(router_socket_guard.as_ref().unwrap().clone());
    }


}
#[async_trait]
impl EventBusPort for ZMQEventBus {
    type EventType = DomainEvent;

    async fn publish(&self, event: Self::EventType, topic: String) -> Result<(), Error> {
        Self::validate_topic(&topic)?;

        let publisher = self.get_publisher().await?;
        let data = serde_json::to_string(&event)
            .map_err(|e| Error::SerializationError(format!("Failed to serialize event: {}", e)))?;

        let message = format!("{} {}", topic, data);

        match publisher.send(message.as_bytes(), 0) {
            Ok(_) => {
                info!("Published event to topic {}: {:?}", topic, event);
                Ok(())
            }
            Err(e) => {
                error!("Failed to publish event to topic {}: {}", topic, e);
                // Attempt to reconnect and retry once
                if let Ok(_) = self.try_reconnect(&publisher).await {
                    publisher.send(message.as_bytes(), 0)
                        .map_err(|e| Error::PublishError(format!("Failed to publish event after reconnect: {}", e)))?;
                    Ok(())
                } else {
                    Err(Error::PublishError("Failed to publish event".to_string()))
                }
            }
        }
    }

    async fn subscribe(&self, topic: String, callback: fn(Self::EventType)) -> Result<(), Error> {
        Self::validate_topic(&topic)?;

        let subscriber = self.get_subscriber().await?;
        subscriber.set_subscribe(topic.as_bytes())
            .map_err(|e| Error::SubscriptionError(format!("Failed to subscribe to topic {}: {}", topic, e)))?;

        info!("Subscribed to topic {}", topic);

        let is_running = Arc::clone(&self.is_running);
        let topic_clone = topic.clone();

        tokio::spawn(async move {
            while is_running.load(std::sync::atomic::Ordering::SeqCst) {
                match subscriber.recv_msg(0) {
                    Ok(msg) => {
                        match String::from_utf8(msg.to_vec()) {
                            Ok(message) => {
                                let parts: Vec<&str> = message.splitn(2, ' ').collect();
                                if parts.len() == 2 && parts[0] == topic_clone {
                                    match serde_json::from_str::<DomainEvent>(parts[1]) {
                                        Ok(event) => callback(event),
                                        Err(e) => error!("Failed to deserialize message: {}", e),
                                    }
                                }
                            }
                            Err(e) => error!("Invalid UTF-8 in message: {}", e),
                        }
                    }
                    Err(e) => {
                        error!("Error receiving message on topic {}: {}", topic_clone, e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
             info!("Subscription loop ended for topic {}", topic_clone);
        });

        Ok(())
    }
    async fn subscribe_many(&self, topics: Vec<String>, callback: fn(Self::EventType)) -> Result<(), Error> {
        if topics.is_empty() {
            return Err(Error::ValidationError("Topics list cannot be empty".to_string()));
        }

        for topic in topics {
            self.subscribe(topic, callback).await?;
        }
        Ok(())
    }

    /// Stops all event bus operations
    pub async fn shutdown(&self) -> Result<(), Error> {
        self.is_running.store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zmq_event_bus_creation() {
        let valid_address = "tcp://127.0.0.1:5555".to_string();
        assert!(ZMQEventBus::new(valid_address).is_ok());

        let empty_address = "".to_string();
        assert!(ZMQEventBus::new(empty_address).is_err());

        let invalid_address = "invalid://address".to_string();
        assert!(ZMQEventBus::new(invalid_address).is_err());
    }

    #[test]
    fn test_topic_validation() {
        assert!(ZMQEventBus::validate_topic("valid.topic").is_ok());
        assert!(ZMQEventBus::validate_topic("").is_err());
        assert!(ZMQEventBus::validate_topic("invalid topic").is_err());
    }
}
