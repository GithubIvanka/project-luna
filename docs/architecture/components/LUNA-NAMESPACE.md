# `luna-namespace`

## Назначение

Реализует Linux-specific namespace и materialization primitives для уже разрешённого запуска приложения.

## Владеет

- Linux mount namespace;
- создание и настройкой изолированного namespace;
- построением RAM-backed logical `/`;
- подключением разрешённых mapping sources;
- runtime pseudo-filesystems;
- Linux-механизмами применения разрешённых ограничений;
- низкоуровневой подготовкой процесса перед `execve()`.

## Не владеет

`luna-namespace` не принимает решения о разрешениях, не строит application policy, не устанавливает Bundles, не управляет `ApplicationInstance` и не становится application init/supervisor.

## Вход

Компонент получает уже валидированный и авторизованный результат. Нельзя передавать ему произвольный `ApplicationPlan` как будто он уже разрешён.

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
```

## Изоляция

Для каждого `ApplicationInstance` mount namespace обязателен.

PID namespace не создаётся по умолчанию. Приложение остаётся обычным non-1 процессом системного PID namespace.

User, network, IPC, UTS, time и другие namespaces создаются только когда это необходимо по разрешённой policy.

## Logical root

Рабочий `/` приложения — отдельная RAM-backed runtime-среда. Она собирается из `RuntimeProfile` и авторизованных mappings.

Полный System Image не используется как готовый `/` приложения и не раскрывается приложению целиком.

## Linux enforcement

В зависимости от разрешённой policy применяются mount isolation, credentials, capabilities, Landlock, cgroups и другие Linux primitives. `CAP_SYS_ADMIN` и эквивалентный host-level доступ не выдаются приложению по умолчанию.

## Материалиазация

Компонент не добавляет ресурсы, которых нет в `AuthorizedApplicationPlan`. Ошибка materialization является отказом запуска, а не поводом расширить доступ.

## Очистка

После завершения запуска временные namespace/mount resources должны быть освобождены. Ошибка cleanup должна оставаться наблюдаемой отдельно от результата процесса.

## Статус

Базовые Linux namespace и mount primitives существуют. Полная production materialization, credential/capability enforcement и cleanup hardening продолжаются.