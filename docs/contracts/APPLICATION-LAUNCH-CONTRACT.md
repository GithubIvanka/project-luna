# Контракт запуска приложения

**Статус:** принятый архитектурный контракт
**Область:** `UserSession` → `luna-app-runtime` → `ApplicationInstance`

## 1. Общая модель

`luna-app-runtime` получает запрос на запуск и строит один `ApplicationInstance`. Внутри жизненного цикла этого экземпляра выполняется:

```text
планирование
  ↓
mapping
  ↓
authorization
  ↓
materialization
  ↓
запуск
```

Конкретная последовательность:

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

## 2. ApplicationPlan и MappingPlan

`ApplicationPlan` содержит identity Bundle/приложения, версию, `UserSession`, `RuntimeSpec`, executable, аргументы, требования к ресурсам, mappings и запросы разрешений. План не является разрешением.

`MappingPlan` описывает логические пути и допустимые классы источников. `luna-root-mapping` владеет семантикой mapping, его валидацией и построением детерминированного представления.

Физические пути `LUNA-SYS/...` и `LUNA-DATA/...` не являются частью публичной семантики Bundle.

## 3. RuntimeSpec и выбор libc

`RuntimeSpec` является частью `ApplicationInstance`. Он определяет execution environment приложения.

Для приложения выбирается ровно одна libc/runtime-среда:

```text
musl
или
Glibc
```

`musl` — native runtime Luna. `glibc` — compatibility runtime для приложений, которым требуется glibc. Сам выбор libc не является отдельным daemon или runtime-компонентом.

`RuntimeProfile` отдельно описывает минимальный набор доверенных системных логических ресурсов, доступных приложению. Он не выбирает libc.

## 4. Авторизация

`luna-security` — единственная policy authority. Он проверяет application identity, mappings, capabilities и иные requests и создаёт sealed `AuthorizedApplicationPlan`.

```text
request != grant
```

Authorization должна завершиться до materialization. `Deny`, ошибка политики и неподдерживаемое ограниченное решение приводят к fail closed.

Trust, криптографическая подпись и authorization — разные решения. Доверие Bundle не означает автоматического предоставления прав приложению.

## 5. Materialization и namespace

`luna-namespace` получает только авторизованный результат и реализует Linux-specific materialization:

- отдельный mount namespace для каждого `ApplicationInstance`;
- RAM-backed logical `/`;
- разрешённые mappings;
- `RuntimeProfile`;
- необходимые runtime filesystems;
- требуемые ограничения доступа;
- финальная подготовка процесса.

PID namespace по умолчанию не создаётся. Приложение остаётся обычным процессом в системном PID namespace и получает обычный PID, отличный от `1`.

## 6. ApplicationInstance

`ApplicationInstance` принадлежит `luna-app-runtime` и хранит identity экземпляра, identity/версию приложения, session identity, `RuntimeSpec`, lifecycle state, PID/process identity после создания, результат завершения и диагностическую информацию об ошибке.

Внешний caller не меняет lifecycle напрямую.

Базовые состояния:

```text
Created → Starting → Running → Stopping → Stopped
Starting → Failed
Running → Crashed
Stopping → Failed
```

## 7. Граница компонентов

```text
luna-app-manager   → установка и lifecycle Bundle
luna-app-runtime   → ApplicationInstance и запуск
luna-root-mapping  → mapping semantics / MappingPlan
luna-security      → authorization / trust policy
luna-namespace     → namespace/materialization
luna-system-runtime → system-wide supervision
```

Новый application init/supervisor не создаётся.

## 8. ELF dependency closure

luna-app-runtime содержит этап планирования ELF-зависимостей, который не вызывает host dynamic loader. Анализатор извлекает класс ELF, endian, machine, PT_INTERP, DT_NEEDED, DT_RPATH и DT_RUNPATH.

ElfDependencyClosure рекурсивно обходит interpreter и shared objects через явный ElfDependencyResolver. Циклы схлопываются в множество closure; несоответствие архитектуры приводит к fail closed.

FilesystemElfResolver работает только с явно переданными доверенными источниками и не использует LD_LIBRARY_PATH, host ld.so.cache или host default directories. $ORIGIN нормализуется лексически.

Этот этап пока является подготовкой dependency closure. Подключение closure к MappingPlan, provenance ресурсов и окончательной авторизации luna-security остаётся отдельной следующей стадией.
