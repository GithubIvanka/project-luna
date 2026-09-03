# `UserSession`

**Статус:** domain/lifecycle model реализована; production integration продолжается.

## Назначение

`UserSession` представляет одну интерактивную пользовательскую сессию как единую Luna domain entity.

## Владение

`UserSession` — session boundary, которую владеет и координирует `luna-system-runtime`. Это не отдельный процесс session manager.

## Состояния

Модель должна различать identity пользователя, session state и login/authentication state.

```text
Starting
  ↓
Authenticating
  ↓ success
Active
  ↓
Restricted / Ending
  ↓
Ended
```

Login failure или cancellation никогда не переводят session в `Active`.

## Desktop

```text
luna-system-runtime
 ↓
UserSession
 ↓
luna-login
 ↓
authentication
 ↓
Active UserSession
 ↓
niri-session
 ↓
niri
 ↓
Noctalia
```

`niri-session` здесь implementation detail, а не новый Luna component.

## Приложения

Одна UserSession может иметь несколько application runtime activities. `luna-app-runtime` получает session boundary при запуске ApplicationInstance.

## Несколько пользователей

Несколько UserSessions могут существовать одновременно. При уходе пользователя из активного desktop application behavior определяется policy; допустимы продолжение, restricted lifetime или завершение. Default restriction должен быть явным policy, а не побочным эффектом UI.

## Не владеет

Application execution implementation, system-wide supervision, authorization policy, bootloader behavior или GUI toolkit.

## Открыто

Session switching, restriction enforcement, logout/re-authentication и production authentication IPC.