# `luna-bluetooth`

**Status:** domain boundary implemented; provider integration incomplete

## Purpose
Expose Bluetooth device/power/pairing state through a Luna-owned boundary.

## Owns
- Bluetooth state;
- Bluetooth device model;
- provider abstraction;
- lifecycle state exposed to clients.

Current domain includes `BluetoothState` and `BluetoothDevice` concepts.

## Does not own
Kernel Bluetooth implementation, generic device discovery, authorization policy, GUI widgets or application runtime.

## Provider direction
The PC image packages BlueZ (`bluetoothd`/`bluetoothctl`). Packaging the daemon is not the same as implementing the Luna provider contract.

## Dependencies
Device/security/session/event contracts and BlueZ/Linux facilities.

## Open
D-Bus provider, pairing/trust flow, authorization integration and desktop controls remain.
