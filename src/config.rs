use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use config::{Config as ConfigLoader, ConfigError as ConfigLoaderError, File};
use thiserror::Error;
use log::{info, warn};

use crate::common::enums::{ExchangeName, StrategyName, BrokerName};

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to load configuration: {0}")]
    LoadError(String),
    #[error("Invalid configuration: {0}")]
    ValidationError(String),
    #[error("Missing required configuration: {0}")]
    MissingConfig(String),
    #[error("Config Loader Error: {0}")]
    LoaderError(#[from] ConfigLoaderError)
}

pub type Result<T> = std::result::Result<T, ConfigError>;

/// Main configuration structure for the trading platform
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub event_bus: EventBusConfig,
    pub components: ComponentsConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            event_bus: EventBusConfig::default(),
            components: ComponentsConfig::default(),
        }
    }
}

impl Config {
    /// Validates the configuration
    pub fn validate(&self) -> Result<()> {
        self.event_bus.validate()?;
        self.components.validate()?;
        Ok(())
    }

    /// Loads configuration from the specified file path
    pub fn load_from_file(path: PathBuf) -> Result<Self> {
        info!("Loading configuration from: {:?}", path);

        if !path.exists() {
            return Err(ConfigError::LoadError(format!("Config file not found: {:?}", path)));
        }

        let mut builder = ConfigLoader::builder();

        // Add default values
        builder = builder.set_default("event_bus.type", "zmq")?;

        // Load from file
        builder = builder.add_source(File::from(path));

        // Add environment variables
        builder = builder.add_source(
            config::Environment::with_prefix("TRADING_PLATFORM")
                .separator("_")
                .try_parsing(true)
        );

        let config: Self = builder.build()?.try_deserialize()?;
        config.validate()?;

        Ok(config)
    }

   /// Loads configuration from the default path
    pub fn load() -> Result<Self> {
        let config_path = std::env::var("CONFIG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("config.toml"));

        Self::load_from_file(config_path)
    }
}

/// Event bus configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EventBusConfig {
    pub type_name: String,
    pub zmq_address: String,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            type_name: "zmq".to_string(),
            zmq_address: "tcp://127.0.0.1:5555".to_string(),
        }
    }
}

impl EventBusConfig {
    pub fn validate(&self) -> Result<()> {
        if self.type_name.is_empty() {
            return Err(ConfigError::ValidationError("Event bus type cannot be empty".to_string()));
        }
        if self.zmq_address.is_empty() {
            return Err(ConfigError::ValidationError("ZMQ address cannot be empty".to_string()));
        }
        Ok(())
    }
}

/// Components configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ComponentsConfig {
    pub exchanges: Option<HashMap<ExchangeName, ExchangeConfig>>,
    pub strategies: Option<HashMap<StrategyName, StrategyConfig>>,
    pub broker: Option<HashMap<BrokerName, BrokerConfig>>,
    pub data_transformations: Option<HashMap<String, DataTransformationConfig>>,
    pub ui: Option<SubscriberConfig>,
    pub logging: Option<SubscriberConfig>,
    pub metrics: Option<SubscriberConfig>,
    pub audit: Option<SubscriberConfig>,
}

impl Default for ComponentsConfig {
    fn default() -> Self {
        Self {
            exchanges: Some(HashMap::new()),
            strategies: Some(HashMap::new()),
            broker: Some(HashMap::new()),
            data_transformations: Some(HashMap::new()),
            ui: Some(SubscriberConfig::default()),
            logging: Some(SubscriberConfig::default()),
            metrics: Some(SubscriberConfig::default()),
            audit: Some(SubscriberConfig::default()),
        }
    }
}

impl ComponentsConfig {
    pub fn validate(&self) -> Result<()> {
        if let Some(exchanges) = &self.exchanges {
            for (name, config) in exchanges {
                config.validate().map_err(|e|
                    ConfigError::ValidationError(format!("Exchange {}: {}", name, e)))?;
            }
        }

        if let Some(strategies) = &self.strategies {
            for (name, config) in strategies {
                config.validate().map_err(|e|
                    ConfigError::ValidationError(format!("Strategy {}: {}", name, e)))?;
            }
        }

        if let Some(brokers) = &self.broker {
            for (name, config) in brokers {
                config.validate().map_err(|e|
                    ConfigError::ValidationError(format!("Broker {}: {}", name, e)))?;
            }
        }

        Ok(())
    }
}
/// Subscriber configuration
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct SubscriberConfig {
    pub enabled: bool,
    pub topics: Vec<String>,
    pub level: Option<String>,
    pub file: Option<String>
}


impl SubscriberConfig {
    pub fn validate(&self) -> Result<()> {
        if self.enabled && self.topics.is_empty() {
            return Err(ConfigError::ValidationError("Enabled subscriber must have at least one topic".to_string()));
        }
        Ok(())
    }
}
/// Exchange configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ExchangeConfig {
    pub api_url: String,
    pub pairs: Vec<String>,
}

impl ExchangeConfig {
    pub fn validate(&self) -> Result<()> {
        if self.api_url.is_empty() {
            return Err(ConfigError::ValidationError("API URL cannot be empty".to_string()));
        }
        if self.pairs.is_empty() {
            return Err(ConfigError::ValidationError("At least one trading pair must be specified".to_string()));
        }
        Ok(())
    }
}

/// Broker configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BrokerConfig {
     pub host: String,
     pub port: u16,
     pub client_id: u16,
     pub account_id: String,
     pub paper_trading: bool,
     pub max_retries: u32,
     pub timeout: u64,
}

impl BrokerConfig {
    pub fn validate(&self) -> Result<()> {
        if self.host.is_empty() {
            return Err(ConfigError::ValidationError("Broker host cannot be empty".to_string()));
        }
         if self.port == 0{
            return Err(ConfigError::ValidationError("Broker port cannot be zero".to_string()));
         }
        Ok(())
    }
}

/// Strategy configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StrategyConfig {
    pub params: HashMap<String, String>,
    pub enabled: bool,
}

impl StrategyConfig {
    pub fn validate(&self) -> Result<()> {
        if self.enabled && self.params.is_empty() {
            return Err(ConfigError::ValidationError("Enabled strategy must have parameters".to_string()));
        }
        Ok(())
    }
}
/// Data transformation configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DataTransformationConfig {
    pub source_topic: String,
    pub destination_topic: String,
    pub enabled: bool,
    pub params: HashMap<String, String>,
}

impl DataTransformationConfig {
    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            if self.source_topic.is_empty() {
                return Err(ConfigError::ValidationError("Source topic cannot be empty".to_string()));
            }
            if self.destination_topic.is_empty() {
                return Err(ConfigError::ValidationError("Destination topic cannot be empty".to_string()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_exchange_config_validation() {
        let config = ExchangeConfig {
            api_url: "".to_string(),
            pairs: vec![],
        };
        assert!(config.validate().is_err());

        let config = ExchangeConfig {
            api_url: "http://example.com".to_string(),
            pairs: vec!["BTC/USD".to_string()],
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_subscriber_config_validation() {
        let config = SubscriberConfig {
            enabled: true,
            topics: vec![],
            level:None,
            file:None
        };
        assert!(config.validate().is_err());

        let config = SubscriberConfig {
            enabled: true,
            topics: vec!["topic1".to_string()],
            level:Some("info".to_string()),
            file:Some("file.log".to_string())
        };
        assert!(config.validate().is_ok());
    }
}
