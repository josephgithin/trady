use async_trait::async_trait;
use futures_util::StreamExt;
use log::{error, info};
use serde_json::from_str;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::{
    connect_async,
    tungstenite::protocol::Message,
    WebSocketStream,
    MaybeTlsStream
};
use tokio::net::TcpStream;
use url::Url;

use crate::common::enums::ExchangeName;
use crate::common::types::Error;
use crate::config::ExchangeConfig;
use crate::domain::events::{DomainEvent, PriceUpdateEvent, ExchangeData};
use crate::domain::interfaces::DataAdapterPort;
use crate::infra::adapters::exchange::{ExchangeAdapter, ExchangeAdapterImpl};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Adapter for connecting to and processing data from Coinbase
#[derive(Debug)]
pub struct CoinbaseAdapter {
    base: ExchangeAdapterImpl,
    socket: Arc<Mutex<Option<WsStream>>>,
}

impl CoinbaseAdapter {
    /// Creates a new CoinbaseAdapter instance
    pub fn new(config: ExchangeConfig) -> Result<Self, Error> {
        Ok(CoinbaseAdapter {
            base: ExchangeAdapterImpl::new(config, ExchangeName::Coinbase)?,
            socket: Arc::new(Mutex::new(None)),
        })
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

    async fn start<F>(&self, mut callback: F) -> Result<(), Error>
    where
        F: FnMut(DomainEvent) + Send + 'static,
    {
        let url = self.get_api_url();
        let pairs = self.get_pairs();
        info!("Starting Coinbase adapter for pairs {:?}", pairs);

        let subscribe_message = serde_json::json!({
            "type": "subscribe",
            "product_ids": pairs,
            "channels": ["ticker"]
        })
        .to_string();

        let url = Url::parse(&url)
            .map_err(|e| Error::ConnectionError(format!("Invalid URL: {}", e)))?;

        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| Error::ConnectionError(format!("Failed to connect to Coinbase: {}", e)))?;

        let mut websocket = ws_stream;

        websocket
            .send(Message::Text(subscribe_message))
            .await
            .map_err(|e| Error::ConnectionError(format!("Failed to subscribe to Coinbase: {}", e)))?;

        info!("Subscribed to Coinbase");

        let socket = Arc::clone(&self.socket);
        *socket.lock().await = Some(websocket);

        let socket_clone = Arc::clone(&socket);

        tokio::spawn(async move {
            if let Some(mut ws) = socket_clone.lock().await.take() {
                while let Some(msg_result) = ws.next().await {
                    match msg_result {
                        Ok(msg) => {
                            if let Message::Text(text) = msg {
                                match Self::process_coinbase_message(&text).await {
                                    Ok(event) => {
                                        if event.coinbase_data.is_some() {
                                            callback(DomainEvent::PriceUpdate(event));
                                        }
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
            }
            error!("Coinbase WebSocket connection closed");
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        info!("Stopping Coinbase adapter");
        if let Some(mut ws) = self.socket.lock().await.take() {
            ws.close(None)
                .await
                .map_err(|e| Error::ConnectionError(e.to_string()))?;
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
            _ => {
                // Return empty event for subscription acknowledgments and other non-ticker messages
                Ok(PriceUpdateEvent {
                    kraken_data: None,
                    coinbase_data: None,
                })
            }
        }
    }

    /// Validates the required fields in a message
    fn validate_fields(data: &serde_json::Value) -> Result<(), Error> {
        if data.get("type").is_none() {
            return Err(Error::ParseError("Missing 'type' field".to_string()));
        }
        if data.get("price").is_none() && data.get("type") == Some(&serde_json::Value::String("ticker".to_string())) {
            return Err(Error::ParseError("Missing 'price' field in ticker message".to_string()));
        }
        if data.get("product_id").is_none() && data.get("type") == Some(&serde_json::Value::String("ticker".to_string())) {
            return Err(Error::ParseError("Missing 'product_id' field in ticker message".to_string()));
        }
        Ok(())
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

    #[tokio::test]
    async fn test_process_subscription_message() {
        let message = r#"{
            "type": "subscriptions",
            "channels": [{"name": "ticker", "product_ids": ["BTC-USD"]}]
        }"#;

        let result = CoinbaseAdapter::process_coinbase_message(message).await.unwrap();
        assert!(result.coinbase_data.is_none());
    }

    #[tokio::test]
    async fn test_message_validation() {
        let valid_message = serde_json::json!({
            "type": "ticker",
            "price": "50000.50",
            "product_id": "BTC-USD"
        });
        assert!(CoinbaseAdapter::validate_fields(&valid_message).is_ok());

        let invalid_message = serde_json::json!({
            "price": "50000.50",
            "product_id": "BTC-USD"
        });
        assert!(CoinbaseAdapter::validate_fields(&invalid_message).is_err());
    }

    #[tokio::test]
    async fn test_new_adapter() {
        let config = ExchangeConfig {
            api_url: "wss://ws-feed.pro.coinbase.com".to_string(),
            pairs: vec!["BTC-USD".to_string()],
        };
        
        let adapter = CoinbaseAdapter::new(config);
        assert!(adapter.is_ok());
    }
}
