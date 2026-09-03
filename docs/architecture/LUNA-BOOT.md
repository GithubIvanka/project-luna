# Project Luna — `luna-boot.efi`

**Статус:** принятая архитектурная граница; реализация и hardening продолжаются.  
**Граница:** UEFI boot component, вне обычного userspace workspace.

## 1. Назначение

`luna-boot.efi` отвечает только за то, что относится непосредственно к UEFI boot flow:

- обнаружение SYSTEM;
- обнаружение System Images и kernels;
- чтение image manifests и boot metadata;
- выбор совместимой пары image/kernel;
- открытие Boot Menu только по запросу пользователя;
- загрузку выбранного Linux kernel;
- подготовку boot parameters;
- обработку boot-time fallback;
- передачу управления kernel.

Он не владеет UserSession, application lifecycle, Bundle installation, desktop и обычными update transactions.

## 2. Цепочка загрузки

```text
UEFI
 ↓
luna-boot.efi
 ↓
compatible Linux kernel
 ↓
luna-init
 ↓
logical root
 ↓
luna-system-runtime
 ↓
UserSession
```

`luna-boot.efi` не обязан монтировать SquashFS: это ответственность раннего userspace.

## 3. Нормальная загрузка

Обычная загрузка выполняется сразу, без двухсекундной задержки из-за Boot Menu.

```text
B/b уже ожидает ввод?
├── нет → выбрать рабочую совместимую пару → boot
└── да → Boot Menu
```

Выбор usable pair не равен простому выбору самого нового файла.

## 4. Boot Menu

Принятая модель:

1. **Продолжить загрузку Luna** — загрузить выбранную/default compatible pair.
2. **Подробная загрузка** — отключить обычный splash и включить диагностику.
3. **Выбор System Image** — показать доступные образы и только совместимые kernels.
4. **Recovery Environment** — отдельный recovery mode.
5. **Factory Environment** — factory System Image + factory kernel.
6. **Загрузка с USB / внешнего устройства** — передать boot selection внешнему носителю.

Boot Menu является исключительным путём.

## 5. Выбор image/kernel

`luna-boot` читает обнаруженные manifests и использует их для определения:

- версии image;
- архитектуры;
- совместимых kernels;
- boot metadata.

Kernel без подтверждённой совместимости показывать или запускать нельзя.

## 6. Fallback

Fallback зависит от класса отказа.

```text
selected image/kernel
      ↓
image failure
      ↓
previous compatible image where safe/possible
      ↓
other usable fallback
      ↓
Factory
      ↓
Recovery
```

Для kernel-level failure, включая panic, может потребоваться reboot; после него выбирается следующая совместимая рабочая комбинация согласно Boot State.

## 7. Boot State

Boot state отделён от общего System State и Recovery State.

Обычная загрузка не должна переписывать его без необходимости. Изменения выполняются только по значимым событиям: новая активация, подтверждение, failure, rollback или переход в специальный режим.

Точный формат описан в `docs/contracts/BOOT-STATE-CONTRACT.md` как черновик Phase 0.

## 8. Пост-`ExitBootServices`

После `ExitBootServices` загрузчик не использует Boot Services, их allocator, console APIs или UEFI filesystem protocols.

Всё необходимое для handoff должно быть подготовлено до выхода.

## 9. Граница ответственности

После передачи управления kernel ответственность `luna-boot` заканчивается. Logical root, `luna-init`, system runtime и userspace выполняют последующую инициализацию.

`luna-boot` не превращается в второй init system и не становится общим recovery manager.

## 10. Проверка

Загрузчик должен отклонять malformed metadata, несовместимые пары и небезопасные boot structures. Trust policy, требующая сложной userspace логики, не должна самовольно переноситься в UEFI component.

## 11. Текущий статус реализации

В проекте уже присутствует отдельный UEFI implementation с Linux boot protocol handling, включая загрузку kernel через тестовый init. Следующий приоритет — привести discovery, compatibility, fallback и конечный handoff к единым Phase 0 contracts.
