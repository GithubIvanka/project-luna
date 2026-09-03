# `luna-audio`

**Status:** domain boundary implemented; provider integration incomplete

## Purpose
Expose Luna's audio domain independently of the desktop implementation.

## Owns
- audio state;
- endpoint/device domain model;
- volume/routing operations at the Luna boundary;
- provider abstraction.

Current domain includes `Volume`, `AudioState` and `AudioEndpoint` concepts.

## Does not own
PipeWire internals, authorization policy, GUI widgets, UserSession lifecycle or generic device discovery.

## Provider direction
The current PC image packages PipeWire, PipeWire-Pulse and WirePlumber. This is infrastructure packaging, not proof of a fully integrated Luna audio provider.

## Dependencies
Shared domain values, security/session context where needed, and the selected Linux audio stack.

## Open
D-Bus/daemon provider integration, per-user/session routing and Noctalia control integration remain.
