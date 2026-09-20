# `luna-bundle`

## Назначение

Доменная модель Bundle и реализация кодека LBP1.

## Владеет

- metadata, resources и capabilities Bundle;
- проверкой manifest;
- чтением и записью RFC-0002 `.lbp`;
- вычислением `ContentIdentity`;
- проверкой поддерживаемых подписей;
- извлечением payload для `luna-app-manager`.

## Не владеет

Компонент не выбирает место установки, не публикует физические пути DATA, не запускает процессы и не участвует в выборе boot target.

## Trust и подпись

Подпись проверяет криптографическую подлинность содержимого. Она не является разрешением на установку или запуск. Trust и authorization определяются `luna-security`.

```text
.lbр
 ↓
проверка формата и целостности
 ↓
Bundle
 ↓
trust / authorization
```

## Граница

```text
байты .lbp
   ↕
кодек LBP1
   ↕
BundleManifest / resources
```

Manifest использует логические resource paths. Физическая установка выполняется за пределами этого crate.

## Статус

Кодек и round-trip/integration tests существуют. Полное соответствие RFC и интеграция trust policy ещё дорабатываются.