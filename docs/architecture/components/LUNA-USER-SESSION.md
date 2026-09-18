# `luna-user-session`

## Назначение

`UserSession` — доменная сущность, объединяющая identity пользователя и конкретный lifecycle его входа в систему.

## Владеет

- identity пользователя и сессии;
- `LoginState` и переходами authentication flow;
- lifecycle сессии;
- связь аутентифицированного пользователя с пользовательским execution context;
- внутреннюю реализацию login flow через внешний greetd/Noctalia Greeter.

## Не владеет

`luna-user-session` не является daemon, process supervisor, generic runtime layer или security authority.

## Состояния

Для активной архитектуры важны три состояния пользовательской сессии:

```text
ACTIVE
RESTRICTED
TERMINATED
```

До перехода в `ACTIVE` UserSession переводит `LoginState` из `VISIBLE` в `AUTHENTICATING` и выполняет login flow. После успешной аутентификации `LoginState` становится `SUCCEEDED`, а `SessionState` — `ACTIVE`. Ошибка аутентификации оставляет сессию неактивной и фиксируется в `LoginState::FAILED`.

`RESTRICTED` означает, что сессия сохраняется как управляемый контекст, но доступ и выполнение приложений ограничиваются согласно policy. Конкретная session policy может вместо этого разрешить продолжение или потребовать завершение приложений.

## Жизненный цикл

```text
создана
  ↓
аутентификация
  ↓
ACTIVE
  ↓
RESTRICTED
  ↓
ACTIVE
  ↓
TERMINATED
```

Переходы между `ACTIVE` и `RESTRICTED` не создают новую сессию.

## Взаимодействие

`luna-system-runtime` владеет коллекцией `UserSession` и координирует её lifecycle. `luna-app-runtime` получает активный session context и использует identity сессии при создании `ApplicationInstance`.

Несколько `UserSession` могут существовать одновременно. Переключение пользователя не требует пересоздания глобального system runtime.

## Политика приложений

Для каждой сессии политика может разрешать:

```text
продолжить выполнение
оставить под управлением в RESTRICTED
завершить приложения
```

Системные службы и операции обновления могут продолжать работу при смене пользователя, если это безопасно.

## Статус

State model, session identity и графический login flow объединены в одном компоненте. Handoff entry point поставляется тем же crate и не образует отдельного архитектурного компонента. greetd и Noctalia Greeter остаются внешними механизмами аутентификации и отображения login UI.