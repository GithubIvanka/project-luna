# `luna-app-manager`

## Назначение

Управляет жизненным циклом установленных Bundle приложений.

## Владеет

- планированием установки, обновления, удаления и импорта Bundle;
- проверкой входных Bundle;
- координацией integrity и trust checks;
- регистрацией установленных приложений;
- lifecycle пользовательских данных приложения и поиском orphaned data.

## Не владеет

`luna-app-manager` не запускает процессы и не заменяет `luna-app-runtime`.

## Зависимости

`luna-common`, `luna-bundle`, `luna-security`, `luna-state` и
`luna-update-manager`. Filesystem backend реализует `UpdateBackend`; журнал и
фазы операции принадлежат `UpdateEngine`.

## Жизненный цикл

```text
inspect → validate → stage → commit → registered
                     ↘ failure / cleanup
```

## Хранение

Установленные Bundle находятся в `LUNA-DATA/system/apps`. Этот физический путь не является частью семантики manifest Bundle.

Bundle разделяется между пользователями, разные версии могут сосуществовать. Пользовательские данные и конфигурация остаются вне immutable Bundle.

## Зависимости приложения

Отсутствующие зависимости не скачиваются молча: менеджер определяет требование, ищет подходящий источник, объясняет операцию и получает необходимое подтверждение/разрешение.

## Статус

Реализован `FilesystemApplicationManager`: проверка настоящего LBP1, trust для
точного ContentIdentity, отдельная management authorization, staging, атомарная
публикация версии, полная проверка установленного дерева, удаление и recovery
прерванных операций. Данные и конфигурация пользователей не посещаются при
установке, rollback или удалении.

Текущий приватный каталог backend — `system/apps/.bundles-v2/<opaque slot>`.
Он содержит external `receipt.toml` и приложение с обычным именем
`luna-cli.lunaapp/{manifest.toml,integrity.toml,payload/}`. ID/Version остаются
metadata; транспортный `.lbp` не устанавливается. Verify восстанавливает тот
же RFC ContentIdentity из directory Bundle и сверяет с external receipt.
Физические пути не становятся API приложения. Provider directories сохраняются.

Прежний prototype `.bundles/<id>/<version>` не преобразуется автоматически.
При установленном legacy Bundle или pending checkpoint новый manager выдаёт
диагностику и сохраняет старое хранилище. Journalled migration ещё не реализована;
проверки нового формата используют отдельный offline DATA fixture.

Update устанавливает независимую новую версию, сохраняя старую. Launch caller
указывает зарегистрированные ID и Version; автоматического выбора default
версии пока нет. Management CLI работает только с явно выбранным **offline DATA**.
Отдельный локальный launch IPC уже связывает authenticated UserSession с
App Runtime: verified static/dynamic musl Bundle и один authorized Wayland
endpoint. Read-only desktop catalog использует bounded manager inventory и
отдельную user/content Launch policy через existing broker; registration/content
verification не является execution grant. См.
[DESKTOP-DISCOVERY-STAGE.md](../../development/application-bundles/DESKTOP-DISCOVERY-STAGE.md).
Live management IPC остаётся следующим этапом. Подписи и зависимости пока отклоняются при установке
с явной диагностикой; они не считаются проверенными и не скачиваются молча.

Подробности и проверяемый сценарий:
[`DIRECTORY-STAGE.md`](../../development/application-bundles/DIRECTORY-STAGE.md)
и [`DIRECTORY-BUNDLE-CONTRACT.md`](../../contracts/DIRECTORY-BUNDLE-CONTRACT.md).
Границы запуска и evidence:
[`DYNAMIC-WAYLAND-STAGE.md`](../../development/application-bundles/DYNAMIC-WAYLAND-STAGE.md).
