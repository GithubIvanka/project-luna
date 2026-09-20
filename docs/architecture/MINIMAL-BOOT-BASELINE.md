# Minimal Linux baseline для Luna

## Назначение

OneFileLinux фиксируем как поведенческий эталон минимальной рабочей Linux-системы, а не как кодовую базу для копирования.

Его важная идея:

~~~
UEFI
  ↓
EFI-stub Linux kernel
  ↓
встроенный initramfs
  ↓
/init
  ↓
PID 1
  ↓
системная инициализация
  ↓
login/session
~~~

В OneFileLinux rootfs встраивается в kernel через `CONFIG_INITRAMFS_SOURCE="../alpine-minirootfs"`, а `/init` передаёт управление `/sbin/init`. Alpine OpenRC затем выполняет sysinit/boot/default, после чего `inittab` поднимает getty. Для Luna это только reference behavior: Alpine, OpenRC, getty и консольный login в целевую runtime-цепочку не входят.

## Основной принцип

### Один минимальный boot artifact

Boot-critical часть должна быть максимально самодостаточной:

~~~
kernel + Luna boot payload + минимальный userspace
~~~

Сейчас физическое размещение Luna остаётся:

~~~
LUNA-SYS
├── kernels/
├── cores/
└── images/
~~~

System Image содержит только boot/runtime minimum до UserSession boundary. GUI provider не входит в него: normal/factory используют Physical LUNA-DATA, Recovery использует singleton Recovery DATA Image.

Всё остальное относится к расширению DATA.

### Kernel делает критический device work сам

Не следует строить boot path вокруг нескольких демонов, каждый из которых должен успеть стартовать раньше следующего.

Для минимального desktop profile kernel должен иметь встроенными необходимые механизмы:

~~~
devtmpfs
proc/sysfs
tmpfs
ELF
epoll/eventfd/signalfd/timerfd
Unix sockets
input/evdev
DRM/KMS
нужный GPU provider
нужный storage/filesystem backend
~~~

Модули допустимы для необязательного оборудования и расширений после старта.

### PID 1 остаётся маленьким

`luna-init` отвечает за:

~~~
boot handoff
  ↓
ранний /dev /proc /sys /run
  ↓
System Image
  ↓
DATA
  ↓
logical root
  ↓
luna-system-runtime
~~~

Он не должен становиться вторым desktop supervisor.

## Целевые группы Luna

Архитектурный компонент не обязан быть отдельным процессом. Узкие обязанности можно оставить отдельными Rust crates/modules внутри одной runtime boundary.

### Group A — Boot

Процесс:

`luna-boot.efi`

Внутри:

~~~
discovery
GPT/filesystem reader
target resolver
boot policy
handoff builder
kernel loader
boot-attempt state
~~~

После `ExitBootServices` эта группа заканчивается.

### Group B — System Core

Процессы:

~~~
luna-init (PID 1)
    ↓
luna-system-runtime
~~~

Внутри остаются узкие Luna crates:

~~~
filesystem
DATA attachment
state
config
event
device inventory
service supervision
security foundations
update/recovery coordination
~~~

Они не должны превращаться в длинную последовательную цепочку отдельных daemon процессов.

### Group C — UserSession

Это единая desktop/session boundary:

~~~
UserSession
├── identity
├── authentication
├── credentials
├── seat
├── device access
├── input
├── DRM/KMS
├── graphics buffers
├── Wayland protocol
├── compositor
├── login UI
└── desktop session lifecycle
~~~

Снаружи:

~~~
luna-system-runtime
        ↓
UserSession
        ↓
active graphical session
~~~

Функциональность seatd/libseat, необходимый DRM access, libinput-like input processing, greetd-like auth/session boundary и wlroots-like compositor responsibilities рассматриваются как reference contracts. Это не означает переносить эти проекты целиком.

### Group D — Application Runtime

После ACTIVE UserSession:

~~~
UserSession
   ↓
luna-app-runtime
   ↓
ApplicationInstance
~~~

Внутри уже принятой модели:

~~~
ApplicationPlan
  ↓
MappingPlan
  ↓
luna-security
  ↓
AuthorizedApplicationPlan
  ↓
luna-namespace
  ↓
process
~~~

## Как извлекаем минимум из внешнего проекта

Для каждого внешнего проекта рассматриваем четыре уровня:

1. Kernel contract — device nodes, ioctls, netlink/events, sysfs.
2. Userspace contract — обязательные операции.
3. State machine — действительно нужные состояния и переходы.
4. Compatibility — функции, необходимые только сторонним приложениям.

В System Image попадает только необходимое из первых трёх уровней. Compatibility может быть provider layer и жить на DATA.

## Минимальный UserSession

System Image не должен содержать обязательную desktop shell/compositor цепочку.

Normal / Factory:
~~~
Physical LUNA-DATA → Niri + Noctalia
~~~

Recovery:
~~~
Recovery DATA → Niri + Noctalia + Ghostty + recovery tools
~~~

Целевой путь:

~~~
UserSession
  ├── SessionAuth
  ├── SessionCredentials
  ├── SeatController
  ├── InputBackend
  ├── GraphicsBackend
  ├── Compositor
  └── SessionUI
~~~

Все эти части могут находиться внутри `luna-user-session` crate и одной runtime boundary.

### SeatController

Не переносим весь seatd.

Нужно:

- определить active seat;
- открыть/передать UserSession доступ к DRM/input;
- контролировать ownership lifecycle;
- корректно отдать ресурсы при завершении session.

### InputBackend

Не переносим весь libinput.

Для Alpha достаточно:

- обнаружение `/dev/input/event*`;
- чтение `struct input_event`;
- keyboard/pointer classification;
- repeat/debounce при необходимости;
- преобразование в Luna input events;
- hotplug и reopen.

Udev database не должен быть обязательным условием basic input discovery.

### GraphicsBackend

Не переносим весь wlroots.

Минимум:

~~~
open DRM device
select connector/encoder/crtc
modeset
allocate framebuffer
mmap/scanout
page-flip
DRM event handling
bind output to active session
~~~

Для QEMU сначала достаточно virtio-gpu. Реальные GPU остаются provider boundary.

### Compositor

Не переносим wlroots целиком.

Первый вариант может быть минимальным:

- один output;
- один seat;
- один foreground surface;
- минимальный Wayland object model;
- `wl_display`;
- `wl_registry`;
- `wl_compositor`;
- `wl_shm`;
- необходимый shell/input минимум.

Расширенный xdg-shell/client compatibility добавляется после подтверждения basic graphical session.

### SessionAuth

Не запускаем отдельный greetd daemon как обязательный boot component.

UserSession выполняет:

~~~
credentials
  ↓
authentication policy
  ↓
authenticated identity
  ↓
session activation
~~~

Интерактивная аутентификация теперь является Luna-native boundary; greetd сохраняется только как reference source и не запускается в runtime.

### SessionUI

Не делаем Noctalia или Niri boot requirement для самого System Image. Они являются Physical DATA desktop providers.

Recovery GUI baseline предоставляется Recovery DATA Image через тот же Niri provider, что и normal/factory DATA; seat/session ownership выполняется Luna core.

~~~
background
username
password/input field
status/error
login action
~~~

Полный shell/theme stack может поставляться с DATA.

## TTY и console

Из runtime boot path удаляются:

~~~
getty
OpenRC console login
multiple tty logins
console shell
~~~

Это не обязательно означает отключение всего kernel TTY/PTY support.

PTY нужен терминальным приложениям, поэтому:

- boot console = debug-only;
- virtual console login = не boot requirement;
- PTY/TTY capability = сохраняем, пока поддерживаются terminal applications.

## Минимальный syscall surface

Точный список зависит от реализации, но boot/session path должен иметь небольшой контролируемый surface.

### Boot / PID 1

~~~
openat
close
read
write
fstat/statx
mmap
munmap
mount
umount
chdir
dup3
fcntl
execve
waitid/waitpid
reboot
~~~

### Device / event

~~~
socket
bind
recvmsg
sendmsg
poll/ppoll
epoll
ioctl
readlinkat
getdents64
~~~

Hotplug channel: netlink kobject uevent.

### Session / credentials

~~~
setgroups
setresgid
setresuid
setsid
setpgid
prctl
umask
~~~

### Graphics

Основная часть:

~~~
openat
ioctl(DRM)
mmap
poll/epoll
~~~

### Internal IPC

~~~
AF_UNIX
SOCK_STREAM / SOCK_SEQPACKET
socketpair
bind/listen/accept/connect
sendmsg/recvmsg
SCM_RIGHTS
~~~

D-Bus остаётся compatibility layer, а не foundation boot path.

## Что остаётся на DATA

В DATA постепенно уходят:

~~~
полные fonts
themes
icons
locales/translations
sounds
desktop shell
большие application runtimes
glibc runtimes
large GPU/codec providers
third-party desktop services
~~~

System Image сохраняет только минимальные ресурсы, необходимые для базовой UserSession и восстановления.

## OneFileLinux → Luna

| OneFileLinux | Luna |
|---|---|
| UEFI запускает EFI image | `luna-boot.efi` |
| EFI-stub kernel | Luna Linux kernel |
| встроенный initramfs | System Image/direct-init payload |
| `/init` | `luna-init` |
| `/sbin/init` + OpenRC | `luna-system-runtime` |
| OpenRC sysinit/boot/default | Luna startup graph |
| getty | отсутствует |
| console login | UserSession |
| Alpine userspace | Luna System Image |
| kernel modules в rootfs | boot-critical встроены, optional providers отдельно |

## Критерий минимальности System Image

Новая зависимость попадает в System Image только если:

~~~
без неё kernel не может стартовать
или
без неё PID 1 не может подготовить runtime
или
без неё нельзя найти/подключить DATA
или
без неё UserSession не может показать базовый login/GUI
или
без неё не работает обязательный security boundary
~~~

В остальных случаях:

~~~
provider / compatibility / optional
~~~

и по возможности поставляется с DATA.

## Переход Alpha

Текущие внешние provider/compatibility layers:

~~~
systemd-udevd   (optional compatibility backend)
Niri
libinput
wlroots
Noctalia
Ghostty
~~~

Отдельные `seatd`, `greetd`, Noctalia Greeter и `dbus-run-session` больше не входят в runtime chain и не должны возвращаться как обязательные daemon dependencies.

Следующий шаг — втягивать минимальные контракты внутрь Group C:

~~~
InputBackend
  ↓
SeatController
  ↓
GraphicsBackend
  ↓
минимальный SessionUI/compositor
~~~

После этого greetd/seatd/libinput/wlroots перестанут быть обязательной последовательной цепочкой.

## Reference material

OneFileLinux:
https://github.com/zhovner/OneFileLinux

Локальный минимальный анализ:

~~~
dist/sources/reference/onefilelinux-analysis/
~~~

Reference-код не копируется в Luna как собственная разработка.
