use crate::common::types::Error;
use crate::domain::events::DomainEvent;
use crate::domain::interfaces::EventBusPort;
use log::error;

pub async fn publish_event<T>(
    event_bus: &T,
    event: T::EventType,
    topic: String,
) -> Result<(), Error>
where
    T: EventBusPort,
{
    event_bus
        .publish(event, topic)
        .await
        .map_err(|e| {
            error!("Error publishing event to topic: {}", e);
            e
        })
}


pub fn generic_error_handler(e: anyhow::Error) {
    error!("Error on component: {}", e);
}
