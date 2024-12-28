use async_trait::async_trait;
use std::fmt::Debug;
use crate::common::types::Error;
use crate::domain::events::{DomainEvent, TradeSignalEvent};
use crate::domain::interfaces::StrategyPort;
use crate::domain::models::StrategyConfig;
use log::info;

/// Defines the core functionality for all trading strategies
#[async_trait]
pub trait TradingStrategy<T: Send + Sync + Debug>: StrategyPort<EventType = DomainEvent, TradeSignalType = TradeSignalEvent> + Send + Sync {
    /// Returns the unique name of the trading strategy
    fn name(&self) -> String;
}

/// Base implementation for trading strategies
#[derive(Debug)]
pub struct TradingStrategyImpl {
    strategy_config: StrategyConfig,
    sink_topics: Vec<String>,
}

impl TradingStrategyImpl {
    pub fn new(config: StrategyConfig, sink_topics: Vec<String>) -> Self {
        TradingStrategyImpl {
            strategy_config: config,
            sink_topics,
        }
    }

    pub fn get_sink_topics(&self) -> Vec<String> {
        self.sink_topics.clone()
    }

    pub fn get_config(&self) -> &StrategyConfig {
        &self.strategy_config
    }
}

#[async_trait]
impl<T: Send + Sync + Debug> StrategyPort for TradingStrategyImpl {
 type EventType = DomainEvent;
 type TradeSignalType = TradeSignalEvent;
 
 async fn analyze_market_data(&self, _event: Self::EventType) -> Result<Vec<Self::TradeSignalType>, Error> {
     Ok(vec![])
 }
 
 async fn start(&self, _callback: fn(Self::TradeSignalType)) -> Result<(), Error> {
     info!("Starting strategy {}", self.strategy_config.strategy_name);
     Ok(())
 }
 
 async fn stop(&self) -> Result<(), Error> {
     info!("Stopping strategy {}", self.strategy_config.strategy_name);
     Ok(())
 }
 
 async fn configure_strategy(&mut self, config: StrategyConfig) -> Result<(), Error> {
     info!(
         "Configuring strategy {} with {:?}",
         self.strategy_config.strategy_name,
         config
     );
     self.strategy_config = config;
     Ok(())
 }
 
 async fn get_strategy_config(&self) -> Result<StrategyConfig, Error> {
     Ok(self.strategy_config.clone())
 }
 
 fn get_source_topics(&self) -> Vec<String> {
     vec![]
 }
 }
