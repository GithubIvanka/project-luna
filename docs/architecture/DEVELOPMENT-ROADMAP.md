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
RAM-backed logical /
  ↓
luna-system-runtime (PID 1)
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
- `FAILURE-RECOVERY-CONTRACT.md`;
- `LUNA-INIT-CONTRACT.md`.

Контракт `LUNA-INIT-CONTRACT.md` фиксирует RAM-backed root bootstrap, внутренний immutable System Image source и отсутствие `switch_root` как целевой модели.

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

Цель: `luna-init` создаёт RAM-backed logical `/`, подготавливает минимальный runtime и передаёт управление `luna-system-runtime` без превращения SquashFS в постоянный `/`.

Нужно реализовать и проверить:

- поиск SYSTEM/DATA;
- валидацию selected System Image и adjacent manifest;
- подключение System Image как внутреннего read-only source;
- deterministic boot-critical materialization set;
- RAM-backed logical `/`;
- runtime `/dev`, `/proc`, `/sys`, `/run`, `/tmp`;
- controlled DATA exposure;
- отсутствие classic `switch_root` handoff;
- устойчивый handoff в `luna-system-runtime` PID 1.

Отдельно:

- lazy hydration дополнительных immutable resources;
- проверка независимости materialized resources от lifetime source image;
- privileged end-to-end tests.

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
  ↓ validate
luna-security
  ↓ Allow
AuthorizedApplicationPlan
  ↓
ApplicationLaunchContext + RuntimeProfile
  ↓
luna-namespace
  ↓
ApplicationInstance
```

Security обязателен до materialization. Ошибка policy — fail closed.

`luna-app-runtime` не создаёт отдельный `luna-app-init`. ApplicationInstance запускается непосредственно как application process. PID namespace не является обязательной частью текущего execution model; mount namespace и остальные isolation primitives определяются policy/runtime profile.

## Этап 8 — обновление и восстановление

- атомарная подготовка новой версии;
- independent kernel update;
- activation;
- health confirmation;
- rollback;
- soft fallback;
- Factory;
- Recovery;
- retention;
- proof that active runtime resources are independent from an image before image removal.

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
