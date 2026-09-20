# Linux → Luna Boot Stack: gap analysis

Дата анализа: 2026-09-19

Документ отвечает на четыре вопроса:
1. Что реально происходит между UEFI и графической пользовательской сессией Linux.
2. Для чего нужен каждый слой и где находится его исходный код.
3. Что уже реализовано в Luna и чего не хватает для полноценного graphical boot path.
4. Какие части имеет смысл сделать Luna-owned Rust вместо переноса внешних daemon/runtime.

Цель не в полном клонировании Linux userspace. Luna использует Linux kernel как kernel,
а собственный userspace должен постепенно владеть системными механизмами над ним.
## 1. Канонический путь

    UEFI firmware
      ↓
    luna-boot.efi
      ↓
    ExitBootServices
      ↓
    Linux kernel
      ↓
    Luna kernel hook / setup_data
      ↓
    luna-init (PID 1)
      ↓
    logical root + /dev + /proc + /sys + /run
      ↓
    device/event manager
      ↓
    system runtime / service readiness
      ↓
    seat + DRM/KMS + input
      ↓
    authentication / UserSession
      ↓
    Wayland compositor
      ↓
    greeter / shell
      ↓
    desktop session

Linux сама по себе не требует systemd, initramfs, PAM, udev, logind или D-Bus.
Это отдельные userspace реализации механизмов. Luna может заменить их собственными
компонентами, сохранив kernel ABI и необходимые protocol ABI.
## 2. UEFI

UEFI загружает UEFI application, предоставляя Boot Services и System Table. OS loader
читает memory map, готовит kernel environment и вызывает ExitBootServices().
После успешного вызова Boot Services больше недоступны; ответственность за платформу
переходит к loader/OS. Источник нормы: UEFI Specification 2.10.

Luna status: реализовано.

boot/luna-boot — отдельный Rust UEFI application на crate uefi. Он проверяет
EFI/LUNA-SYS identity, выбирает target, загружает kernel/init, строит
LunaBootHandoffV1 и ведёт BootAttempt.

Для обычной Alpha загрузки нам не нужен GRUB/systemd-boot. Secure Boot, key enrollment,
capsule updates и глубокая UEFI Runtime Services интеграция относятся к production
hardening, но не являются первым GUI blocker.
## 3. Kernel handoff и early userspace

Kernel получает стандартный Linux x86 boot protocol и Luna-specific setup_data.
Наш kernel-side код валидирует handoff, проверяет адрес/размер init и передаёт управление
direct-PID1 пути.

Luna status: базовый механизм реализован.

kernel/rust/luna_boot.rs уже фиксирует handoff и memory-resident luna-init.
Serial boot показывает последовательность KernelHandoff → KernelStarted → InitStarted →
InitReady, после чего luna-init становится PID 1.

Обычный Linux стек часто использует initramfs для device discovery, storage, firmware,
LVM/RAID и подготовки root. Luna сознательно не использует старую initramfs/switch_root
цепочку. Вместо этого kernel → luna-init → System Image → logical root.

luna-init уже монтирует devtmpfs, proc, sysfs, devpts, tmpfs /run, /tmp и /dev/shm,
подключает DATA, материализует System Image и запускает luna-system-runtime.
## 4. Device model: devtmpfs ≠ udev

Здесь находится главная инфраструктурная дыра.

Devtmpfs создаёт kernel device nodes, но полноценная userspace device model обычно
добавляет обработку kernel uevents, чтение sysfs metadata, properties вроде ID_INPUT
и ID_SEAT, permissions/groups, aliases/symlinks, hotplug monitor и replay/settle.

Current Luna status:
- luna-device-manager существует как domain model и уже умеет напрямую обнаруживать input devices через sysfs;
- DeviceId, VolumeId, VolumeInfo и VolumeState уже определены;
- native kernel uevent stream ещё не подключён;
- текущий Alpha использует udev только как переходный metadata/provider layer для внешнего libinput/wlroots path;
- udev database не является целевым Luna requirement.

Reference source: systemd/src/udev и standalone eudev. eudev специально отделяет
udev от конкретной init system.

Целевой Rust backend:
    NETLINK_KOBJECT_UEVENT
          ↓
    Rust Uevent parser
          ↓
    Sysfs inspector
          ↓
    Device identity / properties
          ↓
    permissions + seat assignment
          ↓
    Luna device registry + event stream
## 5. Seat management

Seat management — отдельный механизм поверх device discovery. Он решает, какой
графический/input client получает доступ к shared devices без постоянного root.

seatd/libseat — компактная reference implementation. Upstream описывает его как
mediation доступа к graphics/input и поддерживает standalone operation.

Luna status: seatd service больше не запускается; графическая UserSession использует native Luna session ownership и `LIBSEAT_BACKEND=builtin` как переходный compatibility backend.
Это переходное решение.

Целевой вариант является внутренним UserSession/SeatController boundary: active seat,
ownership lifecycle и выдача ограниченного набора DRM/input resources. Compositor не
должен зависеть от отдельного seat daemon.

Внутренний boundary оформляется как Luna seat contract, без копирования API libseat.
## 6. DRM/KMS, input и Wayland

Для графической сессии нужны как минимум:
- DRM device discovery;
- KMS modesetting;
- buffer allocation/render path;
- input event handling;
- Wayland protocol server;
- compositor lifecycle.

wlroots и libinput рассматриваются как reference implementations для извлечения
минимальных contracts, а не как код, который необходимо переносить целиком.

Luna status:
- kernel DRM/KMS путь работает в QEMU;
- текущий внешний compositor path использует Niri/wlroots/libinput;
- внутри UserSession уже заложены native InputBackend, SeatController и DRM resource boundary;
- минимальный native GraphicsBackend и compositor ещё не завершены.

Целевая миграция: сохранить Linux DRM/input ABI и постепенно заменить providers
минимальными Luna-owned Rust modules. Mesa/GPU acceleration остаётся optional provider.
## 7. Authentication, UserSession и privilege setup

Linux-PAM предоставляет API для authentication, account management, session open/close
и password management. Исходники Linux-PAM открыты.

Luna status:
- luna-user-session владеет UserSession state machine;
- UserSession является единой Group C boundary для auth, credentials, seat,
  input, graphics, compositor и session UI;
- native InputBackend, SeatController и DRM resource boundary уже заложены;
- native authentication boundary уже выполняется внутри `luna-user-session` для выбранной Alpha identity;
- native `UserCredentials` выполняет credential transition внутри `luna-user-session`; отдельный внешний privilege utility не требуется.

Интерактивный ввод credentials и полноценная account/password policy остаются следующим native SessionUI/auth слоем.

greetd полезен только как historical/reference source для сравнения auth/session contracts и больше не входит в runtime chain.

Для Luna-owned stack нужно реализовать только необходимый session contract:
- credential lookup;
- password verification policy;
- account state;
- supplementary groups;
- setuid/setgid/setresuid/setresgid;
- HOME/USER/SHELL/XDG_RUNTIME_DIR;
- environment sanitization;
- final exec boundary.

Это должно жить внутри UserSession, а не становиться отдельным auth/credential daemon.

Privilege-management utility больше не является runtime dependency: credential transition реализован native-кодом Luna.
## 8. Service supervisor и IPC

Systemd/OpenRC/dinit нужны не потому, что Linux требует конкретный init, а потому что
системе нужны lifecycle, dependency ordering, readiness, restart policy и shutdown ordering.

Luna status: luna-system-runtime уже является главным long-lived supervisor. Это правильно
с точки зрения принятой архитектуры, но readiness пока частично ad-hoc: sockets проверяются
только для отдельных сервисов, а общего dependency graph ещё нет.

Следующее расширение именно luna-system-runtime:
- service definition;
- dependency DAG;
- readiness contract;
- restart policy;
- failure domain;
- ordered shutdown.

D-Bus для загрузки Linux не обязателен. В desktop path он используется внешними services.
Luna должна предпочитать собственный versioned Unix-socket IPC и оставить D-Bus только как
ограниченный compatibility provider там, где он нужен внешнему desktop stack.
## 9. Что реально блокирует полноценный GUI boot

Alpha blockers:
1. Native device/event path ещё использует optional udev compatibility metadata для полного provider compatibility.
2. Native Luna UserSession владеет seat/session boundary; внешний seatd service не требуется.
3. libinput требует userspace device metadata; одного devtmpfs недостаточно.
4. Интерактивный credential entry и полноценная password/account policy ещё не закрыты native SessionUI.
5. Privilege transition выполняется native `UserCredentials` внутри `luna-user-session`.

Уже работающие фундаментальные слои:
- UEFI → luna-boot.efi;
- Luna handoff через Linux setup_data;
- direct-PID1 luna-init;
- devtmpfs/proc/sysfs/devpts/tmpfs;
- System Image materialization;
- DATA attachment;
- DRM node preparation;
- luna-system-runtime supervision;
- System Image/PC image packaging.

Production-only follow-ups: cgroups v2 policy, robust shutdown, user switching,
suspend/resume, hotplug removal, secure auth storage, portals, audio/media,
network/Bluetooth/removable media, GPU acceleration, Secure Boot/measured boot.
## 10. Rust migration plan

Recommended order:

    1. luna-device-manager backend
       uevent + sysfs + registry

    2. Luna seat authority
       DRM/input ownership

    3. Luna credentials
       user/group/session setup

    4. Native SessionUI authentication completion
       credential entry + password/account policy

    5. Luna service graph
       dependencies/readiness/restart

    6. Luna IPC
       internal Unix socket protocol

    7. Luna compositor
       DRM/KMS + input + Wayland

    8. Luna shell/session UI
       replace remaining external shell/provider pieces

Это позволяет каждый раз заменять один system mechanism, не ломая весь boot path.
Внешние providers остаются временными compatibility layers и не становятся частью SoT.
## 11. Reference source pack

Исходники для разбора скачаны локально в dist/sources/reference/ и не являются
runtime dependencies.

systemd — udev architecture/reference
 e96ff3b5b92dc06ed6f623eaa0c44e97e1650dbb

eudev — standalone udev implementation
 aa49f5cc7e3959b297b9355c72067776d238d5d0

linux-pam — authentication/session model
 9330e6b7a8e533ecb12ee321ff800f17bac30835

seatd — seat ownership
 427b5d956afb589c4c8a1612c75644a9254d2051

libinput — input device behavior
 5cf833b572f5a33663888da321dce0ba1abd82a2

util-linux — исторические low-level utility references
 55e3c4dba6101a47ecaae71c91edbe10c59b8d56

Основные desktop/reference sources уже есть в dist/sources: Niri, Noctalia, Noctalia Greeter,
greetd, Wayland, Wayland protocols, wlroots и PipeWire/WirePlumber.

Для minimal baseline дополнительно зафиксирован OneFileLinux analysis:
dist/sources/reference/onefilelinux-analysis/.
## 12. Итог

Luna уже имеет собственный bootloader, kernel handoff, direct-PID1 init, immutable
System Image, DATA model и system runtime.

Продолжать путь «добавим ещё Linux packages» неправильно. Для самостоятельной ОС
нужно переносить выявленные механизмы в Luna-owned Rust contracts:

    kernel primitive
       ↓
    Rust system component
       ↓
    Luna protocol/state
       ↓
    desktop provider

Первый приоритет — сделать полноценный Rust backend для luna-device-manager. После него
наиболее важные границы — seat, credentials/authentication и privilege transition.
Только после стабилизации этих слоёв имеет смысл заменять wlroots/libinput desktop stack.

### Внешние источники

- UEFI Specification 2.10: https://uefi.org/specs/UEFI/2.10/
- Linux kernel documentation: https://docs.kernel.org/
- systemd/udev: https://github.com/systemd/systemd
- eudev: https://github.com/eudev-project/eudev
- libinput: https://gitlab.freedesktop.org/libinput/libinput
- seatd/libseat: https://github.com/kennylevinsen/seatd
- Linux-PAM: https://github.com/linux-pam/linux-pam
- greetd: https://github.com/kennylevinsen/greetd
- wlroots: https://gitlab.freedesktop.org/wlroots/wlroots
- util-linux: https://github.com/util-linux/util-linux
