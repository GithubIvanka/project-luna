# `luna-cli`

## Назначение

Пользовательская командная строка `luna`.

## Владеет

Стабильной CLI-поверхностью и передачей команд в соответствующие domain APIs.

## Не владеет

Привилегированной архитектурной policy и не дублирует логику системных managers.

GUI и CLI являются тонкими клиентами общих backend contracts. Отключение CLI не должно отменять выполняющуюся backend operation.

## Статус

Реализованы `luna bundle pack/inspect` (inspect читает LBP1 и directory Bundle) и
`luna app --offline-data-root <DATA> list/install/update/verify/remove/recover`.
Dispatch использует существующие Bundle/App Manager/State/Update APIs.
Установка требует явно переданного `--trust-content <BLAKE3-256>`; capabilities
manifest остаются requests и не превращаются в grants запуска.

Management-команды — offline adapter для разработки и Recovery-подобных операций с выбранным
DATA, а не production daemon или протокол действующей desktop session.
Оператор должен владеть DATA; одновременно работающий `luna-system-runtime`,
удерживающий state database, исключает такое использование.

`luna app run ID VERSION [--expect-content BLAKE3] [-- ARGS...]` обращается к локальному broker действующего
System Runtime. Caller должен принадлежать authenticated desktop UserSession;
runtime проверяет UID и точную session cgroup по peer PID. Эта команда не принимает
offline DATA root, физические пути, credentials или session ID. Она ждёт process
outcome и возвращает exit code либо `128 + signal`; запуск/authorization error — `1`.
Native-musl subset поддерживает static и Bundle-owned dynamic ELF с явным
root-owned Launch grant. `desktop.wayland` требует capability и точного socket
path grant текущей сессии. Stdio GUI приложения остаётся `/dev/null`; Ghostty
с `terminal.pty` создаёт собственный PTY для fish внутри AppInstance. Caller CLI
stdio ещё не пересылается; live management IPC
остаются отдельными задачами. Fixed Alpha launchers вызывают эту же команду.
Границы:
[DYNAMIC-WAYLAND-STAGE.md](../../development/application-bundles/DYNAMIC-WAYLAND-STAGE.md).
Текущий GUI/terminal этап:
[TERMINAL-FILES-STAGE.md](../../development/application-bundles/TERMINAL-FILES-STAGE.md).

`luna app list` получает bounded TOML catalog через тот же UID/cgroup-authenticated
broker и обновляет ephemeral root-owned XDG presentation. Catalog содержит
verified registered graphical applications с user/content grants; физические
пути и management authority не выдаются. Noctalia entries запускают pinned
ID/version/content через ordinary RunRequest; manifest `entry.args` задаёт defaults.
Registered Terminal не получает broker socket. См.
[DESKTOP-DISCOVERY-STAGE.md](../../development/application-bundles/DESKTOP-DISCOVERY-STAGE.md).

Команды и доказательства:
[`DIRECTORY-STAGE.md`](../../development/application-bundles/DIRECTORY-STAGE.md),
[`LAUNCH-STAGE.md`](../../development/application-bundles/LAUNCH-STAGE.md).
