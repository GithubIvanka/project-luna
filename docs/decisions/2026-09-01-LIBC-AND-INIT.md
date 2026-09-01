# Project Luna — libc, Runtime and Init Architecture

**Дата:** 2026-09-01  
**Статус:** принято  
**Архитектурный SoT:** `docs/ARCHITECTURE.md`

Этот документ фиксирует архитектурные решения по системной libc, совместимости с glibc, раннему userspace и постоянному system supervisor. Он не отменяет решения из `docs/ARCHITECTURE.md` и должен рассматриваться как их уточнение.

## 1. Родная libc Luna

Project Luna использует **musl libc** как системную libc userspace.

Это относится к компонентам самой Luna, включая ранний userspace, системные сервисы, management/runtime components и штатные системные утилиты.

Цель этого решения не сводится к максимальной производительности. Выбор musl основан прежде всего на соответствии архитектуре Luna:

- небольшая системная основа;
- простой и предсказуемый runtime;
- минимизация системного footprint;
- удобный fit для immutable System Image;
- отсутствие необходимости делать glibc обязательной частью базовой системы.

Производительность является важным свойством, но не считается архитектурной гарантией превосходства musl над glibc во всех нагрузках.

## 2. glibc как compatibility runtime

glibc **не является второй системной libc Luna**.

glibc предоставляется как отдельный runtime для программ, которые требуют GNU libc ABI/runtime.

Концептуально:

```text
Luna system
    └── musl

Compatibility runtimes
    └── glibc
```

glibc runtime должен быть versioned и управляться как отдельный runtime artifact/компонент. Конкретный layout физических файлов является implementation detail и не должен становиться частью пользовательского logical root.

## 3. Правило одного libc environment на процесс

Обычный процесс Luna работает в одном libc environment:

```text
process
  └── musl environment
```

или:

```text
process
  └── glibc environment
```

Смешивание musl и glibc как взаимозаменяемых `libc.so` внутри одного процесса не является поддерживаемой моделью.

Luna не должна пытаться решить эту проблему заменой файлов `/lib` или `/usr/lib` в глобальной системе.

Разные процессы на одной системе при этом могут одновременно использовать разные libc environments.

## 4. Runtime selection

Runtime environment выбирается на уровне application/runtime policy и материализуется перед запуском процесса.

Концептуальные варианты:

```text
runtime = luna
runtime = glibc
runtime = bundle
```

`runtime = luna` означает штатное Luna/musl окружение.

`runtime = glibc` означает подключение утверждённого glibc compatibility runtime.

`runtime = bundle` разрешает полностью self-contained runtime, когда это предусмотрено Bundle contract.

Точный manifest schema будет формализован отдельно и не считается утверждённым этим документом.

## 5. Mapping boundary

Физические библиотеки и runtime paths не должны быть видимы приложению напрямую.

`luna-root-mapping` определяет logical mapping, а `luna-namespace` материализует его в Linux mount/filesystem namespace после прохождения security authorization и mapping validation.

Следовательно, приложение получает нормальный Linux-compatible logical root, а Luna может представить ему соответствующий runtime:

```text
logical /lib
    ↓
    musl runtime
```

или:

```text
logical /lib
    ↓
    glibc runtime
```

или другой разрешённый private runtime.

Физическая организация `SYSTEM`, `DATA` и runtime storage остаётся внутренней деталью Luna.

## 6. Изоляция glibc runtime

glibc runtime подключается только к тем execution environments, которым он необходим.

Он не должен становиться глобальным fallback для всей системы.

Это обеспечивает:

- отсутствие зависимости системных Luna services от glibc;
- независимое версионирование compatibility runtime;
- возможность удалить/обновить glibc runtime без смены libc системной основы;
- параллельный запуск musl- и glibc-приложений;
- отсутствие глобального загрязнения library namespace.

## 7. luna-init

`luna-init` является специализированным **early-userspace init** и частью boot/early-boot boundary.

Он не является обычным service manager и не должен превращаться в большой постоянно работающий daemon.

Основные обязанности:

```text
kernel
  ↓
luna-init
  ├── discover SYSTEM
  ├── read selected System Image information
  ├── mount/open selected SquashFS
  ├── prepare logical root
  ├── attach DATA
  ├── prepare required kernel virtual filesystems/devices
  └── transfer control to the permanent Luna runtime
```

`luna-boot.efi` не монтирует SquashFS и не реализует logical root construction. Этот boundary принадлежит early userspace.

## 8. Permanent system supervisor

После построения рабочего logical root постоянным владельцем системного process supervision остаётся `luna-system-runtime`.

Он является PID 1 system supervisor в рабочей userspace системе и реализует собственную init/service-supervision модель Luna.

Концептуально она ближе к **runit/OpenRC**, чем к systemd:

```text
luna-system-runtime
    ├── service lifecycle
    ├── dependency ordering
    ├── readiness/state tracking
    ├── restart policy
    ├── process supervision
    ├── signal/lifecycle handling
    ├── resource supervision
    └── UserSession orchestration
```

Luna не обязана копировать интерфейсы, unit model или исторические механизмы OpenRC/runit. Это собственный supervisor, оптимизированный под архитектуру Luna.

## 9. Единственный владелец process supervision

`luna-system-runtime` является единственным системным владельцем process supervision.

`luna-app-runtime` не создаёт отдельный supervisor. Он управляет `ApplicationInstance` и обращается к системному runtime за операциями процесса в соответствии с существующим runtime contract.

Это решение уже зафиксировано в `docs/decisions/2026-09-01-RUNTIME-INTEGRATION.md`.

## 10. Boot-to-runtime flow

Целевая последовательность:

```text
UEFI
  ↓
luna-boot.efi
  ↓
Linux kernel
  ↓
luna-init
  ↓
SYSTEM / selected SquashFS System Image
  ↓
DATA attach
  ↓
logical root
  ↓
luna-system-runtime (PID 1)
  ↓
UserSession
  ↓
luna-app-runtime
  ↓
ApplicationInstance
  ↓
logical runtime environment
       ├── musl
       ├── glibc
       └── bundle-private runtime
```

## 11. Security and materialization

Runtime selection, security authorization and mapping validation must be completed before namespace materialization and process execution.

В частности, запрос приложения на glibc runtime не является сам по себе полномочием получить произвольные системные библиотеки. Runtime должен разрешаться из известных Luna-managed runtime sources.

## 12. Invariants

Следующие положения считаются архитектурными инвариантами:

1. Luna system userspace is musl-based.
2. glibc is an optional compatibility runtime, not the system libc.
3. One process does not freely mix musl and glibc libc environments.
4. Multiple libc environments may coexist across different processes.
5. Runtime libraries are presented through Luna logical mapping/namespace mechanisms.
6. `luna-init` owns early userspace System Image construction and DATA attach.
7. `luna-system-runtime` owns permanent process supervision.
8. `luna-app-runtime` does not duplicate system process supervision.
9. Runtime selection must pass security/policy and mapping validation before materialization.
10. Physical runtime/library layout is an implementation detail, not part of the logical application filesystem contract.

## 13. Deferred implementation details

Следующие вопросы пока не фиксируются этим решением:

- точный физический каталог glibc runtime;
- точный Bundle manifest field для runtime selection;
- формат и versioning runtime manifests;
- точный набор библиотек, разрешённых в базовом glibc runtime;
- стратегия ABI/version compatibility checks;
- способ доставки и обновления runtime artifacts;
- финальная реализация production `luna-init`.

Эти вопросы должны быть решены отдельными implementation/architecture decisions, когда появятся необходимые технические контракты.
