# `luna-system-manager`

## Назначение

Владеет смыслом persistent system targets и связями между ними.

Полные targets имеют identity:

```text
current  = System Image + luna-init + kernel
factory  = System Image + luna-init + kernel
fallback = System Image + luna-init + kernel
recovery = System Image + luna-init + kernel + Recovery DATA Image
```

## Владеет

- моделями `current`, `factory`, `fallback`, `recovery`;
- domain meaning System State;
- загрузкой и изменением durable state через `luna-state`;
- типизированными query/mutation для runtime и update orchestration;
- сохранением атомарной identity полного target.

## Не владеет

UEFI boot selection, непосредственным kernel loading или произвольной записью kernel artifacts.

## Совместимость

Фактическая цепочка разрешается так:

```text
System Image manifest
  ↓ compatible luna-init
luna-init manifest
  ↓ compatible kernel
kernel
```

`luna-system-manager` хранит смысл target; `luna-boot.efi` выполняет фактическое boot-time resolution.

## Взаимодействие

`luna-system-runtime` читает состояние для работы системы. `luna-update-manager` владеет выполнением update transactions и использует этот компонент для semantics system targets.

## Статус

Durable `redb`-backed state model существует. Полная валидация mutation/install backends ещё разрабатывается.