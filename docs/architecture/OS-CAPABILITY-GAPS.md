# Project Luna — карта недостающих возможностей ОС

**Статус:** актуальная planning baseline.  
**Источник:** текущая архитектура и принятые решения.

Этот документ отвечает на вопрос: какие возможности ещё необходимы, прежде чем Luna можно будет назвать полноценной ежедневной PC-операционной системой. Наличие архитектурной границы не означает, что её реализация завершена.

## 1. Уже определённые границы

Архитектура уже имеет отдельные границы для:

- `luna-boot.efi`;
- System Images;
- logical root и Root Mapping;
- filesystem primitives;
- namespaces;
- security;
- durable state;
- events/operations;
- Bundles;
- managers приложений, системы, updates, kernels и устройств;
- system runtime;
- UserSession;
- application runtime;
- graphical login;
- CLI;
- file manager;
- audio/network/Bluetooth domains.

## 2. Что ещё необходимо

### A. Boot и handoff

- реальное обнаружение System Images в SYSTEM;
- проверка manifest;
- выбор совместимого kernel;
- полная fallback state machine;
- окончательный kernel → logical root → system-runtime handoff;
- QEMU/OVMF и затем hardware smoke coverage.

### B. System Image

- окончательная схема manifest;
- identity и integrity rules;
- boot metadata;
- compatibility semantics;
- retention и представление factory.

Payload остаётся непосредственно SquashFS.

### C. Logical root

- production-grade lazy/hybrid доступ к System Image;
- полный mapping SYSTEM/DATA/user/system;
- lifetime/materialization semantics;
- интеграция с application namespaces.

### D. Security

- durable policy representation;
- реальное kernel enforcement grants/denies;
- user-mediated permission UI/IPC;
- trust store и publisher/repository trust flow;
- enforcement для devices/volumes.

### E. Application management/runtime

- полный Bundle install transaction;
- dependency resolution;
- hardening import `.deb`/`.rpm`;
- supervision `ApplicationInstance` вместе с security/namespace;
- cleanup/retention application data.

### F. UserSession и graphical login

- production authentication IPC/security integration;
- полное формирование graphical-session environment;
- lifecycle нескольких concurrent UserSessions;
- user switching и restricted-session policy;
- logout/restart/re-authentication.

Отдельный session-manager компонент не нужен.

### G. Devices/storage

- real device discovery;
- automount lifecycle;
- friendly volume presentation;
- safe unmount/eject;
- removable-media policy;
- filesystem error handling;
- device permission integration.

### H. Networking

- provider integration;
- state/events для Luna clients;
- connection UI;
- security/policy integration.

Текущий implementation direction использует NetworkManager как provider.

### I. Audio

- PipeWire/WirePlumber integration;
- device/profile/volume routing;
- session/user policy;
- desktop controls.

### J. Bluetooth

- BlueZ integration;
- pairing/trust state;
- lifecycle и authorization;
- desktop controls.

### K. Desktop

- надёжный niri startup;
- Noctalia integration;
- themes/icons;
- notifications/power/session controls;
- application launch integration.

### L. Files

- полноценные file operations;
- volumes;
- permissions/error presentation;
- интеграция с filesystem backend;
- file access/portal model для приложений.

Установка Yazi или другого file manager не доказывает наличие прямой integration с его internal API.

### M. Updates/rollback

- durable reconciliation;
- complete System Image transaction;
- independent kernel update path;
- checkpoint/rollback integration;
- health-gated automatic rollback;
- Recovery/Factory integration.

### N. Diagnostics

- structured health collection;
- `DiagnosticReport`;
- bounded repair coordination;
- export to external media;
- privacy-aware filtering.

### O. Installer/first boot

- installation media;
- disk/partition provisioning;
- initial user/admin creation;
- secure credential setup;
- factory-state creation;
- initial image/kernel registration.

### P. Power/hardware lifecycle

- suspend/resume;
- shutdown/reboot orchestration;
- battery/AC state;
- thermal/power policy;
- display hotplug.

### Q. Hardware enablement

- широкое покрытие GPU/input/storage/network/audio;
- kernel module/driver lifecycle contract;
- firmware handling;
- capability discovery.

## 3. Критерий завершения

Luna не считается полноценной PC ОС только потому, что собирается image.

```text
install
 ↓
UEFI boot
 ↓
System Image + compatible kernel
 ↓
luna-init
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
files / network / audio / Bluetooth / removable media
 ↓
update + rollback + recovery
 ↓
shutdown / reboot / resume
```

Каждая стрелка должна иметь исполняемое integration evidence.

## 4. Важное правило

Это gap map, а не разрешение создавать отдельный crate для каждого пункта. Возможность должна принадлежать существующей границе или использовать Linux/upstream provider, если это соответствует архитектуре. Новый Luna component требует отдельного архитектурного решения.
