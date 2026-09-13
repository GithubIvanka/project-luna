# Контракт System State

**Статус:** Accepted  
**Date:** 2026-09-13  
**Scope:** `luna-system-manager`, system target selection and Recovery

## 1. Назначение

`System State` описывает постоянные роли и целевые комбинации системы. Он не является журналом каждой boot attempt и не заменяет boot-state.

## 2. Target model

Обычный системный target всегда атомарен:

```text
SystemTarget
├── image
├── init
└── kernel
```

Элементы target выбираются как совместимая цепочка:

```text
System Image
    ↓ compatible
luna-init
    ↓ compatible
Kernel
```

Смешивание артефактов из разных target без повторной проверки совместимости запрещено.

## 3. Persistent roles

System State содержит три роли:

```text
System State
├── current
│   ├── image
│   ├── init
│   └── kernel
│
├── factory
│   ├── image
│   ├── init
│   └── kernel
│
└── recovery
    ├── image
    ├── init
    ├── kernel
    └── data
```

### Current

`current` — рабочая система, выбранная для обычной загрузки.

Её DATA provider — физический `LUNA-DATA`.

### Factory

`factory` — гарантированный рабочий системный target, предназначенный для восстановления системы и Factory Environment.

Его DATA provider — физический `LUNA-DATA`.

### Recovery

`recovery` — специализированный системный target для восстановления и диагностики.

Recovery дополнительно содержит ссылку на Recovery DATA Image, который становится RAM-backed DATA provider во время Recovery startup.

## 4. Recovery target

Логически Recovery target имеет форму:

```text
RecoveryTarget
├── target
│   ├── image
│   ├── init
│   └── kernel
│
└── data
    └── Recovery DATA Image
```

Recovery DATA Image не заменяет Recovery System Image. Это отдельный DATA payload.

## 5. Recovery DATA

Recovery DATA Image содержит логическую DATA-структуру Luna:

```text
system/
├── apps/
├── drivers/
├── libs/
├── config/
└── ...

users/
└── recovery/
    ├── home/
    ├── data/
    └── config/

data/
cache/
```

Она материализуется в RAM и представляется системе как обычный DATA provider.

Recovery таким образом использует те же системные интерфейсы, что и обычная система, несмотря на виртуальную природу DATA.

## 6. Physical DATA interaction

Recovery DATA не зависит от `luna-data.toml` и не требует физического DATA для собственного запуска.

После запуска Recovery может работать с физическим `LUNA-DATA` как с отдельным объектом:

```text
Recovery Virtual DATA
        │
        ├── diagnostics
        ├── DATA discovery
        ├── DATA selection
        ├── binding repair
        └── recovery utilities
                  │
                  ▼
             Physical DATA
```

## 7. Persistent storage

The system manager persists the target identities in system state storage. The logical keys are:

```text
system/current/image
system/current/init
system/current/kernel

system/factory/image
system/factory/init
system/factory/kernel

system/recovery/image
system/recovery/init
system/recovery/kernel
system/recovery/data
```

`system/recovery/data` identifies the Recovery DATA Image version.

## 8. System State vs Boot State

System State answers:

```text
Which complete target is current?
Which complete target is factory?
Which complete target is recovery?
Which Recovery DATA Image belongs to recovery?
```

Boot State answers:

```text
Which boot attempt is in progress or was interrupted?
Which target was attempted?
Which failure domain was observed?
How deep is the fallback chain?
```

These models remain separate.

## 9. Fallback relationship

The state model supports the following policy:

```text
Current target
    │
    ├── Image / Init failure → soft fallback without reboot
    │
    └── Kernel panic → reboot → previous compatible kernel

Recovery target failure
    ↓
Factory target
```

The fallback operation always resolves a complete target rather than independently swapping image, init or kernel fields.

## 10. Update policy

Updates modify System State atomically after the new target is fully prepared and validated.

A successful update of `current` updates all three current target fields together.

Factory and Recovery remain independent roles and may retain different Image, init and kernel versions from current.

Recovery System Image and Recovery DATA Image may be updated independently of the normal current System Image lifecycle.
