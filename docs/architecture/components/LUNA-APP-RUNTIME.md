# `luna-app-runtime`

**Статус:** граница `ApplicationInstance` реализована; интеграция продолжается.

## Назначение

Владеет выполнением и жизненным циклом запущенных приложений.

## Владеет

- identity и state `ApplicationInstance`;
- lifecycle процессов приложения;
- подготовкой execution environment;
- связью экземпляра с `UserSession`;
- выбором runtime по `RuntimeSpec`.

`RuntimeKind` является свойством `RuntimeSpec`, а не самостоятельным компонентом. Принятые semantics включают Luna, Glibc и Bundle runtime.

## Поток запуска

```text
ApplicationPlan
  ↓
MappingPlan
  ↓
luna-security
  ↓
luna-namespace
  ↓
exec
  ↓
ApplicationInstance
```

Запуск требует активной `UserSession` и использует только проверенный security/mapping context.

## Не владеет

Bundle install/remove, созданием UserSession, system-wide supervision, authorization policy, raw filesystem primitives или UEFI boot.

## Ошибки

Ошибка одного ApplicationInstance не должна автоматически завершать другие UserSessions. Cleanup namespace, mounts, processes и ресурсов является обязательной частью lifecycle.

## Зависимости

`luna-common`, `luna-user-session`, root-mapping, security, namespace и system-runtime contracts.

## Открыто

Production integration с namespace/security, resource limits, restart policy и полноценный lifecycle IPC.