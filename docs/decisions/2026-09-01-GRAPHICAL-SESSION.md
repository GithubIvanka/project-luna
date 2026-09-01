# Project Luna — Graphical Session Decisions

**Дата:** 2026-09-01  
**Статус:** принято  
**Архитектурный SoT:** `docs/ARCHITECTURE.md`

## 1. Никакого TTY в штатной загрузке

TTY не является частью обычного пользовательского boot/session flow Luna.

Штатная последовательность:

```text
UEFI
 ↓
luna-boot.efi
 ↓
Linux kernel
 ↓
early userspace
 ↓
System Image + DATA
 ↓
luna-system-runtime
 ↓
graphical login UserSession
 ↓
authentication
 ↓
Active UserSession
 ↓
Wayland session
 ↓
niri
 ↓
Noctalia Shell
```

TTY/serial shell может существовать только как development, diagnostic или recovery mechanism. Он не является штатным способом запуска графического интерфейса.

## 2. Login = UserSession

Экран входа является частью `UserSession` lifecycle, а не отдельной заменой UserSession.

Принята последовательность состояний:

```text
Starting
  ↓
Authenticating
  ↓
Active
```

При отмене/ошибке входа:

```text
Authenticating
  ↓
Ending
  ↓
Ended
```

Аутентификация не должна оставлять пользователю уже активный полноценный session context до успешной проверки credentials.

## 3. Desktop stack

Целевое пользовательское окружение:

```text
Wayland
  ↓
niri
  ↓
Noctalia Shell
```

`niri` остаётся compositor/window manager, а Noctalia — desktop shell/UI layer. Они не становятся частью `luna-system-runtime` domain model.

Niri запускается как компонент графической сессии. Его session integration, portals и desktop services должны быть подключены через принятый Linux session/service mechanism, не через пользовательский TTY flow.

## 4. DATA/system/drivers

Канонический DATA layout обязательно включает:

```text
DATA/system/
├── apps/
├── drivers/
├── libs/
├── volumes/
├── config/
└── state/
```

`drivers/` является частью system-managed DATA и не является пользовательским application data.

## 5. Запись решения

Данное решение уточняет session/desktop implementation и не изменяет базовую модель `EFI / SYSTEM / DATA / SWAP`, `UserSession`, `luna-system-runtime`, Security, namespace или Bundle architecture.
