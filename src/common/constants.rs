// src/common/constants.rs
pub const DATA_TOPIC: &str = "data";
pub const TRADES_TOPIC: &str = "trades";
pub const ORDERS_TOPIC: &str = "orders"; // Note: Changed from ORDER_TOPIC
pub const LOG_TOPIC: &str = "log";
pub const METRICS_TOPIC: &str = "metrics";
pub const AUDIT_TOPIC: &str = "audit";

pub mod topics {
    pub use super::*;
}

pub mod timeouts {
    pub const DEFAULT_TIMEOUT_MS: u64 = 5000;
    pub const NETWORK_TIMEOUT_MS: u64 = 10000;
    pub const ORDER_TIMEOUT_MS: u64 = 30000;
}

pub mod rate_limits {
    pub const MAX_REQUESTS_PER_SECOND: u32 = 10;
    pub const MAX_ORDERS_PER_SECOND: u32 = 5;
}
