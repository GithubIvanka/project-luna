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

До перехода в `ACTIVE` UserSession переводит `LoginState` из `VISIBLE` в `AUTHENTICATING` и выполняет native authentication boundary. В текущем Alpha интерактивный native `SessionUI` получает keyboard/pointer input через seat/input boundary, выполняет проверку credentials внутри `luna-user-session` и после успеха передаёт `UserCredentials` в session lifecycle. Отдельный greeter/login daemon не запускается. После успешной аутентификации `LoginState` становится `SUCCEEDED`, а `SessionState` — `ACTIVE`. В normal login flow `AccountDirectory` предоставляет eligible local accounts. При единственном аккаунте он автоматически выбирается и UI сразу показывает только пароль. При нескольких аккаунтах `SessionUI` сначала показывает selectable avatar cards с display name и username; после выбора вводится только пароль выбранной identity. Rendering login UI изолирован в отдельном renderer module того же crate и не является отдельным daemon.

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
├── renderer
├── rings
├── boot_presentation
├── compositor
└── session_ui
```

Каждый модуль имеет узкий контракт и может тестироваться отдельно, но для boot path они образуют одну UserSession runtime boundary.

В Alpha загрузочная презентация после запуска ядра также использует эту
графическую границу. `BootPresentation` временно владеет DRM master, KMS и
framebuffer во время реальной подготовки служб системным runtime. `rings`
вычисляет три каменные ленты и 48 независимых случайных циклов рун; это нативная
программная отрисовка без видеопетли. `luna-system-runtime` сообщает этапы и
останавливает презентацию до входа. Остановка ждёт потока, восстановления CRTC
и освобождения DRM; затем существующий SessionUi выполняет аутентификацию.
Это внутренние модули, отдельного загрузочного daemon или greeter нет.
Ранний UEFI использует отдельный статический знак.

Нативный вход обрабатывает относительные события мыши и абсолютные координаты
указателя evdev. Пределы `ABS_X`/`ABS_Y` считываются через `EVIOCGABS` и
нормализуются в пиксели активного KMS-экрана. Это позволяет использовать
виртуальный USB tablet QEMU при масштабировании окна без захвата мыши.
Полноценная калибровка сенсорных устройств остаётся задачей backend рабочего
стола; собственная граница входа не заменяет libinput.

## Статус

State model, session identity и базовый session lifecycle уже объединены в одном компоненте. Login flow, seat, input, graphics, renderer и compositor постепенно переносятся из transitional external providers в эти внутренние модули. Пока provider backend ещё используется, это явно помечается как переходный слой.
