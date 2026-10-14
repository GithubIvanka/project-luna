# Контракт запуска приложения

**Статус:** принятый архитектурный контракт
**Область:** `UserSession` → `luna-app-runtime` → `ApplicationInstance`

## 1. Общая модель

`luna-app-runtime` получает запрос на запуск и строит один `ApplicationInstance`. Внутри жизненного цикла этого экземпляра выполняется:

```text
планирование
  ↓
mapping
  ↓
authorization
  ↓
materialization
  ↓
запуск
```

Конкретная последовательность:

```text
ApplicationPlan
    ↓
MappingPlan
    ↓
luna-root-mapping
    ↓
luna-security
    ↓
AuthorizedApplicationPlan
    ↓
luna-namespace
    ↓
ApplicationInstance
    ↓
процесс приложения
```

## 2. ApplicationPlan и MappingPlan

`ApplicationPlan` содержит identity Bundle/приложения, версию, `UserSession`, `RuntimeSpec`, executable, аргументы, требования к ресурсам, mappings и запросы разрешений. План не является разрешением.

`MappingPlan` описывает логические пути и допустимые классы источников. `luna-root-mapping` владеет семантикой mapping, его валидацией и построением детерминированного представления.

Физические пути `LUNA-SYS/...` и `LUNA-DATA/...` не являются частью публичной семантики Bundle.

## 3. RuntimeSpec и выбор libc

`RuntimeSpec` является частью `ApplicationInstance`. Он определяет execution environment приложения.

Для приложения выбирается ровно одна libc/runtime-среда:

```text
musl
или
Glibc
```

`musl` — native runtime Luna. `glibc` — compatibility runtime для приложений, которым требуется glibc. Сам выбор libc не является отдельным daemon или runtime-компонентом.

`RuntimeProfile` отдельно описывает минимальный набор доверенных системных логических ресурсов, доступных приложению. Он не выбирает libc.

## 4. Авторизация

`luna-security` — единственная policy authority. Он проверяет application identity, mappings, capabilities и иные requests и создаёт sealed `AuthorizedApplicationPlan`.

```text
request != grant
```

Authorization должна завершиться до materialization. `Deny`, ошибка политики и неподдерживаемое ограниченное решение приводят к fail closed.

Trust, криптографическая подпись и authorization — разные решения. Доверие Bundle не означает автоматического предоставления прав приложению.

## 5. Materialization и namespace

`luna-namespace` получает только авторизованный результат и реализует Linux-specific materialization:

- отдельный mount namespace для каждого `ApplicationInstance`;
- RAM-backed logical `/`;
- разрешённые mappings;
- `RuntimeProfile`;
- необходимые runtime filesystems;
- требуемые ограничения доступа;
- финальная подготовка процесса.

PID namespace по умолчанию не создаётся. Приложение остаётся обычным процессом в системном PID namespace и получает обычный PID, отличный от `1`.

## 6. ApplicationInstance

`ApplicationInstance` принадлежит `luna-app-runtime` и хранит identity экземпляра, identity/версию приложения, session identity, `RuntimeSpec`, lifecycle state, PID/process identity после создания, результат завершения и диагностическую информацию об ошибке.

Внешний caller не меняет lifecycle напрямую.

Базовые состояния:

```text
Created → Starting → Running → Stopping → Stopped
Starting → Failed
Running → Crashed
Stopping → Failed
```

## 7. Граница компонентов

```text
luna-app-manager   → установка и lifecycle Bundle
luna-app-runtime   → ApplicationInstance и запуск
luna-root-mapping  → mapping semantics / MappingPlan
luna-security      → authorization / trust policy
luna-namespace     → namespace/materialization
luna-system-runtime → system-wide supervision
```

Новый application init/supervisor не создаётся.

Process host предоставляется существующим `luna-system-runtime`: App Runtime
сохраняет lifecycle экземпляра, а System Runtime владеет spawn/wait/termination.
Linux descendant boundary должна охватывать также процессы, сменившие process
group/session через `setsid`. Текущий backend использует отдельную cgroup v2 и
не освобождает materialization до завершения её потомков.

## 8. ELF dependency closure

luna-app-runtime содержит этап планирования ELF-зависимостей, который не вызывает host dynamic loader. Анализатор извлекает класс ELF, endian, machine, PT_INTERP, DT_NEEDED, DT_RPATH и DT_RUNPATH.

ElfDependencyClosure рекурсивно обходит interpreter и shared objects через явный ElfDependencyResolver. Циклы схлопываются в множество closure; несоответствие архитектуры приводит к fail closed.

FilesystemElfResolver работает только с явно переданными доверенными источниками и не использует LD_LIBRARY_PATH, host ld.so.cache или host default directories. $ORIGIN нормализуется лексически.

Installed native-musl adapter подключает static entry либо dynamic closure к
MappingPlan и отдельной Launch authorization. Interpreter и shared objects
выбираются только из явно объявленных mappings того же verified Bundle; каждый
ELF node требует точных Read/Execute grants. Canonical musl interpreter и fixed
logical search paths исключают host fallback. RPATH/RUNPATH и custom musl path
отклоняются; metadata virtual/file translations проверяются перед authorization.
Выбор dependencies из других providers остаётся отдельной стадией.
Границы и evidence: [DYNAMIC-WAYLAND-STAGE.md](../development/application-bundles/DYNAMIC-WAYLAND-STAGE.md).

## 9. Wayland resource handoff

### Immutable default arguments and desktop content pin

Optional `entry.args` в TOML manifest содержит argv defaults, не shell text.
Отсутствующее/пустое поле canonical writer не выводит, сохраняя прежнюю
ContentIdentity. Непустые defaults входят в ContentIdentity. Installed planner
добавляет их перед caller argv; суммарно максимум 64 аргумента, каждый максимум
4096 bytes, без NUL. Никакая shell expansion не выполняется.

Format-1 RunRequest сохраняет прежние обязательные поля и допускает optional
`content_identity`. CLI: `luna app run ID VERSION --expect-content BLAKE3 -- ARGS`.
Pin — ровно 64 lowercase hex digits. Broker сравнивает его сразу после manager
verification, до compositor probe, storage и spawn. Pin защищает от stale
presentation; обычная exact-content Launch authorization остаётся обязательной.

### Registered desktop catalog

Existing application broker принимает `{format=1, operation="catalog"}` от того
же authenticated UID/desktop cgroup, что и RunRequest. Caller не задаёт user,
session, DATA path или grants. `luna app list` — read-only live catalog; прежние
offline management команды остаются отдельным operator API.

Manager ограничивает enumeration до 64 registered versions, проверяя каждый
receipt/layout/manifest/ContentIdentity; overflow или повреждённая регистрация
отклоняют весь snapshot. Catalog выбирает supported native-musl graphical
applications с `desktop.wayland` и exact-content user policy. Shared installed
planner проверяет ELF closure/mappings/default argv и полные authorization
requests, включая compositor logical Read. Discovery не probe-ит compositor,
не создаёт data/config/home, mounts или endpoint proof и не возвращает usable
launch plan. Реальный запуск повторяет verification/authorization и требует
настоящий sealed Wayland proof. Catalog не обещает доступность runtime endpoint.

Bounded TOML response (128 KiB) содержит ID/version/content hash/friendly manifest
name и terminal presentation flag, без physical paths. Duplicate ID/version,
unsafe identifiers, noncanonical versions, controls/NUL или name >512 bytes
отклоняются. Каждая версия имеет отдельный opaque desktop filename и explicit
version в Exec; версия автоматически не подменяется.

System Runtime публикует root-owned ephemeral entries под
`/run/luna/desktop-applications/share/applications`: на login до Noctalia и при
успешном explicit catalog refresh. Ancestors root-owned, не user runtime home.
Application directory inode сохраняется; files 0444 обновляются atomic rename
из sibling staging. Ошибка refresh очищает только собственные entries, logout
удаляет собственные objects. Неизвестные objects не удаляются recursively.

Noctalia получает share path в XDG_DATA_DIRS и использует свой scanner/inotify.
Generated Exec содержит только validated ID/version/hash и `/usr/bin/luna`,
никогда manifest name/default argv/shell command; Terminal/DBusActivatable=false.
Name — key-file string с literal internal spaces и escaped backslash. Иконки
пока generic; для terminal — terminal icon. Эти файлы — presentation cache,
не receipt/trust/grant. Noctalia также читает обычные user/system entries;
они не становятся registered Bundle от попадания в меню. Arbitrary directory
suffix `.lunaapp` не является proof установки.

Live install/update/remove и automatic management-triggered refresh ещё не
предоставлены IPC. Registered Terminal не получает доступ к broker socket;
его filesystem/cgroup grants ради refresh не расширяются. Explicit list
доступен trusted desktop clients текущей UserSession.

`desktop.wayland` является запросом capability к desktop provider. Он требует
отдельного policy grant и точного socket mapping Read. System Runtime выбирает
endpoint только текущей authenticated UserSession, проверяет UID, peer PID/cgroup
и формирует proof. Grants предшествуют privileged endpoint probe; полная plan
authorization предшествует storage/materialization.

App Runtime сверяет proof с sealed plan и credentials; namespace закрепляет один
socket inode и повторно проверяет peer перед exec. Приложение подключается после
смены UID. Parent runtime directory и root-connected socket FD не передаются.
Kernel enforcement должен ограничивать named и abstract UNIX endpoints; текущий
backend требует Landlock ResolveUnix ABI9 и AbstractUnixSocket scope, fail closed.
Grant выбранного compositor разрешает его Wayland protocol; filtering отдельных
interfaces и другие desktop resources здесь не реализованы.

## Home и приватный терминал installed musl Bundle

`user.home` — явный запрос доступа к home текущей активной UserSession.
Installed adapter выбирает `selected DATA/users/<session-user>/home` и отображает
его как `/home`. Bundle не передаёт physical path или другого пользователя;
`/home` зарезервирован для runtime и не может быть затенён payload mapping.
Root-owned exact-content policy отдельно предоставляет `Use(user.home)` и
Read/Write/Execute grants `/home`. Broker проверяет UID владельца и no-follow
цепочку DATA/users/user/home после authorization. Mapping закрепляется защищённым
mount fd. HOME становится `/home`; `/data` и `/config` остаются приватными
каталогами этого приложения. Capability не предоставляет внешние тома или
каталоги других пользователей.

Writable subtree mappings разрешают обычные файлы, каталоги, symlinks и Refer
для работы file manager; MakeChar/MakeBlock/MakeFifo/MakeSock этим правом не
предоставляются. Read-only mappings остаются без MakeSym. Luna Files применяет
свою no-overwrite/staging/Trash policy поверх этого mapping; подробности:
[Files operations](../development/application-bundles/FILES-OPERATIONS-STAGE.md).

`terminal.pty` предоставляет только synthetic `/dev/{null,zero,random,urandom,tty}`
и приватный `devpts` с `newinstance`, `/dev/ptmx -> pts/ptmx`. Он требует отдельного
capability grant и совпадающих non-root credentials активной сессии. Process host
создаёт POSIX session через `setsid`, отделяя inherited controlling TTY; terminal
setup проверяет session leadership. Shell получает controlling TTY только после
создания своего slave в приватном devpts. Host console, existing PTY, disks, GPU
и input devices не отображаются. Landlock разрешает только Read/Write/ReadDir
этого synthetic `/dev`; MakeChar/MakeBlock/Execute и создание устройств запрещены.

Для terminal instance приватный tmpfs `/tmp` допускает scratch files и named UNIX
socket внутри этого же instance (Yazi DDS). Rule разрешает MakeSock/ResolveUnix
только этого `/tmp`, не host runtime; abstract UNIX scope сохраняется. Execute,
MakeChar/MakeBlock не предоставляются. Эти временные данные исчезают с instance.

Ghostty создаёт PTY и запускает fish внутри того же ApplicationInstance, namespace
и cgroup. Новый supervisor или CLI FD relay не вводится. Стандартные потоки GUI
приложения по-прежнему `/dev/null`; его shell использует созданный Ghostty PTY.
Обычный `luna app run` для CLI приложения ещё не пересылает интерактивный terminal
caller. Завершение окна/лидера и logout используют существующий cgroup cleanup.

Для текущего desktop subset broker выбирает Wayland GDK и Cairo GSK; для
`terminal.pty` — software Mesa/llvmpipe и `/bin/fish`. Это не GPU capability.
Payload должен явно содержать требуемые GTK, Mesa, shell, fonts и terminfo
ресурсы. Dlopen resources задаются явно при подготовке Bundle; автоматического
provider selection или host fallback нет.

## 10. Открытие выбранного файла из приложения

Запрос `application.open` разрешает отображение ровно
`/run/luna/application.sock` существующего System Runtime. Нужны отдельные
exact-content разрешения Use этой capability и Read сокета. Каталог `/run/luna`
целиком не предоставляется. Доказательство закрепляет собственный root-owned
сокет посредника; его inode проверяется после монтирования перед exec.

Посредник различает доверенную cgroup рабочего стола и cgroup конкретного
запущенного ApplicationInstance. Только рабочий стол принимает обычные Run/Catalog.
Экземпляр с `application.open` и разрешённым HOME получает только запросы
`file-handlers` и `open-file`; он не выбирает пользователя, сессию или grants.
Отправитель определяется SO_PEERCRED и проверяется для каждой порции через
SO_PASSCRED/SCM_CREDENTIALS, затем повторно сверяется с живой cgroup перед
обработкой. SCM_RIGHTS отклоняются с закрытием всех полученных дескрипторов.
Передача унаследованного соединения другому процессу не передаёт ему роль клиента.
Механика подтверждается [описанием UNIX-сокетов Linux](https://man7.org/linux/man-pages/man7/unix.7.html)
и [recvmsg](https://man7.org/linux/man-pages/man2/recvmsg.2.html).

Необязательное `entry.file_open` манифеста объявляет отдельный неизменяемый
режим просмотра одного файла. Пример для Terminal:

```toml
[entry.file_open]
mime_types = ["text/plain"]
args = ["--working-directory=/tmp", "-e", "/bin/less"]
```

Отсутствующее поле не выводится каноническим writer и сохраняет прежнюю
ContentIdentity. Поле входит в идентичность содержимого бандла. MIME — заявленная
возможность обработчика, выбранная пользователем, а не доверенное определение
байтов файла. Максимум 16 уникальных точных MIME, без параметров и шаблонов.
Аргументы: entry.args, затем file_open.args, затем фиксированный `/open/selected`;
суммарно максимум 64, каждый максимум 4096 байтов без NUL. Аргументы от caller
этот режим не принимает; оболочка для их интерпретации не запускается.

Режим запрашивает Use(user.selected-file) и Read(`/open/selected`) отдельно
от обычных разрешений приложения. Он исключает `user.home` и `application.open`
из эффективного плана и отображений: обычный Terminal/fish сохраняет HOME,
а экземпляр просмотра получает выбранный файл, свои data/config, ресурсы
бандла и явно разрешённые Wayland/PTY. Доказательство выбранного файла обязательно.
Весь `/open` зарезервирован и не может быть затенён полезной нагрузкой бандла.

Первый поддерживаемый источник — обычный файл владельца UID активной сессии
внутри её HOME, без перехода по символическим ссылкам в родителях или конечном
имени. Логический путь передаётся ограниченными hex-байтами, сохраняя имена
не в UTF-8. Физический путь определяет runtime из выбранной DATA.
O_PATH удерживает исходный inode; после защищённого read-only монтирования
проверяются session/UID, тип, dev/ino и точное File/Read отображение.
Подмена пути до монтирования приводит к отказу. Путь /proc/self/fd не используется
как обход ограничений защищённого монтирования. Представление файла остаётся
живым: изменение байтов через другой разрешённый путь не создаёт снимка.

Каталог обработчиков проверяет регистрацию, ContentIdentity, поддерживаемый
план и все разрешения этого режима без открытия файла, создания storage,
монтирования или выдачи пригодного для запуска плана. Ответ ограничен 128KiB
и 64 версиями. Запуск требует обязательного ID/version/content pin, совпадения
выбранного MIME и повторной полной авторизации до создания данных приложения.

Ответ open-file подтверждает успешный запуск, а не окончание обработчика.
System Runtime продолжает учёт и очистку всех экземпляров независимо от
соединения Files; выход из сессии завершает и отдельно запущенные обработчики.
Отдельный предел — 16 активных экземпляров. Новый системный сервис не вводится.
Внешние тома, запись в выбранный файл, автоматические ассоциации и управление
установкой из работающей сессии остаются отдельными этапами.
