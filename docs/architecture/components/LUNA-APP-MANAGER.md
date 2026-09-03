# `luna-app-manager`

**Статус:** принятая граница; интеграция и hardening продолжаются.

## Назначение

Управляет жизненным циклом установленных Luna Bundles и связанной с ними изменяемой application data. Компонент отвечает за состояние установленного приложения, но не за выполнение его процессов.

## Владеет

- install/import Bundle;
- verification и registration;
- update/removal;
- migration;
- policy очистки application data;
- импорт поддерживаемых `.deb`/`.rpm` в Luna Bundle form.

## Не владеет

`UserSession`, запуском `ApplicationInstance`, созданием namespace, authorization policy, низкоуровневым filesystem backend или транзакциями обновления System Image.

## Установка

Безопасный поток:

```text
inspect
  ↓
validate
  ↓
integrity / trust checks
  ↓
security decision
  ↓
stage
  ↓
atomic commit
```

Ошибка должна приводить к откату незавершённой операции: частично зарегистрированный Bundle недопустим.

## Хранилище

Установленные immutable Bundles находятся в `DATA/system/apps`. Пользовательские данные и настройки хранятся в соответствующем `DATA/users/<user>/`.

## Зависимости

Использует `luna-bundle`, `luna-fs`, `luna-config`, `luna-security`, `luna-state` и update contracts при необходимости.

## Открыто

Полная dependency resolution, reconciliation транзакций, миграции данных и hardening импорта сторонних пакетов.