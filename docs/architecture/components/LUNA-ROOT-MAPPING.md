# `luna-root-mapping`

**Статус:** foundation реализован; integration с logical root/materialization продолжается.

## Назначение

Определяет логическую модель filesystem mapping и строит проверяемый `MappingPlan` для конкретной execution environment.

## Владеет

- logical paths;
- mapping declarations;
- Root Mapping semantics;
- построением и валидацией `MappingPlan`;
- связыванием Bundle/resource declarations с runtime/user/system context.

## Не владеет

Authorization policy, namespace creation, raw filesystem I/O, Bundle container codec или process lifecycle.

## Принцип

Физические пути DATA не являются публичной семантикой Bundle. Приложение получает логический view:

```text
/
├── app
├── lib
├── data
└── tmp
```

а реальное physical mapping остаётся внутренней реализацией.

## Security boundary

`MappingPlan` описывает требуемое отображение, но не выдаёт право на его materialization.

```text
MappingPlan
    ↓
luna-security
    ↓
luna-namespace
```

## Ошибки

Неполный, неоднозначный или внутренне противоречивый mapping должен отклоняться до security decision.

## Зависимости

`luna-common`, `luna-fs` и domain resource descriptions.

## Открыто

Полная logical-root contract, lazy materialization и mapping rules для user files/external volumes.