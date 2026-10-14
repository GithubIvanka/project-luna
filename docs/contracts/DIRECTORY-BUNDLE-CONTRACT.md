# Directory Application Bundle v1

Дата: 2026-10-11. Контракт первого реализованного installed representation.
Основание: уточнения пользователя об обычном имени, собственном расширении,
TOML metadata и отсутствии транспортного `.lbp` внутри приложения. Рабочее
расширение реализации — `.lunaapp`; оно не является magic формата.

Не меняет RFC-0002 ContentIdentity, Application Launch Contract, единственный
PID 1 или формат System Image. Границы компонентов остаются существующими.

## 1. Состав

```text
luna-cli.lunaapp/
├── manifest.toml
├── integrity.toml
└── payload/
```

Ровно эти три entries. Нет вложенного `.lbp`, receipt, пользовательского config
или mutable state. `bundle.id`, `bundle.version`, `bundle.name` находятся в
manifest. Обычное имя не содержит добавленного ID/Version. Генератор нормализует
friendly name в lowercase ASCII basename с дефисами, максимум 80 characters;
для имени без ASCII letters/digits использует `application.lunaapp`.

Suffix — подсказка discovery. Явный inspection допускает переименованную
директорию с другим basename. Приложение распознаётся проверкой содержимого;
называть directory Bundle установленным только по suffix запрещено.

## 2. Metadata

`manifest.toml` — точная canonical serialization `LbpManifest::to_toml`.
Поддерживаемые singular aliases RFC читаются в transport; на установке manifest
нормализуется существующим writer без изменения его ContentIdentity.

```toml
# integrity.toml
format = "luna-directory-bundle"
layout_version = 1
payload_encoding = "luna-canonical-tar-v1"
content_identity = "<64 lowercase hex characters>"
```

Неизвестные format/version/encoding и неизвестные поля integrity отклоняются.
Descriptor содержит проверяемый digest, не подпись, trust grant или receipt.
Все mutable installation metadata принадлежат App Manager и находятся снаружи.

## 3. ContentIdentity

```text
BLAKE3(canonical manifest bytes || canonical payload TAR bytes)
```

Для поддерживаемого subset это ровно RFC-0002 ContentIdentity. Directory reader
переиспользует canonical TAR header/ordering/padding writer и стримит результат
в BLAKE3, без хранения или восстановления транспортного контейнера. Он не
вводит другой вид identity. Файлы и их implicit parent directories должны
точно соответствовать entry/mappings; лишние файлы и пустые каталоги запрещены.

На install manager сравнивает reconstructed identity с проверенным transport
identity и exact-content Install trust. Позже сверяет её с внешним receipt.
Внешний reader сравнивает с integrity descriptor, но сам не решает trust.
Изменение descriptor вместе с payload создаёт другое содержимое, а не право
использовать старый trust pin или разрешения запуска.

## 4. Чтение и filesystem safety

- Bundle root и все payload directories — `0555`; metadata — `0444`; обычные
  payload files — `0444`, executable files — `0555`.
- Все entries принадлежат одному владельцу. Symlinks, hard links и special
  entries запрещены, включая symlinks в компонентах переданного пути.
- Directory reader закрепляет directories через `openat` с `O_NOFOLLOW`,
  перечисляет их через kernel-owned `/proc/self/fd`. Leaf сначала закрепляется
  через `O_PATH|O_NOFOLLOW`; только regular file/directory открывается для чтения
  через fd link с `O_NONBLOCK`. Native runtime должен предоставлять `/proc`.
- Размер, тип, device/inode, mode, owner и link count проверяются на открытых
  дескрипторах. Фактическое чтение ограничено; короткий stream отклоняется.
- Metadata максимум 1 MiB каждый; payload content суммарно максимум 1 GiB;
  inventory максимум 100000 entries; глубина payload максимум 128.

Это проверка snapshot содержимого, не механизм наблюдения за будущими writes.
Read-only mode не защищает от доверенного владельца/root, который может вернуть
write permission. Launch должен сохранить принятую integrity/trust/authorization
границу при materialization; inspection не выполняет payload.

## 5. Регистрация и транзакции

```text
DATA/system/apps/.bundles-v2/
├── <opaque slot>/
│   ├── receipt.toml
│   └── luna-cli.lunaapp/...
├── .transactions/<UpdateId>/operation.toml
├── .garbage/<UpdateId>/...
└── .lock
```

Slot — BLAKE3 с отдельным domain prefix от ID и Version; private implementation
handle, не имя приложения. Receipt format 2 содержит ID, Version, exact
ContentIdentity, UpdateId и safe `directory_name`. Registered inventory требует
ровно receipt и named directory Bundle. Registry enumeration сверяет slot,
receipt, manifest и вычисленную identity, а не доверяет названию каталога.

Manager владеет exclusive lock, staging, publication и reconciliation; журнал
принадлежит существующему UpdateEngine/luna-state. Все metadata/payload
синхронизируются перед commit. Install/remove перемещают весь registration
container атомарно; recovery работает только со своим checkpoint. Независимые
версии сохраняются. Удаление не посещает пользовательские data/config.

## 6. Границы первого варианта

Первый writer/installer поддерживает unsigned application Bundle без отдельной
RESOURCES секции и без unresolved dependencies. Transport с другим валидным
TAR encoding/metadata/ordering/padding явно отклоняется, если он не может быть
восстановлен в directory v1 с той же ContentIdentity. Подпись не считается
проверенной по section hash.

Legacy `.bundles` остаётся на месте. При legacy installations или pending
checkpoints новый manager прекращает операции до отдельной journalled migration.
Автоматическая миграция не реализована. Component Bundles не получают этот
application suffix автоматически.

Offline registry/inspection реализованы. Active UserSession launch имеет узкий
native-musl путь: verified registered Bundle → отдельный exact-content Launch
grant → авторизованные mappings → ApplicationInstance. Локальный IPC предоставляет
запуск и bounded authenticated desktop catalog, не live management. Прежний static subset описан в
[LAUNCH-STAGE.md](../development/application-bundles/LAUNCH-STAGE.md); explicit
Bundle-owned dynamic closure и один authorized Wayland endpoint — в
[DYNAMIC-WAYLAND-STAGE.md](../development/application-bundles/DYNAMIC-WAYLAND-STAGE.md).
Начальный Files/Terminal GUI subset описан в
[TERMINAL-FILES-STAGE.md](../development/application-bundles/TERMINAL-FILES-STAGE.md).
Catalog проверяет manager registration/content/user policy, затем публикует
ephemeral `.desktop` presentation; suffix сам по себе не становится receipt.
См. [Application Launch Contract](APPLICATION-LAUNCH-CONTRACT.md).
Установка из portable directory, external CLI stdio relay, custom icons/Show
Contents, live management, другие desktop resources и дальнейший third-party
ecosystem остаются последующими этапами.
