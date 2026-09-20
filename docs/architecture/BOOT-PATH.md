# Полный путь загрузки Luna

## 1. UEFI → luna-boot.efi

```text
Питание
  ↓
UEFI
  ↓
EFI/Luna/luna-boot.efi
```

Обычная загрузка не ждёт Boot Menu. Удержание `B` открывает меню как исключительный путь.

## 2. EFI, LUNA-SYS и LUNA-DATA

`EFI` и `LUNA-SYS` обязаны находиться на одном физическом диске. `luna-boot.efi` проверяет эту связь и использует ОС только из `LUNA-SYS` этого диска.

`LUNA-DATA` может находиться на том же диске или на другом. Для обычной загрузки используется `LUNA-SYS/config/luna-data.toml` с GUID диска и GUID раздела. Если привязанный DATA недоступен или неоднозначен, обычная загрузка переходит в Recovery вместо случайного выбора.

В Recovery физический DATA можно обнаружить и выбрать явно. После выбора Recovery может записать GUID-пару обратно в `luna-data.toml`.

## 3. Discovery

`luna-boot.efi` обнаруживает:

```text
LUNA-SYS/images/*.squashfs
LUNA-SYS/images/*.toml
LUNA-SYS/cores/*.init
LUNA-SYS/cores/*.toml
LUNA-SYS/kernels/<kernel-id>/...
LUNA-SYS/recovery/recovery.squashfs
LUNA-SYS/recovery/recovery.toml
```

## 4. Совместимость

Артефакты разрешаются только последовательно:

```text
System Image manifest
    ↓ compatible init
luna-init manifest
    ↓ compatible kernel
Linux kernel
```

Manifest System Image объявляет совместимые версии `luna-init`. Manifest `luna-init` объявляет совместимые kernels.

## 5. Boot target

Обычный target:

```text
System Image + luna-init + kernel
```

Recovery:

```text
System Image + luna-init + kernel + Recovery DATA Image
```

Отдельного Recovery System Image нет.

## 6. Выбор

По умолчанию выбирается допустимый target согласно boot policy; при равных прочих условиях предпочтение может отдаваться наиболее новой валидной версии.

При ручном выборе сначала показывается System Image, затем его совместимые init cores, затем kernels, совместимые с выбранным init. Несовместимые сочетания не показываются.

## 7. Подготовка direct-init

`luna-boot.efi` читает `LUNA-SYS/config/boot-state.toml`, разрешает target, загружает kernel и выбранный `.init` ELF в boot-reserved memory и строит `LunaBootHandoffV1`.

В Handoff передаются identity разделов и артефактов, режим загрузки и контекст предыдущей попытки.

После подготовки всех boot objects и непосредственно перед `ExitBootServices` один раз записывается `LunaBootAttempt = in_progress` в NVRAM.

## 8. Linux kernel

Kernel получает стандартный x86 boot protocol и `LunaBootHandoffV1`, проверяет структуру handoff, диапазон `.init`, digest и ELF constraints, создаёт внутренний memory-backed executable object и использует существующий Linux ELF/binfmt путь.


## 9. luna-init — PID 1

После инициализации kernel первым userspace-процессом становится `luna-init` и получает PID 1.

`luna-init` получает read-only boot context через FD 3, выполняет ранний bootstrap, открывает выбранный System Image как immutable source, разрешает normal или Recovery DATA provider, создаёт RAM-backed logical `/`, материализует boot-critical resources и запускает `luna-system-runtime` как дочерний процесс.

`luna-init` остаётся PID 1.

## 10. luna-system-runtime

```text
PID 1  luna-init
  │
  └── luna-system-runtime
        ├── system services
        └── UserSession
              └── luna-app-runtime
```

`luna-system-runtime` получает обычный PID > 1 и отвечает за long-lived system supervision, system state, system managers, события, UserSessions и подтверждение semantic boot success.

После инициализации runtime/system state он подтверждает `SUCCESS` и очищает `LunaBootAttempt` через `efivarfs`.

## 11. Пользовательская сессия

UserSession является одной runtime boundary для всей интерактивной графической среды:

```text
luna-system-runtime
  ↓
UserSession
  ├── authentication
  ├── credentials
  ├── seat
  ├── input
  ├── DRM/KMS
  ├── compositor / Wayland
  └── session UI
           ↓
      active user session
```

В Alpha внешние greetd/seatd/libinput/wlroots/Niri/Noctalia допускаются только как transitional providers. Они не являются обязательной последовательной архитектурной цепочкой.

Ghostty + fish запускаются уже внутри активной пользовательской среды.

## 12. Запуск приложения

```text
UserSession
  ↓
luna-app-runtime
  ↓
ApplicationInstance
```

Внутри экземпляра:

```text
plan
 ↓
mapping
 ↓
authorization
 ↓
materialization
 ↓
process
```

Подробная модель находится в `APPLICATION-EXECUTION.md` и `contracts/APPLICATION-LAUNCH-CONTRACT.md`.

## 13. Soft fallback

Если kernel остаётся работоспособным, failure System Image или раннего userspace может перейти на другой совместимый System Image без reboot. Для нового кандидата повторно разрешаются `image → init → kernel`, причём kernel должен оставаться уже загруженным.

Пользователь может увидеть:

```text
3 failed, running 2
```

## 14. Kernel panic

Kernel panic требует reboot.

```text
kernel panic
  ↓
reboot
  ↓
UEFI
  ↓
luna-boot.efi
  ↓
LunaBootAttempt detected
  ↓
previous compatible target
```

## 15. Recovery

Recovery использует обычный выбранный System Image и совместимые `luna-init` и kernel. Recovery DATA Image материализуется в RAM как `VirtualData`. Его GUI provider — прямой запуск `/usr/bin/niri --session`, такой же как в Normal/Factory. В Recovery виртуальный пользователь `recovery` получает активный UserSession напрямую; интерактивный `greetd`/Noctalia login provider не требуется.

Физическая `LUNA-DATA` при этом не является backing store работающего Recovery; она становится объектом диагностики и ремонта.

## 16. External boot

USB/external boot остаётся UEFI-only chainload path и не является частью обычного Luna userspace runtime.

## 17. ExitBootServices

После `ExitBootServices` `luna-boot.efi` больше не использует UEFI Boot Services. Все данные, необходимые kernel startup, handoff и direct-init, должны быть готовы до этого момента.