# `luna-user-session`

## Назначение

`UserSession` — доменная сущность, объединяющая identity пользователя и конкретный lifecycle его входа в систему.

## Владеет

- identity пользователя и сессии;
- `LoginState` и переходами authentication flow;
- lifecycle сессии;
- связь аутентифицированного пользователя с пользовательским execution context;
- session authentication и credential boundary;
- seat ownership/access;
- input device access и нормализацию Luna input events;
- DRM/KMS access и graphics session lifecycle;
- compositor/session UI lifecycle.

`luna-user-session` является единой Group C boundary. Эти функции являются внутренними библиотеками/modules одного crate/runtime boundary, а не обязательными отдельными daemon processes.

## Не владеет

`luna-user-session` не является generic system supervisor и не владеет общим process supervision.

Внешние provider/reference backends допускаются только там, где Luna пока не владеет полным контрактом: libseat/libinput и desktop provider stack могут использоваться как compatibility layers. Отдельные `seatd`, `greetd` и Noctalia Greeter больше не являются runtime-компонентами Luna.

## Состояния

Для активной архитектуры важны три состояния пользовательской сессии:

```text
ACTIVE
RESTRICTED
TERMINATED
```

До перехода в `ACTIVE` UserSession переводит `LoginState` из `VISIBLE` в `AUTHENTICATING` и выполняет native authentication boundary. В текущем Alpha выбранная boot/session identity валидируется внутри `luna-user-session`; отдельный greeter/login daemon не запускается. После успешной аутентификации `LoginState` становится `SUCCEEDED`, а `SessionState` — `ACTIVE`. Поле для интерактивного ввода credentials остаётся задачей будущего native SessionUI.

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

## Внутренняя структура

Целевая внутренняя структура:

```text
UserSession
├── auth
├── credentials
├── seat
├── input
├── graphics
├── compositor
└── session_ui
```

Каждый модуль имеет узкий контракт и может тестироваться отдельно, но для boot path они образуют одну UserSession runtime boundary.

## Статус

State model, session identity и базовый session lifecycle уже объединены в одном компоненте. Login flow, seat, input, graphics и compositor постепенно переносятся из transitional external providers в эти внутренние модули. Пока provider backend ещё используется, это явно помечается как переходный слой.