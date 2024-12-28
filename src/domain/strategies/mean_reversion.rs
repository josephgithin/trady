use async_trait::async_trait;
use crate::common::types::Error;
use crate::common::enums::{StrategyName, TradeSide};
use crate::domain::events::{DomainEvent, TradeSignalEvent};
use crate::domain::interfaces::StrategyPort;
use crate::domain::models::StrategyConfig;
use crate::domain::strategies::base::{TradingStrategy, TradingStrategyImpl};

/// MeanReversionStrategy implements a mean reversion trading strategy
/// that generates signals based on price deviations from moving averages
#[derive(Debug)]
pub struct MeanReversionStrategy {
    base: TradingStrategyImpl,
}

impl MeanReversionStrategy {
    pub fn new(config: StrategyConfig, sink_topics: Vec<String>) -> Self {
        MeanReversionStrategy {
            base: TradingStrategyImpl::new(config, sink_topics),
        }
    }
}

#[async_trait]
impl TradingStrategy<DomainEvent> for MeanReversionStrategy {
    fn name(&self) -> StrategyName {
        StrategyName::MeanReversion
    }
}
#[async_trait]
impl StrategyPort for MeanReversionStrategy {
    type EventType = DomainEvent;
    type TradeSignalType = TradeSignalEvent;

    async fn analyze_market_data(&self, event: Self::EventType) -> Result<Vec<Self::TradeSignalType>, Error> {
        match event {
            DomainEvent::PriceUpdate(event) => {
                if let Some(kraken_data) = &event.kraken_data {
                     Ok(vec![TradeSignalEvent {
                        symbol: kraken_data.pair.to_string(),
                        side: TradeSide::Buy,
                        size: 0.01,
                        strategy: self.name(),
                     }])
                } else {
                    Ok(vec![])
                }
            }
            _ => Ok(vec![]),
        }
    }

    async fn start(&self, callback: fn(Self::TradeSignalType)) -> Result<(), Error> {
        self.base.start(callback).await
    }

    async fn stop(&self) -> Result<(), Error> {
        self.base.stop().await
    }

    async fn configure_strategy(&mut self, config: StrategyConfig) -> Result<(), Error> {
        self.base.configure_strategy(config).await
    }

   async fn get_strategy_config(&self) -> Result<StrategyConfig, Error> {
       self.base.get_strategy_config().await
    }


    fn get_sink_topics(&self) -> Vec<String> {
        self.base.get_sink_topics()
    }

    fn get_source_topics(&self) -> Vec<String> {
        vec!["price.feed.kraken".to_string()]
    }
}
