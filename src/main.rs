// src/main.rs
mod config;
mod domain;
mod infra;
mod common;
mod ui;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{error, info, warn};
use tokio::time::{sleep, Duration};

use crate::config::Config;
use crate::common::types::Error;
use crate::common::shutdown::ShutdownSignal;
use crate::infra::event_bus::zmq::ZMQEventBus;
use crate::domain::interfaces::{EventBusPort, DataAdapterPort, StrategyPort, BrokerPort, Subscriber};
use crate::domain::events::{DomainEvent, TradeSignalEvent, OrderStatusEvent};
use crate::domain::models::{Order, StrategyConfig};

use crate::infra::adapters::{
    kraken::KrakenAdapter,
    coinbase::CoinbaseAdapter
};
use crate::infra::execution::manager::ExecutionManager;
use crate::infra::broker_adapters::interactive_broker_adapter::InteractiveBrokerAdapter;
use crate::domain::strategies::{
    arbitrage::ArbitrageStrategy,
    mean_reversion::MeanReversionStrategy
};
use crate::common::enums::{ExchangeName, StrategyName, BrokerName};
use crate::common::constants::{
    DATA_TOPIC,
    TRADES_TOPIC,
    ORDERS_TOPIC,
    LOG_TOPIC,
    METRICS_TOPIC,
    AUDIT_TOPIC
};
use crate::ui::{
    console::ConsoleUI,
    logger::Logger,
    metrics::MetricsSubscriber,
    audit::AuditSubscriber
};

/// Trading platform main structure
pub struct TradingPlatformBuilder {
    config: Config,
}

impl TradingPlatformBuilder {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn build(self) -> Result<TradingPlatform, Error> {
        let event_bus = Arc::new(ZMQEventBus::new(self.config.event_bus.zmq_address.clone())?);
        let data_adapters = TradingPlatform::initialize_data_adapters(&self.config)?;
        let strategies = TradingPlatform::initialize_strategies(&self.config)?;
        let broker_adapters = TradingPlatform::initialize_broker_adapters(&self.config)?;
        
        let broker = broker_adapters.get("interactivebrokers")
            .ok_or_else(|| Error::ConfigError("InteractiveBrokers not configured".to_string()))?;

        let execution_manager = Arc::new(Mutex::new(ExecutionManager::new(Box::clone(broker))));

        Ok(TradingPlatform::new(
            self.config,
            event_bus,
            data_adapters,
            strategies,
            broker_adapters,
            execution_manager,
        ))
    }
}

pub struct TradingPlatform {
    config: Config,
    event_bus: Arc<ZMQEventBus>,
    data_adapters: HashMap<String, Box<dyn DataAdapterPort<DomainEvent> + Send + Sync>>,
    strategies: HashMap<String, Box<dyn StrategyPort<EventType = DomainEvent, TradeSignalType = TradeSignalEvent> + Send + Sync>>,
    broker_adapters: HashMap<String, Box<dyn BrokerPort<OrderStatusType = OrderStatusEvent, OrderType = Order> + Send + Sync>>,
    execution_manager: Arc<Mutex<ExecutionManager>>,
    shutdown: ShutdownSignal,
}

impl TradingPlatform {
    pub fn builder(config: Config) -> Result<TradingPlatformBuilder, Error> {
        Ok(TradingPlatformBuilder::new(config))
    }

    fn initialize_data_adapters(config: &Config) -> Result<HashMap<String, Box<dyn DataAdapterPort<DomainEvent> + Send + Sync>>, Error> {
        let mut adapters = HashMap::new();

        if let Some(exchanges) = &config.components.exchanges {
            for (name, config) in exchanges {
                let adapter: Box<dyn DataAdapterPort<DomainEvent> + Send + Sync> = match name {
                    ExchangeName::Kraken => Box::new(KrakenAdapter::new(config.clone())?),
                    ExchangeName::Coinbase => Box::new(CoinbaseAdapter::new(config.clone())),
                    _ => {
                        warn!("Exchange {} not implemented", name);
                        continue;
                    }
                };
                adapters.insert(name.to_string().to_lowercase(), adapter);
            }
        }

        if adapters.is_empty() {
            return Err(Error::ConfigError("No data adapters configured".to_string()));
        }

        Ok(adapters)
    }

    fn initialize_strategies(config: &Config) -> Result<HashMap<String, Box<dyn StrategyPort<EventType = DomainEvent, TradeSignalType = TradeSignalEvent> + Send + Sync>>, Error> {
        let mut strategies = HashMap::new();

        if let Some(strategy_configs) = &config.components.strategies {
            for (name, config) in strategy_configs {
                let strategy: Box<dyn StrategyPort<EventType = DomainEvent, TradeSignalType = TradeSignalEvent> + Send + Sync> = match name {
                    StrategyName::Arbitrage => Box::new(ArbitrageStrategy::new(
                        StrategyConfig::new(name.clone(), config.params.clone(), config.enabled),
                        vec![TRADES_TOPIC.to_string()]
                    )),
                    StrategyName::MeanReversion => Box::new(MeanReversionStrategy::new(
                        StrategyConfig::new(name.clone(), config.params.clone(), config.enabled),
                        vec![TRADES_TOPIC.to_string()]
                    )),
                    _ => {
                        warn!("Strategy {} not implemented", name);
                        continue;
                    }
                };
                strategies.insert(name.to_string().to_lowercase(), strategy);
            }
        }

        if strategies.is_empty() {
            return Err(Error::ConfigError("No strategies configured".to_string()));
        }

        Ok(strategies)
    }

    fn initialize_broker_adapters(config: &Config) -> Result<HashMap<String, Box<dyn BrokerPort<OrderStatusType = OrderStatusEvent, OrderType = Order> + Send + Sync>>, Error> {
        let mut brokers = HashMap::new();

        if let Some(broker_configs) = &config.components.broker {
            for (name, config) in broker_configs {
                let broker: Box<dyn BrokerPort<OrderStatusType = OrderStatusEvent, OrderType = Order> + Send + Sync> = match name {
                    BrokerName::InteractiveBrokers => Box::new(InteractiveBrokerAdapter::new(config.clone())?),
                    _ => {
                        warn!("Broker {} not implemented", name);
                        continue;
                    }
                };
                brokers.insert(name.to_string().to_lowercase(), broker);
            }
        }

        if brokers.is_empty() {
            return Err(Error::ConfigError("No broker adapters configured".to_string()));
        }

        Ok(brokers)
    }

    fn new(
        config: Config,
        event_bus: Arc<ZMQEventBus>,
        data_adapters: HashMap<String, Box<dyn DataAdapterPort<DomainEvent> + Send + Sync>>,
        strategies: HashMap<String, Box<dyn StrategyPort<EventType = DomainEvent, TradeSignalType = TradeSignalEvent> + Send + Sync>>,
        broker_adapters: HashMap<String, Box<dyn BrokerPort<OrderStatusType = OrderStatusEvent, OrderType = Order> + Send + Sync>>,
        execution_manager: Arc<Mutex<ExecutionManager>>,
    ) -> Self {
        Self {
            config,
            event_bus,
            data_adapters,
            strategies,
            broker_adapters,
            execution_manager,
            shutdown: ShutdownSignal::new(),
        }
    }

    async fn start_subscriber(
        &self,
        mut subscriber: Box<dyn Subscriber<DomainEvent> + Send + Sync>,
        topic: String,
        name: String,
    ) -> Result<(), Error> {
        subscriber.start().await?;

        let event_bus = Arc::clone(&self.event_bus);
        let name_clone = name.clone();

        subscriber.subscribe(move |event| {
            let event_bus = Arc::clone(&event_bus);
            let topic = topic.clone();
            let name = name_clone.clone();

            tokio::spawn(async move {
                if let Err(e) = event_bus.publish(event, topic).await {
                    error!("Failed to publish event for {}: {}", name, e);
                }
            });
        }).await?;

        info!("{} subscriber started", name);
        Ok(())
    }

    async fn start_subscribers(&self) -> Result<(), Error> {
        if let Some(ui_config) = &self.config.components.ui {
            if ui_config.enabled {
                let ui = ConsoleUI::new(ui_config.topics.clone())?;
                self.start_subscriber(Box::new(ui), LOG_TOPIC.to_string(), "UI".to_string()).await?;
            }
        }

        if let Some(log_config) = &self.config.components.logging {
            if log_config.enabled {
                let logger = Logger::new(log_config.topics.clone())?;
                self.start_subscriber(Box::new(logger), LOG_TOPIC.to_string(), "Logger".to_string()).await?;
            }
        }

        if let Some(metrics_config) = &self.config.components.metrics {
            if metrics_config.enabled {
                let metrics = MetricsSubscriber::new(metrics_config.topics.clone())?;
                self.start_subscriber(Box::new(metrics), METRICS_TOPIC.to_string(), "Metrics".to_string()).await?;
            }
        }

        if let Some(audit_config) = &self.config.components.audit {
            if audit_config.enabled {
                let audit = AuditSubscriber::new(audit_config.topics.clone())?;
                self.start_subscriber(Box::new(audit), AUDIT_TOPIC.to_string(), "Audit".to_string()).await?;
            }
        }

        Ok(())
    }

    async fn start_data_adapters(&self) -> Result<(), Error> {
        for (name, adapter) in &self.data_adapters {
            let adapter = Arc::new(adapter);
            let event_bus = Arc::clone(&self.event_bus);
            let name = name.clone();
            let sink_topics = adapter.get_sink_topics();

            adapter.start(move |event| {
                let event_bus = Arc::clone(&event_bus);
                let name = name.clone();
                let topics = sink_topics.clone();

                tokio::spawn(async move {
                    for topic in topics {
                        if let Err(e) = event_bus.publish(event.clone(), topic).await {
                            error!("Failed to publish event for adapter {}: {}", name, e);
                        }
                    }
                });
            }).await?;

            info!("Started data adapter: {}", name);
        }
        Ok(())
    }

    async fn start_strategies(&self) -> Result<(), Error> {
        for (name, strategy) in &self.strategies {
            let strategy = Arc::new(strategy);
            let event_bus = Arc::clone(&self.event_bus);
            let name = name.clone();

            let config = strategy.get_strategy_config().await?;
            if !config.enabled {
                info!("Strategy {} is not enabled", name);
                continue;
            }

            strategy.start(move |signal| {
                let event_bus = Arc::clone(&event_bus);
                let name = name.clone();

                tokio::spawn(async move {
                    if let Err(e) = event_bus
                        .publish(DomainEvent::TradeSignal(signal), TRADES_TOPIC.to_string())
                        .await 
                    {
                        error!("Failed to publish signal for strategy {}: {}", name, e);
                    }
                });
            }).await?;

            // Subscribe to market data events for this strategy
            let strategy_clone = Arc::clone(&strategy);
            let name_clone = name.clone();
            let event_bus_clone = Arc::clone(&event_bus);

            event_bus.subscribe_many(strategy.get_source_topics(), move |event| {
                let strategy = Arc::clone(&strategy_clone);
                let event_bus = Arc::clone(&event_bus_clone);
                let name = name_clone.clone();

                tokio::spawn(async move {
                    match strategy.analyze_market_data(event).await {
                        Ok(signals) => {
                            for signal in signals {
                                if let Err(e) = event_bus
                                    .publish(DomainEvent::TradeSignal(signal), TRADES_TOPIC.to_string())
                                    .await 
                                {
                                    error!("Failed to publish trade signal for {}: {}", name, e);
                                }
                            }
                        }
                        Err(e) => error!("Error in strategy {}: {}", name, e),
                    }
                });
            }).await?;

            info!("Started strategy: {}", name);
        }
        Ok(())
    }

    async fn start_execution_manager(&self) -> Result<(), Error> {
        let execution_manager = Arc::clone(&self.execution_manager);
        let event_bus = Arc::clone(&self.event_bus);

        // Subscribe to trade signals
        let exec_clone = Arc::clone(&execution_manager);
        event_bus.subscribe(TRADES_TOPIC.to_string(), move |event| {
            let exec = Arc::clone(&exec_clone);
            tokio::spawn(async move {
                if let DomainEvent::TradeSignal(signal) = event {
                    if let Err(e) = exec.lock().await.handle_trade_signal(signal).await {
                        error!("Error handling trade signal: {}", e);
                    }
                }
            });
        }).await?;

        // Start execution manager
        let exec_manager = execution_manager.lock().await;
        exec_manager.start(move |event| {
            let event_bus = Arc::clone(&event_bus);
            tokio::spawn(async move {
                if let Err(e) = event_bus.publish(DomainEvent::OrderStatus(event), ORDERS_TOPIC.to_string()).await {
                    error!("Failed to publish order status: {}", e);
                }
            });
        }).await?;

        info!("Execution Manager started");
        Ok(())
    }

    pub async fn run(&self) -> Result<(), Error> {
        self.start_subscribers().await?;
        self.start_data_adapters().await?;
        self.start_strategies().await?;
        self.start_execution_manager().await?;

        while !self.shutdown.is_shutdown() {
            sleep(Duration::from_secs(1)).await;
        }

        info!("Shutting down trading platform...");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let config = Config::load()?;
    let platform = TradingPlatform::builder(config)?.build().await?;

    if let Err(e) = platform.run().await {
        error!("Error running the platform: {}", e);
    }

    Ok(())
}
