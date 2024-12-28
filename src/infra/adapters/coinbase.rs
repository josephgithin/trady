use async_trait::async_trait;
use futures::StreamExt;
use log::{error, info};
use serde_json::from_str;
use tokio_websockets::{connect, Message, WebSocketStream, ClientConfig};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::common::enums::ExchangeName;
use crate::common::types::Error;
use crate::config::ExchangeConfig;
use crate::domain::events::{DomainEvent, PriceUpdateEvent, ExchangeData};
use crate::domain::interfaces::DataAdapterPort;
use crate::infra::adapters::exchange::{ExchangeAdapter, ExchangeAdapterImpl};

/// Adapter for connecting to and processing data from Coinbase
#[derive(Debug)]
pub struct CoinbaseAdapter {
    base: ExchangeAdapterImpl,
    socket: Arc<Mutex<Option<WebSocketStream<ClientConfig>>>>,
}

impl CoinbaseAdapter {
    /// Creates a new CoinbaseAdapter instance
    pub fn new(config: ExchangeConfig) -> Self {
        CoinbaseAdapter {
            base: ExchangeAdapterImpl::new(config, ExchangeName::Coinbase),
            socket: Arc::new(Mutex::new(None)),
        }
    }
}
 #[async_trait]
 impl ExchangeAdapter<DomainEvent> for CoinbaseAdapter {
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
impl DataAdapterPort<DomainEvent> for CoinbaseAdapter {
 async fn connect(&self) -> Result<(), Error> {
     self.base.connect().await
 }
 
 async fn process_message(&self) -> Result<Option<DomainEvent>, Error> {
     Ok(None)
 }
 
 async fn start(&self, callback: fn(DomainEvent)) -> Result<(), Error> {
     let url = self.get_api_url();
     let pairs = self.get_pairs();
     info!("Starting Coinbase adapter for pairs {:?}", pairs);
 
     let subscribe_message = serde_json::json!({
         "type": "subscribe",
         "product_ids": pairs,
         "channels": ["ticker"]
     })
     .to_string();
 
     let stream = connect(url).await.map_err(|e| Error::ConnectionError(e.to_string()))?;
     let mut websocket = stream;
 
     websocket
         .send(Message::Text(subscribe_message))
         .await
         .map_err(|e| Error::ConnectionError(e.to_string()))?;
 
     info!("Subscribed to Coinbase");
 
     let socket = Arc::clone(&self.socket);
     *socket.lock().await = Some(websocket);
 
     let socket_clone = Arc::clone(&socket);
     tokio::spawn(async move {
         let mut ws = socket_clone.lock().await.take().unwrap();
         
         while let Some(message) = ws.next().await {
             match message {
                 Ok(msg) => {
                     if let Message::Text(text) = msg {
                         match Self::process_coinbase_message(&text).await {
                             Ok(event) => {
                                 callback(DomainEvent::PriceUpdate(event));
                             }
                             Err(e) => error!("Error parsing message from Coinbase: {}", e),
                         }
                     }
                 }
                 Err(e) => {
                     error!("Error receiving message from Coinbase: {}", e);
                     break;
                 }
             }
         }
         error!("Coinbase WebSocket connection closed");
     });
 
     Ok(())
 }
 async fn stop(&self) -> Result<(), Error> {
     info!("Stopping Coinbase adapter");
     if let Some(mut ws) = self.socket.lock().await.take() {
         ws.close().await.map_err(|e| Error::ConnectionError(e.to_string()))?;
     }
     Ok(())
 }
 
 fn get_sink_topics(&self) -> Vec<String> {
     self.base.get_sink_topics()
 }
}

impl CoinbaseAdapter {
 /// Process a message received from Coinbase WebSocket
 async fn process_coinbase_message(message: &str) -> Result<PriceUpdateEvent, Error> {
     let data: serde_json::Value = from_str(message)
         .map_err(|e| Error::ParseError(format!("Failed to parse Coinbase message: {}", e)))?;
 
     match (
         data.get("type"),
         data.get("price"),
         data.get("product_id"),
     ) {
         (Some(msg_type), Some(price), Some(product_id)) if msg_type == "ticker" => {
             let price = price
                 .as_str()
                 .ok_or_else(|| Error::ParseError("Invalid price format".to_string()))?
                 .parse::<f64>()
                 .map_err(|e| Error::ParseError(format!("Failed to parse price: {}", e)))?;
 
             let symbol = product_id
                 .as_str()
                 .ok_or_else(|| Error::ParseError("Invalid product_id format".to_string()))?
                 .to_string();
 
             Ok(PriceUpdateEvent {
                 kraken_data: None,
                 coinbase_data: Some(ExchangeData {
                     exchange: ExchangeName::Coinbase,
                     pair: symbol,
                     price,
                 }),
             })
         }
         _ => Ok(PriceUpdateEvent {
             kraken_data: None,
             coinbase_data: None,
         }),
     }
 }
 }
 
 #[cfg(test)]
 mod tests {
 use super::*;
 
 #[tokio::test]
 async fn test_process_coinbase_message() {
     let message = r#"{
         "type": "ticker",
         "price": "50000.50",
         "product_id": "BTC-USD"
     }"#;
 
     let result = CoinbaseAdapter::process_coinbase_message(message).await.unwrap();
     
     if let Some(data) = result.coinbase_data {
         assert_eq!(data.exchange, ExchangeName::Coinbase);
         assert_eq!(data.pair, "BTC-USD");
         assert_eq!(data.price, 50000.50);
     } else {
         panic!("Expected Some(ExchangeData), got None");
     }
 }
 
 #[tokio::test]
 async fn test_process_invalid_message() {
     let message = r#"{
         "type": "invalid",
         "price": "not_a_number",
         "product_id": "BTC-USD"
     }"#;
 
     let result = CoinbaseAdapter::process_coinbase_message(message).await.unwrap();
     assert!(result.coinbase_data.is_none());
 }
}
EOF"
