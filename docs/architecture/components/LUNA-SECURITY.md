# `luna-security`

**Статус:** policy foundation реализована; enforcement integration неполная.

## Назначение

Центральная authority Luna для authorization, permissions и trust policy.

## Владеет

- principals и resources;
- permission dimensions: Visibility, Read, Write, Execute, Device Use, Manage;
- authorization requests и decisions;
- policy revisions/snapshots;
- trust decisions, отдельно от cryptographic signature validity.

## Не владеет

Filesystem mapping, namespace creation, raw I/O, GUI presentation, application execution или Bundle parsing.

## Основной контракт

Bundle mappings, capabilities и access declarations — только requests, никогда не grants.

```text
request ≠ grant
```

`Ask` требует явного подтверждения. `Constrained` должен содержать структурированные ограничения. Per-instance policy может ужесточать application policy, но не может ослабить уже enforced deny.

Trust связывает content identity с trust scope. Signature validity, trust и authorization являются отдельными решениями.

## Обязательная цепочка

```text
ApplicationPlan
 ↓
MappingPlan
 ↓
luna-security
 ↓
luna-namespace / runtime enforcement
```

Security decision должен завершиться до materialization namespace. Ошибка policy — fail closed.

## Зависимости

Shared identities и mapping/resource descriptions. Не зависит от GUI/CLI и не является process supervisor.

## Открыто

Durable policy storage, user confirmation IPC/UI, trust store и полное kernel enforcement.