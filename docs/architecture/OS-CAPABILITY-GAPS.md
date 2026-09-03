# Project Luna — OS Capability Gap Map

**Status:** planning baseline
**Authority:** current architecture and accepted decisions

This document answers a different question from the component contracts: which capabilities are still needed before Luna can reasonably be called a complete daily-use PC operating system.

## 1. Already represented architectural boundaries

The current architecture already has explicit boundaries for:

- boot (`luna-boot.efi`);
- System Images;
- logical root and mapping;
- filesystem primitives;
- namespaces;
- security;
- durable state;
- events/operations;
- Bundles;
- application/system/update/kernel/device management;
- system runtime;
- UserSession;
- application runtime;
- graphical login;
- CLI;
- file manager;
- audio/network/Bluetooth domains.

Having a boundary does not mean that its implementation is complete.

## 2. Missing or incomplete capabilities

### A. End-to-end boot and handoff

Needed:

- real System Image discovery from SYSTEM;
- manifest validation and kernel compatibility selection;
- complete fallback state machine;
- final kernel → logical-root → system-runtime handoff;
- QEMU/OVMF and eventually real-hardware smoke coverage.

### B. System Image specification

Needed:

- final manifest schema;
- image identity/integrity rules;
- exact boot metadata;
- compatibility semantics;
- retention metadata and factory representation.

The payload itself remains SquashFS.

### C. Logical root implementation

Needed:

- production-grade lazy/hybrid System Image access;
- complete DATA/user/system mapping;
- robust lifetime/materialization semantics;
- integration with per-application namespaces.

### D. Security enforcement

Needed:

- durable policy representation;
- actual kernel enforcement of grants/denies;
- user-mediated permission UI/IPC;
- trust store and publisher/repository trust flow;
- device/volume authorization enforcement.

### E. Application management/runtime

Needed:

- complete Bundle install transaction;
- dependency resolution;
- package import (`.deb`/`.rpm`) hardening;
- ApplicationInstance supervision integrated with real namespaces and security;
- application data cleanup/retention UX.

### F. UserSession and graphical login

Needed:

- production authentication IPC/security integration;
- complete graphical-session environment construction;
- real session lifecycle for multiple concurrent users;
- user switching/restricted-session policy;
- logout/restart/re-authentication handling.

No separate session-manager component is required by the accepted architecture.

### G. Device and storage management

Needed:

- real device discovery;
- automount lifecycle;
- friendly volume presentation;
- safe unmount/eject;
- removable-media policy;
- filesystem error handling;
- device permission integration.

### H. Networking

Needed:

- NetworkManager/D-Bus provider integration;
- real state/events exposed to Luna clients;
- connection management UI;
- policy/security integration.

### I. Audio

Needed:

- PipeWire/WirePlumber provider integration;
- device/profile/volume routing;
- session/user policy integration;
- desktop control integration.

### J. Bluetooth

Needed:

- BlueZ provider integration;
- pairing/trust state;
- device lifecycle and authorization;
- desktop control integration.

### K. Desktop shell

Needed:

- reliable niri session startup;
- Noctalia integration;
- icon/theme completeness;
- notifications/power/session controls;
- application launch integration.

### L. File manager

Needed:

- same-window navigation;
- real file operations;
- volume integration;
- permissions/error presentation;
- eventual direct use of the accepted filesystem backend model.

Packaging Yazi is not evidence that Luna Files already uses `yazi-core` directly.

### M. Updates and rollback

Needed:

- durable operation reconciliation;
- complete System Image update transaction;
- independent kernel update path;
- checkpoint creation/rollback integration;
- health-gated automatic rollback;
- recovery/factory integration.

### N. Diagnostics

Needed:

- structured health collection;
- DiagnosticReport generation;
- bounded automatic repair coordination;
- export to external media;
- privacy-aware filtering.

### O. Installer / first boot

Needed:

- installation media;
- disk/partition provisioning;
- initial administrator/user creation;
- secure credential setup;
- factory-state creation;
- initial System Image/kernel registration.

### P. Power and hardware lifecycle

Needed:

- suspend/resume;
- shutdown/reboot orchestration;
- battery/AC state where applicable;
- thermal/power policy;
- display hotplug.

### Q. Compatibility and hardware enablement

Needed:

- broad GPU/input/storage/network/audio hardware coverage;
- kernel module/driver lifecycle contract;
- firmware handling;
- hardware capability discovery.

## 3. Completion criterion

Luna should not be considered a complete PC OS merely because an image builds. A credible completion gate is:

```text
install
 ↓
UEFI boot
 ↓
System Image + compatible kernel
 ↓
logical /
 ↓
system runtime
 ↓
graphical authentication
 ↓
UserSession
 ↓
niri + Noctalia
 ↓
applications
 ↓
files / network / audio / bluetooth / removable media
 ↓
update + rollback + recovery
 ↓
shutdown / reboot / resume
```

Each arrow needs executable integration evidence, not only a configuration file or placeholder.

## 4. Important distinction

This list is a gap map, not permission to create one component per bullet. Many capabilities belong inside existing boundaries or can be provided by Linux/third-party daemons under Luna ownership. New Luna components require the AI development rules and an explicit architectural decision.
