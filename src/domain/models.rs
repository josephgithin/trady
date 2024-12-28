use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use rust_decimal::Decimal;
use crate::common::enums::StrategyName;

/// Represents the side of an order (buy/sell)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Represents the type of order
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
    TakeProfit,
}

/// Represents a trading order
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Order {
    pub symbol: String,
    pub side: OrderSide,
    #[serde(with = "rust_decimal::serde::float")]
    pub size: f64,
    #[serde(with = "rust_decimal::serde::float")]
    pub price: f64,
    pub order_type: OrderType,
}

/// Configuration for a trading strategy
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StrategyConfig {
    pub strategy_name: StrategyName,
    pub params: HashMap<String, String>,
    pub enabled: bool,
}

impl Order {
    /// Creates a new order with validation
    pub fn new(
        symbol: String,
        side: OrderSide,
        size: f64,
        price: f64,
        order_type: OrderType,
    ) -> Result<Self, &'static str> {
        if symbol.is_empty() {
            return Err("Symbol cannot be empty");
        }
        if size <= 0.0 {
            return Err("Size must be positive");
        }
        if price < 0.0 {
            return Err("Price cannot be negative");
        }

        Ok(Self {
            symbol,
            side,
            size,
            price,
            order_type,
        })
    }

    /// Creates a new market buy order
    pub fn market_buy(symbol: String, size: f64) -> Result<Self, &'static str> {
        Self::new(symbol, OrderSide::Buy, size, 0.0, OrderType::Market)
    }

    /// Creates a new market sell order
    pub fn market_sell(symbol: String, size: f64) -> Result<Self, &'static str> {
        Self::new(symbol, OrderSide::Sell, size, 0.0, OrderType::Market)
    }

    /// Creates a new limit buy order
    pub fn limit_buy(symbol: String, size: f64, price: f64) -> Result<Self, &'static str> {
        Self::new(symbol, OrderSide::Buy, size, price, OrderType::Limit)
    }

    /// Creates a new limit sell order
    pub fn limit_sell(symbol: String, size: f64, price: f64) -> Result<Self, &'static str> {
        Self::new(symbol, OrderSide::Sell, size, price, OrderType::Limit)
    }
}

impl StrategyConfig {
    /// Creates a new strategy configuration
    pub fn new(strategy_name: StrategyName, params: HashMap<String, String>, enabled: bool) -> Self {
        Self {
            strategy_name,
            params,
            enabled,
        }
    }

    /// Gets a parameter value as a string
    pub fn get_param(&self, key: &str) -> Option<&String> {
        self.params.get(key)
    }

    /// Gets a parameter value as a float
    pub fn get_param_as_f64(&self, key: &str) -> Option<f64> {
        self.params
            .get(key)
            .and_then(|v| v.parse::<f64>().ok())
    }

    /// Gets a parameter value as an integer
    pub fn get_param_as_i64(&self, key: &str) -> Option<i64> {
        self.params
            .get(key)
            .and_then(|v| v.parse::<i64>().ok())
    }

    /// Gets a parameter value as a boolean
    pub fn get_param_as_bool(&self, key: &str) -> Option<bool> {
        self.params
            .get(key)
            .and_then(|v| v.parse::<bool>().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_validation() {
        assert!(Order::market_buy("".to_string(), 1.0).is_err());
        assert!(Order::market_buy("BTC-USD".to_string(), 0.0).is_err());
        assert!(Order::market_buy("BTC-USD".to_string(), -1.0).is_err());
        assert!(Order::limit_buy("BTC-USD".to_string(), 1.0, -1.0).is_err());
        
        let valid_order = Order::market_buy("BTC-USD".to_string(), 1.0);
        assert!(valid_order.is_ok());
    }

    #[test]
    fn test_strategy_config() {
        let mut params = HashMap::new();
        params.insert("threshold".to_string(), "0.5".to_string());
        params.insert("enabled".to_string(), "true".to_string());

        let config = StrategyConfig::new(StrategyName::MeanReversion, params, true);
        
        assert_eq!(config.get_param_as_f64("threshold"), Some(0.5));
        assert_eq!(config.get_param_as_bool("enabled"), Some(true));
        assert_eq!(config.get_param_as_f64("nonexistent"), None);
    }
}
