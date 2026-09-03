# `luna-event`

**Status:** implemented event-domain boundary

## Purpose
Define Luna event and subscription contracts independently of the final broker/transport.

## Owns
- `EventType`;
- `Event`;
- publisher/subscriber contracts;
- subscriptions and delivery semantics.

## Contract
Events and Operations are different concepts. Events may be Ephemeral, Persistent or Audit. Persistent/Audit events support history/replay where required. Delivery is bounded and backpressure-aware; Audit events must not be silently dropped.

Operation sequence is monotonic within an operation when one exists; timestamps are metadata and not ordering authority.

## Does not own
A specific broker, GUI notification system, persistence database, authorization policy or operation execution.

## Dependencies
Minimal shared values. Tokio is an accepted higher-level async direction for components that need it, but this domain crate need not depend on Tokio merely to define its contract.

## Open
Final transport/broker and durable event persistence are integration decisions.
