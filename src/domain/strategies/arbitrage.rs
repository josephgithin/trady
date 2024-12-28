use async_trait::async_trait;
use crate::common::types::Error;
use crate::common::enums::{StrategyName, TradeSide};
use crate::domain::events::{DomainEvent, TradeSignalEvent};
use crate::domain::interfaces::StrategyPort;
use crate::domain::models::StrategyConfig;
use crate::domain::strategies::base::{TradingStrategy, TradingStrategyImpl};

/// ArbitrageStrategy implements a basic arbitrage strategy between Kraken and Coinbase
#[derive(Debug)]
pub struct ArbitrageStrategy {
    base: TradingStrategyImpl,
}

impl ArbitrageStrategy {
    pub fn new(config: StrategyConfig, sink_topics: Vec<String>) -> Self {
        ArbitrageStrategy {
            base: TradingStrategyImpl::new(config, sink_topics),
        }
    }
}
#[async_trait]
impl TradingStrategy<DomainEvent> for ArbitrageStrategy {
    fn name(&self) -> StrategyName {
        StrategyName::Arbitrage
    }
}

#[async_trait]
impl StrategyPort for ArbitrageStrategy {
    type EventType = DomainEvent;
    type TradeSignalType = TradeSignalEvent;

    async fn analyze_market_data(&self, event: Self::EventType) -> Result<Vec<Self::TradeSignalType>, Error> {
        let mut signals = vec![];

        if let DomainEvent::PriceUpdate(price_event) = event {
            if let (Some(kraken_data), Some(coinbase_data)) = (&price_event.kraken_data, &price_event.coinbase_data) {
                let kraken_price = kraken_data.price;
                let coinbase_price = coinbase_data.price;
                let symbol = &kraken_data.pair;

                 let signal = if kraken_price > coinbase_price {
                    Some((TradeSide::Buy, 0.01))
                } else if coinbase_price > kraken_price {
                    Some((TradeSide::Sell, 0.01))
                 } else {
                    None
                };

                if let Some((side, size)) = signal {
                    signals.push(TradeSignalEvent {
                        symbol: symbol.to_string(),
                        side,
                        size,
                        strategy: self.name(),
                    });
                }
            }
        }
        Ok(signals)
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
       vec![
           "price.feed.kraken".to_string(),
           "price.feed.coinbase".to_string(),
        ]
    }
}
