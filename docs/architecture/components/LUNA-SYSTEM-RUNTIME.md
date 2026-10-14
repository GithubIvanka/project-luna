# `luna-system-runtime`

## Назначение

Долгоживущий system-wide runtime и supervisor, который запускается `luna-init` как дочерний процесс. Он не является PID 1.

## Связь процессов

```text
PID 1
luna-init
  │
  └── luna-system-runtime
        ├── системные службы
        └── UserSession
              └── luna-app-runtime
```

## Владеет

- общесистемным supervision;
- координацией startup/shutdown после раннего bootstrap;
- коллекцией и lifecycle `UserSession`;
- координацией system managers;
- system events;
- запуском и наблюдением доверенных system processes;
- финальным подтверждением semantic boot success;
- очисткой `LunaBootAttempt` после подтверждения успеха.

## Не владеет

UEFI discovery, kernel handoff, выбором System Image, проверкой init compatibility, Bundle installation или application authorization policy.

## Успешная загрузка

В обычном boot mode runtime напрямую запускает и supervises `luna-device-manager`
и `luna-volume-manager`. Внешний service manager не является обязательным для
этого критического пути. Runtime ждёт readiness обоих компонентов и проверяет:
наличие native kernel uevent monitor, обнаруженных block partitions, включённой
auto-attach policy и хотя бы одного доступного Volume. Если любой этап не
завершился, runtime не подтверждает boot success, а графическая сессия не
начинается.

После успешного gate runtime подтверждает `SUCCESS` и очищает `LunaBootAttempt`.
Только затем, если доступен `/usr/bin/openrc`, запускаются внешние compatibility
runlevels для некритичных desktop/system providers. OpenRC не запускает Luna
Device Manager или Volume Manager и не владеет их readiness.

Recovery mode сохраняет отдельную последовательность с виртуальным DATA provider
и не требует обычного physical-volume startup.

```text
SystemRuntime started
      ↓
runtime + system state ready
      ↓
semantic SUCCESS
      ↓
clear LunaBootAttempt
```

## Жизненный цикл

```text
spawned by luna-init
  ↓
starting
  ↓
running
  ↓
system/session supervision
  ↓
shutdown or reboot
```

## PID 1 boundary

`luna-init` остаётся PID 1 и продолжает reap дочерних процессов независимо от lifecycle `luna-system-runtime`.

## Ошибки

Падение runtime не создаёт нового supervisor layer. `luna-init` остаётся PID 1 и применяет установленную failure/reboot/recovery policy.

## Статус

Process supervision, UserSession integration и durable state integration присутствуют в коде. Полная production startup orchestration, lifecycle reconciliation и resource limits ещё требуют реализации.
## Transitional seat compatibility provider

The one-file Niri acceptance DATA provider includes seatd. When no external
service manager is present, System Runtime may launch and supervise it as a
transitional libseat compatibility provider. This is not Luna-native seat
ownership or a required runtime component. It is not part of the critical
boot-success gate: a missing seat provider must not undo an already-confirmed
boot. The compatibility path can be removed when luna-user-session owns the
compositor's seat resource handoff.

System Runtime does not launch a second udev daemon. Device Manager is the
single owner of the udev compatibility process, coldplug trigger/settle and
process supervision. UserSession remains the Luna boundary for seat/session
lifecycle; this temporary seatd broker serves only the authenticated Wayland
provider.

## Возврат к login после logout

Запуск authenticated desktop с окружением и сменой credentials регистрирует
PID в существующей связи process → UserSession. Завершение compositor переводит
UserSession в Ended; runtime завершает ApplicationInstances, очищает cgroup desktop и `/run/user/<uid>`
завершившего сессию пользователя, затем возвращается к native login UI.
Повторный вход создаёт новый desktop, без перезагрузки PID 1 или manager services.

## Запуск установленных приложений

Локальный application broker принадлежит текущей authenticated UserSession.
UID и точная cgroup peer PID проверяются до обработки запроса. Broker получает
verified Bundle через существующий App Manager и передаёт его App Runtime;
authorization выполняет `luna-security`. System Runtime предоставляет process
host своему supervisor и cgroup v2 descendant cleanup, включая `setsid` children.
При недоступной boundary запуск Bundle закрыт. Native-musl subset теперь включает
Bundle-owned dynamic closure и один explicitly authorized Wayland socket текущего
UserSession. Broker выбирает endpoint/cgroup/UID, App Runtime проверяет sealed
mapping и proof; child подключается после credentials drop. См.
[DYNAMIC-WAYLAND-STAGE.md](../../development/application-bundles/DYNAMIC-WAYLAND-STAGE.md).

Home capability выбирает проверенный user home с no-follow/UID validation.
Process host вызывает setsid перед child setup; private terminal setup требует
session leadership. Ghostty сам создаёт PTY и fish child в том же AppInstance,
не добавляя supervisor. Explicit desktop environment выбирает Wayland/Cairo и
terminal software Mesa; host device/library fallback не предоставляется. См.
[TERMINAL-FILES-STAGE.md](../../development/application-bundles/TERMINAL-FILES-STAGE.md).

Existing application broker также публикует bounded verified desktop catalog
до Noctalia и при authenticated `luna app list`, без новых services. Root-owned
runtime entries — presentation cache; Exec повторно проходит manager verification
и policy, expected content сравнивается до probe/storage. Ошибка refresh и logout
очищают собственные entries; неизвестные objects не удаляются. См.
[DESKTOP-DISCOVERY-STAGE.md](../../development/application-bundles/DESKTOP-DISCOVERY-STAGE.md).
