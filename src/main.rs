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
use futures::future::join_all;
use tokio::time::{sleep, Duration};

use crate::config::Config;
use crate::common::types::Error;
use crate::common::utils::{publish_event, shutdown::ShutdownSignal};
use crate::infra::event_bus::zmq::ZMQEventBus;
use crate::domain::interfaces::{EventBusPort, DataAdapterPort, StrategyPort, BrokerPort, Subscriber};
use crate::domain::events::{DomainEvent, TradeSignalEvent, OrderStatusEvent};
use crate::infra::adapters::{
    kraken::KrakenAdapter,
    coinbase_adapter::CoinbaseAdapter
};
use crate::infra::execution::manager::ExecutionManager;
use crate::infra::broker_adapters::interactive_broker_adapter::InteractiveBrokerAdapter;
use crate::domain::strategies::{
    arbitrage::ArbitrageStrategy,
    mean_reversion::MeanReversionStrategy
};
use crate::domain::models::StrategyConfig;
use crate::common::constants::{DATA_TOPIC, TRADES_TOPIC, ORDER_TOPIC, LOG_TOPIC, METRICS_TOPIC, AUDIT_TOPIC};
use crate::common::enums::{ExchangeName, StrategyName, BrokerName};
use crate::ui::{
    console::ConsoleUI,
    logger::Logger,
    metrics::MetricsSubscriber,
    audit::AuditSubscriber
};

/// Trading platform main structure
struct TradingPlatform {
    config: Config,
    event_bus: Arc<ZMQEventBus>,
    data_adapters: HashMap<String, Box<dyn DataAdapterPort<DomainEvent> + Send + Sync>>,
    strategies: HashMap<String, Box<dyn StrategyPort<EventType = DomainEvent, TradeSignalType = TradeSignalEvent> + Send + Sync>>,
    broker_adapters: HashMap<String, Box<dyn BrokerPort<OrderStatusType = OrderStatusEvent, OrderType = crate::common::enums::OrderType> + Send + Sync>>,
    execution_manager: Arc<Mutex<ExecutionManager>>,
    shutdown: ShutdownSignal,
}

impl TradingPlatform {
    fn initialize_data_adapters(config: &Config) -> Result<HashMap<String, Box<dyn DataAdapterPort<DomainEvent> + Send + Sync>>, Error> {
        let mut adapters = HashMap::new();

        if let Some(exchanges) = &config.components.exchanges {
            for (name, config) in exchanges {
                let adapter: Box<dyn DataAdapterPort<DomainEvent> + Send + Sync> = match name {
                    ExchangeName::Kraken => Box::new(KrakenAdapter::new(config.clone()).unwrap()),
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

     fn initialize_broker_adapters(config: &Config) -> Result<HashMap<String, Box<dyn BrokerPort<OrderStatusType = OrderStatusEvent, OrderType = crate::common::enums::OrderType> + Send + Sync>>, Error> {
        let mut brokers = HashMap::new();

        if let Some(broker_configs) = &config.components.broker {
            for (name, config) in broker_configs {
                let broker: Box<dyn BrokerPort<OrderStatusType = OrderStatusEvent, OrderType = crate::common::enums::OrderType> + Send + Sync> = match name {
                    BrokerName::InteractiveBrokers => Box::new(InteractiveBrokerAdapter::new(config.clone()).unwrap()),
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

    async fn new(config: Config) -> Result<Self, Error> {
        let event_bus = Arc::new(ZMQEventBus::new(config.event_bus.zmq_address.clone()).unwrap());
        let data_adapters = Self::initialize_data_adapters(&config)?;
        let strategies = Self::initialize_strategies(&config)?;
          let broker_adapters = Self::initialize_broker_adapters(&config)?;
        let broker = broker_adapters.get("interactive_broker")
            .ok_or_else(|| Error::ConfigError("Broker not configured".to_string()))?;

        let execution_manager = Arc::new(Mutex::new(ExecutionManager::new(Box::clone(broker))));

        Ok(Self {
            config,
            event_bus,
            data_adapters,
            strategies,
            broker_adapters,
            execution_manager,
            shutdown: ShutdownSignal::new(),
        })
    }

    async fn start_subscriber(
        &self,
        subscriber: Box<dyn Subscriber<DomainEvent> + Send + Sync>,
        config_name: &str,
        topic: String,
    ) -> Result<(), Error> {
        let event_bus = Arc::clone(&self.event_bus);
        let subscriber = Arc::new(Mutex::new(subscriber));

        subscriber.lock().await.start().await?;

        let subscriber_clone = Arc::clone(&subscriber);
        tokio::spawn(async move {
            let subscriber = subscriber_clone.lock().await;
            if let Err(e) = subscriber.subscribe(move |event| {
                let bus = Arc::clone(&event_bus);
                async move {
                    if let Err(e) = publish_event(&bus, event, topic.clone()).await {
                        error!("Failed to publish event for {}: {}", config_name, e);
                    }
                }
            }).await {
                error!("Error starting {} subscriber: {}", config_name, e);
            }
        });

        info!("{} started", config_name);
        Ok(())
    }

     async fn start_subscribers(&self) -> Result<(), Error> {
         if let Some(ui_config) = &self.config.components.ui {
             if ui_config.enabled {
                 self.start_subscriber(
                   Box::new(ConsoleUI::new(ui_config.topics.clone()).unwrap()),
                     "UI",
                     LOG_TOPIC.to_string()
                 ).await?;
             }
         }
     
         if let Some(log_config) = &self.config.components.logging {
             if log_config.enabled {
                 self.start_subscriber(
                   Box::new(Logger::new(log_config.topics.clone()).unwrap()),
                     "Logger",
                     LOG_TOPIC.to_string()
                 ).await?;
             }
         }
     
         if let Some(metrics_config) = &self.config.components.metrics {
             if metrics_config.enabled {
                 self.start_subscriber(
                    Box::new(MetricsSubscriber::new(metrics_config.topics.clone()).unwrap()),
                     "Metrics",
                    METRICS_TOPIC.to_string()
                 ).await?;
             }
         }
     
          if let Some(audit_config) = &self.config.components.audit {
              if audit_config.enabled {
                  self.start_subscriber(
                   Box::new(AuditSubscriber::new(audit_config.topics.clone()).unwrap()),
                      "Audit",
                      AUDIT_TOPIC.to_string()
                  ).await?;
              }
          }
     
         Ok(())
     }


    async fn start_data_adapters(&self) -> Result<(), Error> {
        for (name, adapter) in self.data_adapters.iter() {
            let adapter = Box::clone(adapter);
            let event_bus = Arc::clone(&self.event_bus);
            let sink_topics = adapter.get_sink_topics();

            tokio::spawn(async move {
                if let Err(e) = adapter.start(move |event| {
                    let bus = Arc::clone(&event_bus);
                    let topics = sink_topics.clone();
                    async move {
                        for topic in topics.iter() {
                            if let Err(e) = publish_event(&bus, event.clone(), topic.to_string()).await {
                                error!("Failed to publish event for adapter {}: {}", name, e);
                            }
                        }
                    }
                }).await {
                    error!("Error starting data adapter {}: {}", name, e);
                }
            });

            info!("Started data adapter: {}", name);
        }
        Ok(())
    }

    async fn start_strategies(&self) -> Result<(), Error> {
      for (name, strategy) in self.strategies.iter() {
            let strategy = Box::clone(strategy);
            let event_bus = Arc::clone(&self.event_bus);

            tokio::spawn(async move {
                let config = strategy.get_strategy_config().await.unwrap();
                if !config.enabled {
                    info!("Strategy {} is not enabled", name);
                    return Ok(());
                }

                let source_topics = strategy.get_source_topics();
                 let sink_topics = strategy.get_sink_topics();
                 
                 strategy.start(move |event| {
                    let bus = Arc::clone(&event_bus);
                     let topics = sink_topics.clone();
                     async move {
                        for topic in topics.iter() {
                         if let Err(e) = publish_event(&bus, DomainEvent::TradeSignal(event.clone()), topic.to_string()).await {
                                    error!("Failed to publish signal for strategy {}: {}", name, e);
                                }
                            }
                        }
                   }).await?;


                event_bus.subscribe_many(source_topics, move |event: DomainEvent| {
                    let strategy = Box::clone(&strategy);
                    let bus = Arc::clone(&event_bus);

                    async move {
                        match strategy.analyze_market_data(event).await {
                            Ok(signals) => {
                                for signal in signals {
                                    if let Err(e) = publish_event(&bus, DomainEvent::TradeSignal(signal), TRADES_TOPIC.to_string()).await {
                                        error!("Failed to publish trade signal: {}", e);
                                    }
                                }
                            }
                            Err(e) => error!("Error in strategy {}: {}", name, e),
                        }
                    }
                }).await?;

                info!("Started strategy: {}", name);
                Ok::<(), Error>(())
            });
        }
        Ok(())
    }

    async fn start_execution_manager(&self) -> Result<(), Error> {
        let execution_manager = Arc::clone(&self.execution_manager);
        let event_bus = Arc::clone(&self.event_bus);

        // Subscribe to trade signals
        let exec_clone = Arc::clone(&execution_manager);
        event_bus.subscribe(TRADES_TOPIC.to_string(), move |event: DomainEvent| {
            let exec = Arc::clone(&exec_clone);

            async move {
                if let DomainEvent::TradeSignal(signal) = event {
                    if let Err(e) = exec.lock().await.handle_trade_signal(signal).await {
                        error!("Error handling trade signal: {}", e);
                    }
                }
            }
        }).await?;

        // Start execution manager
        let exec_manager = execution_manager.lock().await;
        exec_manager.start(move |event| {
            let bus = Arc::clone(&event_bus);
            async move {
                if let Err(e) = publish_event(&bus, DomainEvent::OrderStatus(event), ORDER_TOPIC.to_string()).await {
                    error!("Failed to publish order status: {}", e);
                }
            }
        }).await?;

        info!("Execution Manager started");
        Ok(())
    }

    async fn run(&self) -> Result<(), Error> {
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

    let config = Config::load().expect("Failed to load configuration");
    let platform = TradingPlatform::new(config).await?;

    if let Err(e) = platform.run().await {
          error!("Error running the platform: {}", e);
        }


    Ok(())
}
