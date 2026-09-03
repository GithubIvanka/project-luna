# `luna-cli`

**Статус:** тонкая клиентская граница реализована; полный command/IPC surface в разработке.

## Назначение

`luna-cli` предоставляет пользователю единый интерфейс управления Luna. CLI не должен дублировать внутреннюю бизнес-логику managers.

## Владеет

- разбором команд и аргументов;
- формированием запросов к соответствующим Luna services/clients;
- человекочитаемым и машинным выводом;
- кодами ошибок CLI.

## Не владеет

System Image transactions, Bundle parsing, authorization policy, namespace creation, UserSession supervision или raw filesystem implementation.

## Направление

Команда должна быть тонкой:

```text
luna <command>
   ↓
IPC / domain client
   ↓
ответ соответствующего компонента
```

## Возможные области

Конкретная CLI surface формализуется постепенно. Ожидаются домены `system`, `kernel`, `app`, `device`, `update`, `recovery`, но окончательные команды не считаются утверждёнными этим документом.

## Ошибки

Ошибки нижних компонентов должны сохранять семантику до пользовательского слоя, а не превращаться в generic «operation failed».

## Открыто

Полная команда/IPC surface, machine-readable output contract и окончательная UX-модель.