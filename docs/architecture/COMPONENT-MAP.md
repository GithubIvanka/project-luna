# Карта компонентов

Активная архитектура определяется `docs/ARCHITECTURE.md` и компонентами утверждённого workspace/boot integration.

## Граница загрузки

| Компонент | Ответственность | Связи |
|---|---|---|
| `luna-boot.efi` | UEFI discovery, выбор target, Boot Menu, загрузка артефактов и handoff | UEFI, `LUNA-SYS`, `LUNA-DATA`, Linux boot protocol |
| Linux Luna kernel integration | проверка handoff и прямой запуск memory-resident `luna-init` | `setup_data`, Linux exec/binfmt |
| `luna-init` | PID 1, ранний bootstrap системы | handoff, System Image, DATA, дочерний `luna-system-runtime` |
| `luna-system-runtime` | долгоживущий system runtime и supervisor | managers, events, UserSession |

## Runtime и приложения

| Компонент | Ответственность |
|---|---|
| `luna-user-session` | единая UserSession boundary: identity, auth, credentials, seat, input, DRM/KMS, compositor и session UI |
| `luna-app-runtime` | lifecycle `ApplicationInstance` и координация запуска |
| `luna-app-manager` | lifecycle установленных Bundle |
| `luna-bundle` | модель Bundle и LBP1 codec |
| `luna-root-mapping` | логический mapping и построение `MappingPlan` |
| `luna-security` | trust и authorization policy |
| `luna-namespace` | Linux namespace и materialization |

## Физическая группировка workspace

```text
components/
├── core/
│   ├── boot-adjacent foundation
│   ├── luna-init
│   ├── luna-system-runtime
│   ├── luna-user-session
│   ├── security / namespace / mapping
│   ├── filesystem / state / config / events
│   └── system target / device / kernel / update managers
├── system/
│   ├── luna-app-manager
│   └── luna-cli
├── apps/
│   └── обычные пользовательские приложения
└── external/
    ├── providers/
    │   ├── luna-audio
    │   ├── luna-network
    │   ├── luna-bluetooth
    │   └── luna-files
    └── libraries/
```

Каждый активный компонент имеет отдельный документ в `docs/architecture/components/`. Папка компонента отражает архитектурную роль и не означает отдельный runtime process.

## Разрешение boot target

Bootloader разрешает артефакты последовательно:

```text
System Image manifest
    ↓ совместимый luna-init
luna-init manifest
    ↓ совместимый kernel
Linux kernel
```

## Запуск приложения

`luna-app-runtime` получает запрос запуска и строит `ApplicationInstance`. Внутри этого экземпляра действует:

```text
ApplicationPlan
    ↓
MappingPlan
    ↓
luna-root-mapping
    ↓
luna-security
    ↓
AuthorizedApplicationPlan
    ↓
luna-namespace
    ↓
процесс приложения
```

Для одного процесса выбирается одна libc: `musl` или `glibc`.

## Владение

`luna-init` владеет границей PID 1. `luna-system-runtime` владеет долгоживущим системным supervision. `UserSession` — доменная сущность. `luna-app-runtime` владеет application execution lifecycle. `luna-root-mapping`, `luna-security` и `luna-namespace` сохраняют узкие специализированные обязанности.

Не вводятся `luna-core`, generic `luna-runtime`, `luna-session`, `luna-run-session`, `luna-app-init` или отдельный application supervisor.

Несколько функций одного domain boundary не должны автоматически становиться отдельными процессами. В частности, UserSession реализует seat/input/graphics/authentication/compositor как внутренние модули; отдельные внешние provider daemons являются временной совместимостью, а не частью целевой boot chain.

Компоненты Luna не следует путать с внешними программными поставщиками. Например, `luna-audio`, `luna-bluetooth`, `luna-network` и `luna-files` являются Luna-owned границами, хотя используют внешние PipeWire/WirePlumber, BlueZ, NetworkManager, Yazi и другие проекты.