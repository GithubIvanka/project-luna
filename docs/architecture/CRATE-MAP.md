# Project Luna — карта текущих crate

**Статус:** текущая карта реализации.  
**Источник архитектуры:** `docs/ARCHITECTURE.md`.  
**Контракты компонентов:** `docs/architecture/components/`.

Документ описывает реальные границы репозитория и не переопределяет архитектуру.

## Основа

| Crate | Ответственность | Форма |
|---|---|---|
| `luna-common` | небольшие общие типы и идентификаторы | lib |
| `luna-fs` | низкоуровневые filesystem primitives и metadata | lib |
| `luna-root-mapping` | логические пути и semantics mapping | lib |
| `luna-namespace` | Linux namespace/materialization primitives | lib |
| `luna-config` | модель конфигурации и области её применения | lib |

## Policy и state

| Crate | Ответственность | Форма |
|---|---|---|
| `luna-security` | policy, authorization, grants и trust | lib |
| `luna-state` | durable state, storage abstraction и transactions | lib |
| `luna-event` | события, подписки и delivery contracts | lib |

## Bundles и managers

| Crate | Ответственность | Форма |
|---|---|---|
| `luna-bundle` | Bundle domain, manifest, validation и RFC-0002/LBP1 codec | lib |
| `luna-app-manager` | install/import/update/removal/verification/migration | lib + bin при необходимости |
| `luna-system-manager` | модель состояния системы и запросы | lib + bin при необходимости |
| `luna-update-manager` | state-changing updates, checkpoints и rollback | lib + bin при необходимости |
| `luna-kernel-manager` | kernel inventory, metadata и compatibility | lib + bin при необходимости |
| `luna-device-manager` | device/volume discovery и lifecycle | lib + bin при необходимости |

## Runtime, session и login

| Crate | Ответственность | Форма |
|---|---|---|
| `luna-system-runtime` | единственный system-wide runtime/supervisor и orchestration UserSession | lib + bin |
| `luna-user-session` | domain и lifecycle `UserSession` | lib |
| `luna-app-runtime` | execution/lifecycle `ApplicationInstance` | lib + bin при необходимости |
| `luna-login` | graphical login boundary и authentication phase | lib + bin при необходимости |
| `luna-init` | standalone musl early userspace | standalone bin |

`luna-init` сознательно остаётся вне обычного userspace workspace.

## Пользовательские и domain-клиенты

| Crate | Ответственность | Форма |
|---|---|---|
| `luna-cli` | тонкий пользовательский CLI | lib + bin |
| `luna-files` | file-manager client/boundary | lib + bin |
| `luna-audio` | audio domain/provider boundary | lib |
| `luna-network` | network domain/provider boundary | lib |
| `luna-bluetooth` | Bluetooth domain/provider boundary | lib |

Наличие domain crate не означает автоматически отдельный daemon. Например, NetworkManager, PipeWire, BlueZ и D-Bus — implementation infrastructure, если только отдельное решение не создаёт Luna boundary поверх них.

## Boot

`luna-boot.efi` находится в `boot/luna-boot/` и является отдельным UEFI-проектом.

Он владеет UEFI boot boundary, image/kernel selection и boot-time fallback/handoff, но не владеет UserSession или application lifecycle.

## Направление зависимостей

```text
luna-common
    ↑
foundation crates
    ↑
policy/state/bundle/domain
    ↑
managers
    ↑
runtime
    ↑
CLI / GUI clients
```

Нижний слой не должен зависеть от верхнего только ради удобства.

## Runtime hierarchy

```text
luna-system-runtime
├── UserSession A
│   ├── luna-app-runtime
│   │   └── ApplicationInstance(s)
│   └── GUI/Desktop session
└── UserSession B
    ├── luna-app-runtime
    │   └── ApplicationInstance(s)
    └── GUI/Desktop session
```

## Не-Luna boundaries

Следующие вещи являются механизмами реализации, а не новыми архитектурными компонентами сами по себе:

- `setpriv` и аналогичные identity helpers;
- greetd/greeter;
- niri-session wrappers;
- PipeWire/WirePlumber;
- NetworkManager;
- BlueZ;
- D-Bus;
- Yazi.

## Правило развития

Архитектурная подсистема не становится новым crate автоматически. Crate появляется при начале реальной разработки границы и после проверки зависимости с текущей архитектурой.
