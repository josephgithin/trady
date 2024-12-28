use async_trait::async_trait;
use futures::StreamExt;
use log::{error, info};
use serde_json::from_str;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_websockets::{connect, Message, WebSocketStream, ClientConfig};

use crate::common::enums::ExchangeName;
use crate::common::types::Error;
use crate::config::ExchangeConfig;
use crate::domain::events::{DomainEvent, PriceUpdateEvent, ExchangeData};
use crate::domain::interfaces::DataAdapterPort;
use crate::infra::adapters::exchange::{ExchangeAdapter, ExchangeAdapterImpl};

/// Adapter for connecting to and processing data from Kraken exchange
#[derive(Debug)]
pub struct KrakenAdapter {
    base: ExchangeAdapterImpl,
    socket: Arc<Mutex<Option<WebSocketStream<ClientConfig>>>>,
}

impl KrakenAdapter {
    /// Creates a new KrakenAdapter instance
    pub fn new(config: ExchangeConfig) -> Result<Self, Error> {
        Ok(KrakenAdapter {
            base: ExchangeAdapterImpl::new(config, ExchangeName::Kraken)?,
            socket: Arc::new(Mutex::new(None)),
        })
    }
}

#[async_trait]
impl ExchangeAdapter<DomainEvent> for KrakenAdapter {
 fn get_api_url(&self) -> String {
     self.base.get_api_url()
 }
 
 fn get_pairs(&self) -> Vec<String> {
     self.base.get_pairs()
 }
 
 fn get_exchange_name(&self) -> ExchangeName {
     self.base.get_exchange_name()
 }
}

#[async_trait]
impl DataAdapterPort<DomainEvent> for KrakenAdapter {
 async fn connect(&self) -> Result<(), Error> {
     self.base.connect().await
 }
 
 async fn process_message(&self) -> Result<Option<DomainEvent>, Error> {
     Ok(None)
 }
 
 async fn start(&self, callback: fn(DomainEvent)) -> Result<(), Error> {
 let url = self.get_api_url();
 let pairs = self.get_pairs();
 info!("Starting Kraken adapter for pairs {:?}", pairs);
 let subscribe_message = serde_json::json!({
 "event": "subscribe",
 "pair": pairs,
 "subscription": {"name": "ticker"}
 }).to_string();

 let stream = connect(url)
     .await
     .map_err(|e| Error::ConnectionError(format!("Failed to connect to Kraken: {}", e)))?;
 
 let mut websocket = stream;
 
 websocket
     .send(Message::Text(subscribe_message))
     .await
     .map_err(|e| Error::ConnectionError(format!("Failed to subscribe to Kraken: {}", e)))?;
 
 info!("Subscribed to Kraken");
 
 let socket = Arc::clone(&self.socket);
 *socket.lock().await = Some(websocket);
 
 let socket_clone = Arc::clone(&socket);
 let pairs_clone = pairs.clone();
 
 tokio::spawn(async move {
     let mut ws = socket_clone.lock().await.take().unwrap();
     
     while let Some(message) = ws.next().await {
         match message {
             Ok(msg) => {
                 if let Message::Text(text) = msg {
                     match Self::process_kraken_message(&text, &pairs_clone).await {
                         Ok(event) => {
                             callback(DomainEvent::PriceUpdate(event));
                         }
                         Err(e) => error!("Error parsing message from Kraken: {}", e),
                     }
                 }
             }
             Err(e) => {
                 error!("Error receiving message from Kraken: {}", e);
                 break;
             }
         }
     }
     error!("Kraken WebSocket connection closed");
 });
 
 Ok(())
 }

 async fn stop(&self) -> Result<(), Error> {
     info!("Stopping Kraken adapter");
     if let Some(mut ws) = self.socket.lock().await.take() {
         ws.close().await.map_err(|e| Error::ConnectionError(e.to_string()))?;
     }
     Ok(())
 }
 
 fn get_sink_topics(&self) -> Vec<String> {
     self.base.get_sink_topics()
 }
}

impl KrakenAdapter {
 /// Process a message received from Kraken WebSocket
 async fn process_kraken_message(message: &str, pairs: &[String]) -> Result<PriceUpdateEvent, Error> {
     let data: serde_json::Value = from_str(message)
         .map_err(|e| Error::ParseError(format!("Failed to parse Kraken message: {}", e)))?;
 
     if let Some(array) = data.as_array() {
         if array.len() <= 3 {
             return Ok(PriceUpdateEvent {
                 kraken_data: None,
                 coinbase_data: None,
             });
         }
 
         let symbol = array[3].as_str()
             .ok_or_else(|| Error::ParseError("Invalid symbol format".to_string()))?;
 
         if !pairs.contains(&symbol.to_string()) {
             return Ok(PriceUpdateEvent {
                 kraken_data: None,
                 coinbase_data: None,
             });
         }
 
         if let Some(info) = array[1].as_object() {
             if let Some(price) = info.get("c")
                 .and_then(|p| p.as_array())
                 .and_then(|arr| arr.first())
                 .and_then(|v| v.as_str()) {
                 
                 let price = price.parse::<f64>()
                     .map_err(|e| Error::ParseError(format!("Failed to parse price: {}", e)))?;
 
                 return Ok(PriceUpdateEvent {
                     kraken_data: Some(ExchangeData {
                         exchange: ExchangeName::Kraken,
                         pair: symbol.to_string(),
                         price,
                     }),
                     coinbase_data: None,
                 });
             }
         }
     }
 
     Ok(PriceUpdateEvent {
         kraken_data: None,
         coinbase_data: None,
     })
 }
 }
 
 #[cfg(test)]
 mod tests {
 use super::*;
 
 #[tokio::test]
 async fn test_process_kraken_message() {
     let message = r#"[
         278,
         {"c":["50000.5","1.0"]},
         "ticker",
         "BTC/USD"
     ]"#;
 
     let pairs = vec!["BTC/USD".to_string()];
     let result = KrakenAdapter::process_kraken_message(message, &pairs).await.unwrap();
     
     if let Some(data) = result.kraken_data {
         assert_eq!(data.exchange, ExchangeName::Kraken);
         assert_eq!(data.pair, "BTC/USD");
         assert_eq!(data.price, 50000.5);
     } else {
         panic!("Expected Some(ExchangeData), got None");
     }
 }
 
 #[tokio::test]
 async fn test_process_invalid_message() {
     let message = r#"{"invalid": "message"}"#;
     let pairs = vec!["BTC/USD".to_string()];
     
     let result = KrakenAdapter::process_kraken_message(message, &pairs).await.unwrap();
     assert!(result.kraken_data.is_none());
 }
}
EOF"
