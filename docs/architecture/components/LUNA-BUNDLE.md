# `luna-bundle`

## Назначение

Доменная модель Bundle и реализация кодека LBP1.

## Владеет

- metadata, resources и capabilities Bundle;
- проверкой manifest;
- чтением и записью RFC-0002 `.lbp`;
- вычислением `ContentIdentity`;
- границей кодека/проверки подписей (полная Ed25519 verification ещё не реализована);
- извлечением payload для `luna-app-manager`.
- проверкой archive-independent directory application Bundle v1.

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

Кодек и round-trip/integration tests существуют. Извлечение требует пустую
реальную директорию; implicit directories получают безопасные нормализованные
права независимо от umask. `verify_extracted_payload` проверяет все bytes,
files, directories, права и владельца; лишние entries, symlinks и hard links
отклоняются. Decompression ограничена объявленным размером секции до выделения
неограниченного объёма памяти. Формы `[[mapping]]` / `[[dependency]]` из RFC
читаются как aliases существующих `mappings` / `dependencies`; канонический
writer сохраняет прежние имена и ContentIdentity существующего кодека.

SIGNATURE можно прочитать с проверкой section hash, но это **не** проверка
Ed25519. App Manager пока отклоняет установку с SIGNATURE. Полное соответствие
RFC и signature backend продолжаются.

`directory::DirectoryBundle` распознаёт содержимое `.lunaapp`, проверяет
canonical TOML, полный inventory, owner/modes и ContentIdentity без `.lbp`.
Pinned `openat`/`O_PATH` reader отклоняет links/special files, ограничивает
фактическое чтение и стримит общий canonical TAR encoding в BLAKE3. Format
inspection не означает регистрацию или authorization. Первый materializer
принимает application с canonical TAR writer, без RESOURCES/SIGNATURE.
Контракт: [`DIRECTORY-BUNDLE-CONTRACT.md`](../../contracts/DIRECTORY-BUNDLE-CONTRACT.md).
