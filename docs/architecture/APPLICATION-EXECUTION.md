# Архитектура запуска приложений

## Владение

```text
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
```

`luna-app-runtime` владеет жизненным циклом выполнения приложений и состоянием `ApplicationInstance`. Он не является глобальным supervisor и не становится PID 1 приложения.

## Поток запуска

`luna-app-runtime` получает запрос запуска и строит один `ApplicationInstance`. Внутри экземпляра:

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

Конкретная граница компонентов:

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
      ↓
ApplicationInstance
      ↓
процесс приложения
```

Координация запуска принадлежит `luna-app-runtime`, mapping — `luna-root-mapping`, authorization — `luna-security`, materialization и низкоуровневая настройка Linux namespace — `luna-namespace`.

## ApplicationPlan

План содержит identity приложения и Bundle, версию, `UserSession`, `RuntimeSpec`, executable и аргументы, требования к ресурсам, mapping context и запросы разрешений. План не является grant.

## MappingPlan

`MappingPlan` описывает логические пути и допустимые классы источников. `luna-root-mapping` валидирует их, выбирает семантически допустимые источники и строит детерминированное mapping-представление.

Физические пути `LUNA-SYS/...` и `LUNA-DATA/...` не являются API приложения.

## RuntimeSpec и libc

`RuntimeSpec` принадлежит `ApplicationInstance` и определяет runtime environment. Для одного процесса выбирается одна libc:

```text
musl
или
glibc
```

`musl` используется native Luna userspace. `glibc` предоставляется как compatibility environment для приложений, которым она необходима. Выбор libc не является отдельным компонентом.

`RuntimeProfile` описывает доверенные системные логические ресурсы execution environment. Он не является выбором libc.

## Authorization

`luna-security` — центральная policy authority. Он проверяет requests, mappings, capabilities, trust и ограничения политики и создаёт только авторизованный результат.

```text
request != grant
```

Authorization завершается до materialization. Отказ или невозможность точно представить/применить политику приводит к fail closed.

## Trust Bundle

Trust отвечает на вопрос: можно ли принять конкретный Bundle как доверенный источник для выбранной операции. Это особенно важно для внешних и переносимых Bundles.

Подпись отвечает за криптографическую подлинность содержимого. Trust определяет, доверяет ли Luna этому содержимому/издателю в данном trust scope. Authorization отдельно решает, какие действия и ресурсы разрешены конкретному запуску.

```text
signature validity
        ≠
trust
        ≠
authorization
```

Trust не является отдельным компонентом: решение принимает `luna-security`. Bundle без подписи может быть допустим по policy, а валидная подпись сама по себе не выдаёт права.

## Namespace и materialization

`luna-namespace` материализует уже авторизованный execution environment:

- создаёт отдельный mount namespace;
- формирует RAM-backed logical `/`;
- подключает `RuntimeProfile` и разрешённые mappings;
- создаёт нужные runtime filesystems;
- применяет policy-driven ограничения;
- подготавливает и запускает процесс.

Mount namespace обязателен. PID namespace по умолчанию не создаётся. Приложение остаётся обычным non-1 процессом системного PID namespace.

## ApplicationInstance

`ApplicationInstance` представляет один запуск и хранит instance identity, application identity/version, session identity, `RuntimeSpec`, lifecycle state, process identity/PID после создания, terminal outcome и диагностические сведения об ошибке.

Lifecycle:

```text
Created → Starting → Running → Stopping → Stopped
Starting → Failed
Running → Crashed
Stopping → Failed
```

Terminal states не возвращаются в `Running`. Внешний caller наблюдает состояние, но не изменяет его напрямую.

## Очистка

После завершения процесса или неудачи запуска `luna-app-runtime` обязан освободить временные staging/namespace resources и сохранить отдельно результат процесса и результат cleanup.