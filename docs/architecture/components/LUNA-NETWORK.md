# `luna-network`

**Status:** domain boundary implemented; provider integration incomplete

## Purpose
Expose network connection/device state through a Luna-owned domain boundary.

## Owns
- network state;
- network-device domain model;
- connection/provider abstraction.

Current domain includes `NetworkState` and `NetworkDevice` concepts.

## Does not own
NetworkManager internals, raw device discovery, authorization policy, GUI presentation or application namespace policy.

## Provider direction
The PC image packages NetworkManager and `nmcli`. This is infrastructure packaging, not complete Luna provider integration.

## Dependencies
Device/security/session/event contracts and the selected Linux network service.

## Open
D-Bus provider integration, connection management API/events, UI integration and policy enforcement remain.
