# `luna-login`

**Статус:** интеграция с greetd/Noctalia существует; финальная Luna authentication IPC не завершена.

## Назначение

Графическая граница входа пользователя в `UserSession`.

## Владеет

- graphical login flow;
- выбором пользователя;
- передачей authentication request;
- представлением успеха/ошибки входа;
- созданием перехода к Active `UserSession` после успешной аутентификации.

## Не владеет

Identity database, authorization policy, application runtime, system-wide supervision или UEFI boot.

## Правило состояния

```text
Starting
  ↓
Authenticating
  ↓ success
Active
```

Ошибка, отмена или отказ authentication никогда не переводят UserSession в Active.

## Provider

greetd/greeter может быть implementation infrastructure. Наличие provider не создаёт нового Luna architectural component.

## Открыто

Финальная authentication IPC, credential backend, session switching и production security integration.