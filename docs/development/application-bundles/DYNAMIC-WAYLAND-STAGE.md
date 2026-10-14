# Динамический musl Bundle и Wayland — 2026-10-11

Ветка `luna-onefile-experiment`, HEAD `a1ea279`; без commits/reset/branch switch.
Продолжение [статического запуска](LAUNCH-STAGE.md) через те же App Manager,
App Runtime, security, mapping, namespace и System Runtime. PID 1 — `luna-init`.

## Реализованный subset

Registered directory-v1 `.lunaapp` сохраняет обычное имя, `manifest.toml`,
`integrity.toml` и `payload/`. `.lbp` остаётся транспортным архивом; его нет внутри
установленного приложения. Manifest grammar и ContentIdentity не менялись.

Installed planner поддерживает x86_64 little-endian ELF: прежний static subset
и dynamic musl с `/lib/ld-musl-x86_64.so.1`. Interpreter и каждый `DT_NEEDED`
рекурсивно разрешаются только через объявленные mappings проверенного payload.
Manager держит registry lock до exec; каждый источник покрыт тем же exact-content
Launch trust. В mappings требуются Read/Execute, а для каждого ELF node — отдельные
path grants Read и Execute. Directory grant не заменяет grants closure nodes.

Default search order: `/lib`, `/usr/local/lib`, `/usr/lib` внутри logical root.
Host loader/cache/environment и невыбранные System Image/DATA libraries не
используются. Любой `DT_RPATH`/`DT_RUNPATH`, включая пустой, отклоняется; также
запрещён mapping `/etc/ld-musl-x86_64.path` или покрывающего его subtree.
Повторные `PT_INTERP`/`PT_DYNAMIC`/`PT_PHDR` и search-path tags отклоняются;
`PT_INTERP` без `PT_DYNAMIC` запрещён. `DT_NULL` обязателен внутри объявленного
dynamic range. Полные диапазоны dynamic/string table и interpreter должны
переводиться через `PT_LOAD` в те же file offsets.

Для принятого x86_64 subset проверяются 4 KiB page-rounded LOAD mappings и BSS
zero ranges, а также `filesz <= memsz`, overflow и page congruence. ELF/program
headers должны оставаться отображёнными из проверенных bytes: короткие и разные
virtual aliases отклоняются. Единственный `PT_PHDR`, если он присутствует, должен
точно соответствовать offset/address/size загруженной таблицы. Это закрывает
расхождение между file parser и kernel/musl loader при подмене metadata page или
load bias. Generic parsing других architectures не обещает такой loader-equivalence;
installed adapter их не принимает. Механика подтверждена выбранным Linux source
`fs/binfmt_elf.c` и [исходником musl 1.2.6](https://raw.githubusercontent.com/ifduyue/musl/v1.2.6/ldso/dynlink.c).

Этот adapter не выбирает libraries из нескольких providers, не планирует
автоматически `dlopen` dependencies и не реализует glibc compatibility или
unresolved Bundle dependencies. Дополнительный payload доступен только при
явном объявлении mapping и соответствующих grants; broad subtree grant даёт
доступ ко всему выбранному subtree.

## Один Wayland endpoint

Application manifest может запросить `desktop.wayland`. Root-owned launch policy
для точных user/ID/version/ContentIdentity должна отдельно выдать capability:

```toml
# Внутри соответствующего [[application]], перед [[application.path]]:
capabilities = ["desktop.wayland"]

[[application.path]]
logical = "/run/wayland-0"
access = ["read"]
```

Также требуются execute/runtime/private-data и все payload/data/config grants.
Grant capability и socket Read проверяются `luna-security` до privileged probe.
Полная sealed plan authorization предшествует созданию private storage и namespace.

Broker выбирает единственный `wayland-<digits>` socket в приватном runtime directory
активного пользователя. Проверяются тип без symlink, owner, UID `SO_PEERCRED`
и точная cgroup server PID текущего desktop. Nonblocking connect ограничен 200 ms;
pending connection должна успешно завершиться, full backlog не считается успехом.

App Runtime сверяет proof с SessionId, credentials UID, approved capability/provider
и точным mapping. Secure bind закрепляет inode в `/run/wayland-0`. После materialization
child повторно сверяет device/inode и peer PID/UID, затем применяет chroot, Landlock
и credentials. Probe connections закрываются; приложение само подключается после
exec под UID пользователя. Root-connected socket FD ему не передаётся.

Environment: `WAYLAND_DISPLAY=/run/wayland-0`, `XDG_RUNTIME_DIR=/tmp` (собственный
runtime tmpfs приложения), без `WAYLAND_SOCKET`, `LD_PRELOAD` и `LD_LIBRARY_PATH`.
Absolute display path поддерживается [официальным Wayland client API](https://wayland.freedesktop.org/docs/html/apb.html).
Desktop runtime directory целиком и session/system D-Bus не монтируются.

Namespace требует прежние filesystem rights ABI3 **и ABI9 `ResolveUnix`** плюс
`AbstractUnixSocket` scope с HardRequirement. Socket rule получает только
`ResolveUnix`, по закреплённому O_PATH fd; directory/data/payload rules не получают
это право. Отказы к external named/abstract UNIX endpoints проверяются в QEMU.
Abstract scope не разделяет потомков, унаследовавших тот же Landlock domain.
Более старое ядро приводит
к отказу, ограничения не опускаются автоматически. Выбранный Linux 7.2.4 уже
содержит ABI9; kernel/input snapshot этим этапом не менялись.

`desktop.wayland` разрешает протокол выбранного compositor целиком. Это не
фильтр отдельных Wayland interfaces и не отдельная политика clipboard/screenshots.
GPU/device, audio, portal и другие providers требуют следующих resource handoffs.
Stdio пока `/dev/null`; интерактивный терминал ещё не реализован.

## Настоящее приложение для acceptance

`weston-simple-shm` из официального `weston-clients-14.0.2-r5.apk` (MIT).
Подпись проверена штатным `apk verify` с ключами Alpine. Exact APK SHA256:
`9b8ee28e3217c281244691c9496e86cc3eb1a4bbcd94c3109b8cc0fad0a5ccc1`.
Скрипт `tools/prepare-luna-wayland-fixture.py` не скачивает packages, проверяет
этот digest и canonical inputs, извлекает только выбранный executable и копирует
четыре regular ELF resources из проверенного provider snapshot:

```text
bin/weston-simple-shm
lib/ld-musl-x86_64.so.1
lib/libc.musl-x86_64.so.1
lib/libwayland-client.so.0
lib/libffi.so.8
```

Library aliases копируются как regular files: symlinks не включаются в payload.
Provenance содержит input lock и hashes каждого ELF. Transport pack → offline
install → registered `.lunaapp` выполняются общим CLI/manager, без специального
пути запуска Weston. Test fixtures, bootstrap и grants только в disposable layer.
Production desktop providers пока не преобразованы в полные application Bundles.

Повторяемая подготовка после отдельного signed APK verification:

```sh
python3 tools/prepare-luna-wayland-fixture.py \
  --inputs target/luna-app-launch-20261011/inputs-landlock \
  --apk target/luna-dynamic-apps-20261011/upstream/weston-clients-14.0.2-r5.apk \
  --output /absolute/path/new-wayland-fixture
LUNA_QEMU_DISPLAY=egl-headless \
LUNA_CANONICAL_WAYLAND_FIXTURE=/absolute/path/new-wayland-fixture \
  python3 tools/canonical-luna-workbench.py run build \
  --inputs target/luna-app-launch-20261011/inputs-landlock
```

Без переменной fixture canonical suite всё равно проверяет настоящий dynamic
musl probe и denial отсутствующего library grant. С ней также обязателен real
Weston window, отказ без display capability и cleanup графического приложения
на logout. Все working images проходят двухслотовый controller.

## Evidence

Финальное полное принятие latest-source QEMU: **PASS** в
`target/canonical-workbench/evidence/20261011-225619-427520/results.txt`,
EGL headless с VirGL. Пройдены build/structure/ownership, production/display,
16 Bundle markers и suite PASS, три desktop sessions, logout/reboot/shutdown.
Real Weston window, UID 1000, payload dynamic libraries, capability/library
denials, external named/abstract UNIX denials, logout cleanup и registry/data/config
persistence после reboot подтверждены отдельным `bundle-launch/results.txt`.
Niri JSON фиксирует настоящее окно; pixels не захватывались.

Controller принял B как новый reference, A сохранил для возврата:

```text
B SHA256 a512704c8070ae010c24241cd093461a9beabd5a36495ec19539fc717deee858
A SHA256 4c0c6459712b8b4f3c85d4740b4260fef8294d5e783a5c94e9a267ef40530a10
```

Host focused tests: **118 PASS**,
один privileged test намеренно ignored; scoped Clippy
app-runtime/system-runtime/CLI проходит с `-D warnings`. Отдельный Clippy для всего namespace
выявляет прежний `too_many_arguments` в неизменённом `volume_mount.rs:131`;
lint не отключён, этот результат не выдаётся за PASS. Namespace unit tests проходят.

Первый QEMU `20261011-184554-912184`: production/display и реальный dynamic musl
probe PASS, missing-library grant и Wayland capability deny PASS. GUI spawn
выявил `PathFd::new(socket)` ENXIO в native musl build; host reproduction подтверждён,
direct syscall O_PATH fd проходит. Harness также требовал отсутствующий `jq`;
теперь разбирает конкретный test-window PID стандартными provider tools.
Эталон сохранён, test overlay/build scratch автоматически удалены.

Второй QEMU `20261011-190128-154613`: все 16 Bundle checks PASS, три authenticated
UserSessions, real Weston window с UID 1000, payload musl/Wayland shared mappings,
denials к external named/abstract UNIX sockets, cleanup после logout и persistence
после reboot. Niri JSON зафиксировал PID/window/app_id; pixels не захватывались.
Production/display и три desktop smoke tests PASS. Общий controller остановлен
до promotion после независимого review ELF corner cases; прежний reference A
сохранён. Review замечания исправлены и покрыты отрицательными тестами; повторный
полный прогон использует frozen latest userspace. Эта частичная evidence не
объявляется полным canonical acceptance.

Latest userspace собран в `20261011-192344-220583`. Bundle checks, production,
display, три desktop sessions и logout PASS. Отдельный reboot case прерван:
первый reset произошёл до fixture request (sleep 30), без action marker; затем
дополнительные login/logout/poweroff и QMP `SHUTDOWN reason=host-ui` закрыли
control connection. Гость выполнил reboot/poweroff, но нужная последовательность
persistence checks не завершилась. Источник этого внепланового UI input не
устанавливался; это не доказательство нового сбоя guest reboot.

Тот же неизменённый candidate повторно проверен через `workbench run test`
в `target/canonical-workbench/evidence/20261011-224809-462732` с
`LUNA_QEMU_DISPLAY=egl-headless`. Гость дошёл до greeter; production gate выявил
разорванный serial marker `starting optional compatibility runlevel sysinit`:
форматированный вывод перемешался с Device Manager. Три boot diagnostics
(runlevel/readiness/revision) теперь формируют одну строку с newline перед
`write_all`; boot sequence и строгие test assertions не менялись.

Новая latest-source сборка и полный прогон завершены с PASS в
`target/canonical-workbench/evidence/20261011-225619-427520` с тем же EGL headless.
Остаются virtio-vga-gl/VirGL, настоящие Wayland/Noctalia rendering и keyboard
injection, без интерактивного host window. Boot diagnostics больше не разрывали
production/revision assertions; logout вернул greeter и второй desktop, reboot
сохранил HOME/config/state revision, shutdown подтвердил guest QMP event и PID1
poweroff. Все ранние partial/failure logs сохранены.

Final preservation: 350 baseline files/links, 328 byte-identical и 22 intended
edits, ничего не пропало и нет unexpected edits; три новых source/doc files.
Generated Graphify cache сохранён отдельно. Branch/HEAD не менялись; commits/reset
не было. Все 13 frozen code hashes совпадают с исходниками после прогона.
Selected/original input snapshots и kernel не менялись. Контрольные суммы образов,
очистка временных test disks/build scratch и проверки source/doc links сохраняются
в `target/luna-dynamic-apps-20261011/evidence/final-verification.json`.

## Следующие этапы

Authorized terminal/stdio handoff и интерактивное CLI-приложение; desktop discovery
registered apps; затем packaging настоящих Alpha/third-party apps и необходимые
GPU/audio/portal mappings. Live management IPC, signatures/glibc и journalled
legacy registry migration остаются отдельной работой.
