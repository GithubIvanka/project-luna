//! Event-domain contracts and a deterministic in-memory delivery prototype.
use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::time::SystemTime;
use luna_common::ComponentId;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)] pub struct EventType(String);
impl EventType { pub fn new(value: impl Into<String>) -> Self { Self(value.into()) } pub fn as_str(&self) -> &str { &self.0 } }
#[derive(Clone, Debug, Eq, PartialEq)] pub struct Event { event_type: EventType, source: ComponentId, created_at: SystemTime, payload: Vec<u8> }
impl Event { pub fn new(event_type: EventType, source: ComponentId, created_at: SystemTime, payload: impl Into<Vec<u8>>) -> Self { Self { event_type, source, created_at, payload: payload.into() } } pub fn event_type(&self) -> &EventType { &self.event_type } pub fn source(&self) -> &ComponentId { &self.source } pub fn created_at(&self) -> SystemTime { self.created_at } pub fn payload(&self) -> &[u8] { &self.payload } }
#[derive(Clone, Debug, Eq, PartialEq)] pub struct EventError(String);
impl EventError { pub fn new(message: impl Into<String>) -> Self { Self(message.into()) } }
impl fmt::Display for EventError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) } }
impl std::error::Error for EventError {}
pub trait EventPublisher { fn publish(&mut self, event: Event) -> Result<(), EventError>; }
pub trait EventSubscriber { fn subscribe(&mut self, event_type: &EventType) -> Result<Subscription, EventError>; fn receive(&mut self, subscription: &Subscription) -> Result<Option<Event>, EventError>; }
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)] pub struct SubscriptionId(u64);
impl SubscriptionId { pub const fn new(value: u64) -> Self { Self(value) } pub const fn get(self) -> u64 { self.0 } }
#[derive(Clone, Debug, Eq, PartialEq)] pub struct Subscription { id: SubscriptionId, event_type: EventType }
impl Subscription { pub fn new(id: SubscriptionId, event_type: EventType) -> Self { Self { id, event_type } } pub fn id(&self) -> SubscriptionId { self.id.clone() } pub fn event_type(&self) -> &EventType { &self.event_type } }

/// Synchronous in-memory delivery boundary for contract tests. It provides
/// per-subscription FIFO delivery without selecting a broker implementation.
#[derive(Default)] pub struct InMemoryEventBus { next_id: u64, queues: BTreeMap<SubscriptionId, (EventType, VecDeque<Event>)> }
impl InMemoryEventBus { pub fn new() -> Self { Self::default() } }
impl EventPublisher for InMemoryEventBus {
    fn publish(&mut self, event: Event) -> Result<(), EventError> { for (kind, queue) in self.queues.values_mut() { if kind == event.event_type() { queue.push_back(event.clone()); } } Ok(()) }
}
impl EventSubscriber for InMemoryEventBus {
    fn subscribe(&mut self, event_type: &EventType) -> Result<Subscription, EventError> { let id = SubscriptionId::new(self.next_id); self.next_id = self.next_id.saturating_add(1); self.queues.insert(id.clone(), (event_type.clone(), VecDeque::new())); Ok(Subscription::new(id, event_type.clone())) }
    fn receive(&mut self, subscription: &Subscription) -> Result<Option<Event>, EventError> { let (_, queue) = self.queues.get_mut(&subscription.id).ok_or_else(|| EventError::new("unknown subscription"))?; Ok(queue.pop_front()) }
}

#[cfg(test)]
mod tests {
    use super::{Event, EventPublisher, EventSubscriber, EventType, InMemoryEventBus}; use luna_common::ComponentId; use std::time::SystemTime;
    #[test] fn subscribers_receive_only_matching_events_in_fifo_order() {
        let mut bus = InMemoryEventBus::new(); let kind = EventType::new("system.update.started"); let other = EventType::new("system.update.finished"); let sub = bus.subscribe(&kind).unwrap();
        bus.publish(Event::new(kind.clone(), ComponentId::from("update-manager"), SystemTime::UNIX_EPOCH, b"one".to_vec())).unwrap();
        bus.publish(Event::new(other, ComponentId::from("update-manager"), SystemTime::UNIX_EPOCH, b"two".to_vec())).unwrap();
        assert_eq!(bus.receive(&sub).unwrap().unwrap().payload(), b"one"); assert!(bus.receive(&sub).unwrap().is_none());
    }
}
