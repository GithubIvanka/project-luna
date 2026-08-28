//! Event-domain contracts for Project Luna.
//!
//! The event layer defines messages and publication/subscription boundaries.
//! It does not embed Kafka or another broker. A concrete async transport can be
//! selected by a higher-level implementation crate.

use std::fmt;
use std::time::SystemTime;

use luna_common::ComponentId;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EventType(String);

impl EventType {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    event_type: EventType,
    source: ComponentId,
    created_at: SystemTime,
    payload: Vec<u8>,
}

impl Event {
    pub fn new(
        event_type: EventType,
        source: ComponentId,
        created_at: SystemTime,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            event_type,
            source,
            created_at,
            payload: payload.into(),
        }
    }

    pub fn event_type(&self) -> &EventType {
        &self.event_type
    }

    pub fn source(&self) -> &ComponentId {
        &self.source
    }

    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventError(String);

impl EventError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for EventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EventError {}

/// Publication contract. A concrete implementation may expose this through
/// Tokio or another transport without making the event model depend on it.
pub trait EventPublisher {
    fn publish(&self, event: Event) -> Result<(), EventError>;
}

/// Subscription contract for higher-level event consumers.
pub trait EventSubscriber {
    fn subscribe(&self, event_type: &EventType) -> Result<Subscription, EventError>;
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SubscriptionId(u64);

impl SubscriptionId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subscription {
    id: SubscriptionId,
    event_type: EventType,
}

impl Subscription {
    pub fn new(id: SubscriptionId, event_type: EventType) -> Self {
        Self { id, event_type }
    }

    pub fn id(&self) -> SubscriptionId {
        self.id.clone()
    }

    pub fn event_type(&self) -> &EventType {
        &self.event_type
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, EventType, EventPublisher, EventSubscriber, Subscription, SubscriptionId};
    use luna_common::ComponentId;
    use std::time::SystemTime;

    struct Publisher;

    impl EventPublisher for Publisher {
        fn publish(&self, _event: Event) -> Result<(), super::EventError> {
            Ok(())
        }
    }

    struct Subscriber;

    impl EventSubscriber for Subscriber {
        fn subscribe(&self, event_type: &EventType) -> Result<Subscription, super::EventError> {
            Ok(Subscription::new(SubscriptionId::new(1), event_type.clone()))
        }
    }

    #[test]
    fn event_contract_round_trips() {
        let event_type = EventType::new("system.update.started");
        let event = Event::new(
            event_type.clone(),
            ComponentId::from("update-manager"),
            SystemTime::UNIX_EPOCH,
            b"payload".to_vec(),
        );

        Publisher.publish(event.clone()).expect("publish");
        let subscription = Subscriber.subscribe(&event_type).expect("subscribe");
        assert_eq!(subscription.event_type(), &event_type);
        assert_eq!(event.payload(), b"payload");
    }
}
