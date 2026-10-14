# Directory Bundle: решение после обсуждения

Актуализация: 2026-10-11. Нижний черновик отвергнут пользователем и оставлен
как история обсуждения; он не описывает текущий формат.

Пользователь уточнил: обычное имя `cli`/`luna-cli`, ID и Version в TOML,
собственное расширение без macOS `.app`, отсутствие транспортного `.lbp`
внутри установленного приложения. После поправок попросил продолжить разработку.

Для первого варианта предложено и реализовано `.lunaapp`. Это рабочий выбор
реализации; пользователь не выбирал это написание отдельным ответом. `.pak`
используется для архивов Unreal Engine
([официальная документация](https://dev.epicgames.com/documentation/unreal-engine/packaging-your-project));
отсутствие конфликтов `.ban` не установлено. Глобальная уникальность `.lunaapp`
не утверждается; распознаётся содержимое, а не только расширение.

```text
luna-cli.lunaapp/
├── manifest.toml
├── integrity.toml
└── payload/
    ├── bin/luna
    └── share/about.txt
```

`about.txt` — обычный текстовый ресурс примера, не manifest и не config.
Receipt и UpdateId находятся вне приложения в приватном registration container.
Обычная папка с suffix не проходит проверку формата; скопированный валидный
Bundle не становится зарегистрированным и не получает разрешений запуска.

Текущий контракт:
[`DIRECTORY-BUNDLE-CONTRACT.md`](../../contracts/DIRECTORY-BUNDLE-CONTRACT.md).
Реализация и evidence: [`DIRECTORY-STAGE.md`](DIRECTORY-STAGE.md).
Legacy migration, live UserSession launch, Files discovery/иконки/Show Contents,
поддержка подписей, dependencies и provider/third-party packages остаются открытыми.

## Исторический отвергнутый черновик

Дата: 2026-10-10.
Статус: **черновик для обсуждения; архитектурное решение ещё не принято**.
Код и принятые контракты этим документом не изменяются.

## Основание

Принятая модель — immutable Bundles, подобные приложениям macOS; независимые
версии, общий программный payload и пользовательские данные вне Bundle.
RFC-0002 определяет транспортный `.lbp`, но не публичную структуру установленного
пакета. Текущий backend хранит `bundle.lbp`, `manifest.toml`, `receipt.toml` и
`payload/` в приватном `system/apps/.bundles/<id>/<version>`.

Ссылки:

- [Принятые решения](../../architecture/ACCEPTED-DECISIONS.md), разделы 12, 19.
- [RFC-0002](../../rfc/RFC-0002.md).
- [Этап хранения](STORAGE-STAGE.md).
- [Контракт запуска](../../contracts/APPLICATION-LAUNCH-CONTRACT.md).

## Предложение

Ввести проверяемую directory representation Bundle. Для application использовать
суффикс `.app`, например `org.luna.cli-0.1.0.app`. Имя и физическое расположение
не входят в identity. Для display использовать name из manifest.

```text
org.luna.cli-0.1.0.app/
├── manifest.toml
├── integrity.toml
├── bundle.lbp
└── payload/
    ├── bin/luna
    └── share/about.txt
```

Для первого варианта сохраняется исходный `.lbp`, как в существующем backend.
Это позволяет переиспользовать проверку ContentIdentity и payload без нового
алгоритма канонизации. Размер установленного пакета включает архив и извлечённое
дерево. Удаление или дедупликация исходного архива требует отдельного решения
о независимой проверке и переносимости directory representation.

`manifest.toml` сохраняет текущую TOML schema приложения: BundleId, версия,
тип, архитектура, entry, dependencies, mappings, capabilities и metadata.
`about.txt` является обычным информационным ресурсом; metadata всегда TOML.
Нужные для UI icon/category сведения следует явно определить в расширении
manifest schema; их semantics и участие в ContentIdentity должны быть проверены
до использования. До этого UI показывает стандартную иконку приложения.

`integrity.toml` — небольшой descriptor directory representation. Пример
предлагаемых обязательных полей:

```toml
format = "luna-directory-bundle"
layout_version = 1
content_identity = "<BLAKE3-256 canonical Bundle content>"
```

Descriptor отличает версию структуры каталога от `format = 1` транспортного
manifest. ContentIdentity остаётся тем же значением RFC-0002; descriptor хранит
ожидаемое значение, а читатель вычисляет и проверяет его по Bundle content.
Descriptor сам по себе не подтверждает trust или факт установки.

Для layout v1 корневой inventory фиксирован. `manifest.toml` должен совпадать
с каноническим manifest проверенного `bundle.lbp`; payload должен полностью
совпадать с его TAR inventory, bytes и разрешёнными modes. Дополнительные entries,
symlinks, hardlinks, special files и неизвестный layout version отклоняются.
Managed installed directory и payload остаются read-only. Policy источника
portable Bundle и защита от замены файлов во время запуска требуют отдельной
проверки; suffix и успешный ранний inspect не заменяют эту границу.

Component Bundles сохраняют `type = "component"` и не получают действие
«Запустить приложение». Расширение для них здесь не назначается. Системные
executable providers используют application representation; их supervision
остаётся у `luna-system-runtime`. Этот proposal не меняет владельцев запуска.

## Распознавание и установка

Разделять два независимых измерения:

| Измерение | Значения |
|-----------|----------|
| Формат объекта | обычный каталог, Bundle candidate, структурно корректный Bundle, повреждённый/неподдерживаемый Bundle |
| Учёт и policy | portable/unregistered, installed, trust scope, разрешения выбранной операции |

`.app` — подсказка для дешёвого discovery. Bounded metadata reader проверяет
descriptor и manifest, не исполняет код и не выполняет автоматическую установку.
Полная проверка содержимого выполняется перед принятием пакета и запуском.
Явный inspect может распознать корректный пакет независимо от имени каталога.

Установленный статус определяется менеджером, а не наличием receipt внутри
скопированной папки. Запись в существующем `luna-state` хранит ID, version,
ContentIdentity и binding к управляемому storage. Mutable installation receipts,
transaction IDs и выбор версии принадлежат менеджеру/пользовательской политике
вне immutable Bundle. Новой базы состояния или daemon не вводится.

Публикация файлов и регистрация выполняются одной журналируемой операцией
через существующий UpdateEngine. При прерывании обычный доступ закрыт до
reconciliation. Content checks и policy выполняются также для зарегистрированной
версии; запись каталога приложений не делает изменённое дерево доверенным.

Valid Bundle, скопированный на внешний диск, остаётся portable/unregistered;
запуск требует проверки целостности, trust и authorization. Trust привязан
к проверенному содержимому, а не к пути или суффиксу.

## Files и launcher

Files представляет распознанный application Bundle одним объектом с display
name из manifest и стандартной/валидированной иконкой. Основное действие —
запрос запуска через активную UserSession и `luna-app-runtime`.
Отдельное действие «Показать содержимое» открывает внутреннее дерево.
Повреждённый candidate получает диагностику и не запускается как корректный Bundle.

Launcher получает установленные приложения из API менеджера; portable discovery
обрабатывается явно. Files, CLI и launcher используют одну библиотеку распознавания
из `luna-bundle` и один backend `luna-app-manager`.

## Версии, данные и ресурсы

Managed storage сохраняет текущую группировку по ID, например:

```text
LUNA-DATA/system/apps/.bundles/org.luna.cli/
├── 0.1.0.app/
└── 0.2.0.app/
```

UI показывает приложение и доступные версии. Переименование directory не меняет
BundleId, version или ContentIdentity; managed relocation выполняется менеджером.
Путь приложения в runtime остаётся логическим.

Данные и конфигурация находятся соответственно под
`LUNA-DATA/users/<user>/data/<BundleId>` и `config/<BundleId>`; конкретный mapping
к ним определяется планом запуска и security. Выбор версии хранится вне пакета.
Update сохраняет предыдущую версию. Уже работающий ApplicationInstance сохраняет
свою привязку к версии; физическое удаление используемого payload требует
координации с app-runtime. Сохранение данных при удалении не означает автоматической
совместимости их schema между версиями; миграции данных — отдельная операция.

## Порядок реализации после согласования

1. Зафиксировать публичный layout/recognition contract и descriptor schema.
2. Добавить bounded inspect/verify directory API в существующий `luna-bundle`.
3. Соединить каталог установленного состояния в `luna-state` с UpdateEngine;
   сохранить recoverability каждой физической и state операции.
4. Мигрировать старые версии явной журналируемой операцией. Существующий приватный
   layout остаётся читаемым до завершения миграции; users и loose providers
   не перемещаются. Смена layout не должна менять ContentIdentity или trust pin.
5. Подключить Files/launcher и установленный Bundle к активной UserSession через
   существующий app-runtime, затем выполнить canonical QEMU acceptance.

Критерии будущей проверки: переименованный пакет сохраняет identity; ложный `.app`
отклоняется; изменение manifest/descriptor/bytes и unsafe entries обнаруживается;
копия пакета не выдаёт installed/trust/grants; обе версии доступны независимо;
прерывание migration/registration восстанавливается; данные сохраняются после
update/remove; discovery не исполняет код; запуск и завершение подтверждены в QEMU.
Эти проверки описаны как критерии; реализация и тесты этого proposal ещё не выполнены.
