# `luna-app-runtime`

## Назначение

Владеет выполнением и жизненным циклом запущенных приложений и состоянием `ApplicationInstance`.

`luna-app-runtime` находится между `UserSession` и конкретным `ApplicationInstance`. Внутри одного экземпляра он координирует планирование, mapping, authorization, materialization и запуск процесса.

## Архитектура

```text
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
    │
    ├── ApplicationPlan
    ├── MappingPlan
    ├── RuntimeSpec
    │     └── libc: musl | glibc
    ├── luna-root-mapping
    ├── luna-security
    └── luna-namespace
```

Это структурная иерархия внутри lifecycle экземпляра, а не список отдельных daemon-процессов.

## Владеет

- identity и state `ApplicationInstance`;
- `ApplicationPlan` и executable identity конкретного запуска;
- `RuntimeSpec`;
- lifecycle процесса приложения;
- связь экземпляра с `UserSession`;
- координацию остальных application-execution слоёв.

## Поток запуска

```text
Application launch request
        ↓
luna-app-runtime
        ↓
ApplicationInstance
        │
        ├── plan: ApplicationPlan
        ├── mapping: MappingPlan + luna-root-mapping
        ├── authorization: luna-security
        └── materialization: luna-namespace
```

Внутренняя последовательность:

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

`luna-app-runtime` не принимает security decisions и не создаёт физические mappings самостоятельно.

## RuntimeSpec и libc

`RuntimeSpec` описывает execution environment. Для одного процесса выбирается ровно одна libc:

```text
musl
или
glibc
```

`musl` — native Luna userspace. `glibc` — compatibility environment для приложений, которым требуется glibc.

`RuntimeProfile` — отдельное описание доверенных системных логических ресурсов. Он не определяет libc.

## ApplicationInstance

Экземпляр хранит:

- instance identity;
- application identity и версию;
- session identity;
- `RuntimeSpec`;
- lifecycle state;
- process identity/PID после создания;
- итог завершения (`exit code`, signal или abnormal outcome);
- типизированную стадию и сообщение ошибки запуска/управления.

Внешний caller может наблюдать instance, но не изменяет его lifecycle напрямую.

## Жизненный цикл

```text
Created → Starting → Running → Stopping → Stopped
Starting → Failed
Running → Crashed
Stopping → Failed
```

`Stopped`, `Crashed` и `Failed` — конечные состояния.

## Session boundary

Перед созданием staging и процесса runtime повторно проверяет, что переданный `UserSession` активен и соответствует session identity авторизованного плана.

## Очистка

После завершения процесса runtime освобождает временные resources namespace/staging. Ошибка очистки наблюдаема отдельно и не заменяет результат работы процесса.

## Не владеет

Установкой Bundle, system-wide supervision, глобальной security policy, физическим storage discovery или отдельным application init/supervisor.

## Статус

Домен `ApplicationInstance`, планирование, authorization pipeline и базовый Linux runtime backend реализованы; полная production integration и kernel/provider enforcement продолжаются.