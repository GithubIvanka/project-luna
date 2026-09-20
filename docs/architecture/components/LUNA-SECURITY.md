# `luna-security`

## Назначение

Центральная policy authority Luna для решений о доверии, разрешениях и доступе.

## Владеет

- principals и resources;
- permission dimensions: `Visibility`, `Read`, `Write`, `Execute`, `Device Use`, `Manage`;
- authorization requests и decisions;
- revision/snapshot политики;
- trust decisions;
- capability registration и результаты authorization.

## Не владеет

Filesystem mapping, создание namespace, raw I/O, GUI, запуск процессов или разбор `.lbp` контейнера.

## Разделение trust и authorization

Для Bundle существуют три независимых вопроса:

```text
криптографическая подпись действительна?
              ↓
можно ли доверять содержимому/источнику?
              ↓
разрешено ли этому запуску получить нужные ресурсы?
```

То есть:

```text
signature validity
        ≠
trust
        ≠
authorization
```

Trust нужен прежде всего для внешних и переносимых Bundles и для supply-chain policy. Он не создаёт права приложения.

Trust может быть связан как минимум с `BundleId`, `ContentIdentity` и trust scope. Решение о trust остаётся частью `luna-security`; отдельного `trust-daemon` нет.

Валидная Ed25519-подпись подтверждает соответствие подписи содержимому, но сама по себе не означает, что Bundle доверен или имеет какие-либо permissions.

## Authorization

Bundle declarations, mappings, capabilities и другие requests являются только запросами:

```text
request != grant
```

`luna-security` после проверки policy создаёт typed/sealed авторизованный результат. Отказ, ошибка политики или невозможность точно представить ограничение приводят к fail closed.

`Ask` означает, что требуется явное подтверждение. `Constrained` содержит структурированные ограничения.

Per-instance policy может только ужесточить application-level policy и не может ослабить уже установленный deny.

Authorization может быть привязана к конкретной revision/snapshot политики.

## Capability

`CapabilityRegistry` связывает известный capability с provider. Регистрация provider сама по себе не создаёт `CapabilityGrant` и не принимает policy decision.

## Граница запуска

```text
ApplicationPlan
    ↓
MappingPlan
    ↓
luna-security
    ↓
AuthorizedApplicationPlan
    ↓
luna-namespace
```

Security не выполняет namespace/materialization или process creation.

## Статус

Базовые authorization и capability types реализованы. Полная интеграция provider IPC, trust store, durable policy и kernel enforcement ещё продолжается.