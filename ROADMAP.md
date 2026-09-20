# Project Luna — roadmap

Architecture is defined by [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). This file is implementation order only.

## 1. Direct boot integration

- Finish `luna-init` System Environment construction.
- Resolve `PhysicalData` versus `VirtualData` by boot mode.
- Materialize the boot-critical resource closure.
- Start `luna-system-runtime` as a child of `luna-init` and confirm semantic boot success.
- Complete `LunaBootAttempt` lifecycle with minimal persistent writes.

## 2. Boot targets and recovery

- Finish durable `current`, `factory`, `recovery` and fallback state integration.
- Complete soft image fallback without reboot when the loaded kernel remains usable.
- Complete previous-kernel fallback after kernel panic/reboot.
- Complete Recovery DATA Image handling and physical DATA repair workflow.

## 3. Logical root and resources

- Finish eager materialization of all boot-critical resources.
- Finish lazy hydration with source-lifetime independence.
- Harden physical-path, symlink and staging containment.
- Keep physical `LUNA-SYS`/`LUNA-DATA` paths outside application contracts.

## 4. Application execution

- Finish the security-authorized launch path.
- Replace prototype child `pre_exec` setup with an approved production-safe primitive when that design is explicitly approved.
- Complete resource limits, process reconciliation and Bundle install → launch integration.

## 5. System services and desktop

- Complete device/volume hotplug and eject integration.
- Complete network, audio and Bluetooth backends.
- Complete graphical login, Wayland, niri and Noctalia integration.
- Complete file-manager integration.

## 6. Release hardening

- LBP1 conformance and trust verification.
- Signed release artifacts and Secure Boot integration.
- QEMU/OVMF end-to-end regression tests.
- Real UEFI hardware validation.
- Recovery and interrupted-update tests.

No roadmap item authorizes a new architectural component. Architecture changes require discussion and explicit approval first.
