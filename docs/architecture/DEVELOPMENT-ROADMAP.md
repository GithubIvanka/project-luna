# Project Luna — план разработки операционной системы

**Статус:** рабочий план на ветке `develop`.
**Архитектурный источник истины:** `docs/ARCHITECTURE.md`.

Этот документ переводит текущую архитектуру в порядок инженерной реализации. Он не создаёт новые архитектурные границы сам по себе. Если выполнение пункта требует изменения принятого решения, сначала оформляется отдельное архитектурное решение.

## Цель

Полноценная Luna — это не просто собираемый System Image. Критерий готовности проходит через весь пользовательский жизненный цикл:

```text
установка
  ↓
UEFI
  ↓
luna-boot.efi
  ↓
совместимый kernel + System Image
  ↓
luna-init
  ↓
logical /
  ↓
luna-system-runtime
  ↓
graphical login
  ↓
UserSession
  ↓
Wayland → niri → Noctalia
  ↓
Bundle → ApplicationPlan → MappingPlan
  ↓
Security → Namespace → ApplicationInstance
  ↓
файлы / сеть / звук / Bluetooth / внешние носители
  ↓
обновление / rollback / recovery
  ↓
shutdown / reboot / resume
```

## Этап 0 — архитектурные контракты

До крупной реализации закрепляются отдельными контрактами:

- `SYSTEM-IMAGE-CONTRACT.md`;
- `KERNEL-CONTRACT.md`;
- `BOOT-STATE-CONTRACT.md`;
- `BOOT-HANDOFF-CONTRACT.md`;
- `FAILURE-RECOVERY-CONTRACT.md`.

Все они пока являются черновиками.

## Этап 1 — загрузка

Цель: получить надёжный путь `UEFI → luna-boot → Linux → luna-init`.

Критерии:

- обнаружение SYSTEM;
- чтение manifest;
- фильтрация совместимых kernels;
- выбор `current`;
- handoff по Linux boot protocol;
- корректный `ExitBootServices`;
- диагностика ошибок без обращения к Boot Services после выхода.

## Этап 2 — ранний userspace и logical root

Цель: `luna-init` подготавливает SYSTEM/DATA и передаёт управление `luna-system-runtime` после `switch_root`.

Нужно проверить:

- поиск SYSTEM/DATA;
- доступ к SquashFS;
- построение logical `/`;
- подключение DATA;
- минимальный `/dev`, `/proc`, `/sys`;
- устойчивый handoff.

## Этап 3 — system runtime и состояние

Реализуются/укрепляются:

- `luna-system-runtime`;
- `luna-state` с durable `redb` backend;
- `luna-event`;
- `luna-config`.

Цель — получить управляемое живое userspace без GUI.

## Этап 4 — устройства и хранилище

- обнаружение устройств;
- volumes;
- automount внешних носителей;
- безопасное размонтирование;
- интеграция с DATA;
- базовая политика доступа к устройствам.

## Этап 5 — пользователь и графическая сессия

- `luna-login`;
- аутентификация;
- `UserSession`;
- Wayland;
- niri;
- Noctalia;
- базовые power/session controls.

## Этап 6 — Bundle и управление приложениями

- RFC-0002/LBP1 остаётся источником формата;
- `luna-bundle` отвечает за формат и валидацию;
- `luna-app-manager` отвечает за install/import/update/removal;
- `luna-app-runtime` отвечает только за выполнение и lifecycle.

## Этап 7 — изоляция приложений

Полный execution pipeline:

```text
Bundle declaration
  ↓
ApplicationPlan
  ↓
MappingPlan
  ↓
luna-security
  ↓
luna-namespace
  ↓
luna-app-runtime
  ↓
ApplicationInstance
```

Security обязателен до materialization. Ошибка policy — fail closed.

## Этап 8 — обновление и восстановление

- атомарная подготовка новой версии;
- independent kernel update;
- activation;
- health confirmation;
- rollback;
- soft fallback;
- Factory;
- Recovery;
- retention.

## Этап 9 — production hardware

- GPU/input/storage/network/audio coverage;
- firmware policy;
- suspend/resume;
- power management;
- hotplug;
- безопасное выключение и перезагрузка.

## Этап 10 — установка и выпуск

- installer;
- installation media;
- provisioning EFI/SYSTEM/DATA/SWAP;
- первоначальный пользователь;
- factory state;
- signing/trust;
- воспроизводимый build;
- CI и интеграционные тесты.

## Правило создания crate

Архитектурная возможность не является причиной создавать пустой crate. Новый crate появляется только тогда, когда начинается реальная разработка соответствующей границы.

## Definition of Done

Каждый этап считается завершённым только при наличии исполняемого тестового доказательства. Документ, заглушка или наличие конфигурационного файла не считаются доказательством интеграции.