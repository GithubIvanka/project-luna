# Alpine Foundation — состояние для продолжения разработки

Документация обновлена: 2026-10-14. Последняя полная приёмка: 2026-10-14.
Основная локальная ветка после объединения: `develop`.
Проверенный код Alpha: `90612b8ead7ecdac257b186861e37e9acb536eff`.
Исторический исходный HEAD этого этапа: `a1ea279` (`luna-onefile-experiment`).
Рабочий каталог: `/home/sibitti/source/project-luna`.

## 2026-10-14: аудит оформления и подготовка объединения

Текущее состояние и рабочий план — в
[отчёте аудита](../AUDIT-AND-CONSOLIDATION-2026-10-14.md).
Принятый дизайн — [SPEC](../../design/luna-alpha-20261013/SPEC.md),
перенос в настоящую ОС — [IMPLEMENTATION](../../design/luna-alpha-20261013/IMPLEMENTATION.md).
Полная приёмка `20261014-133032-138118` прошла; эталон A `9728cdec…`,
резерв B `37773741…`. GUI выбора/чтения файла и ограничение прав приняты.
Ручной ввод после resize GTK и начальное разрешение просмотра ещё требуют проверки.

Пользователь прямо разрешил объединить работу в `develop` и продолжать там.
Прежние записи ниже о запрете коммитов и сохранении экспериментальной ветки
описывают условия своих этапов; для текущего объединения действуют новый запрос
и обновлённый AGENTS. Старые ветки и чужие исходники сохраняются.
Сервис памяти сейчас недоступен; рабочий отчёт остаётся в репозитории.

## 2026-10-13: язык документации и комментариев

Документация, пояснения и комментарии в собственном коде Luna — на русском,
по прямому указанию пользователя. Правило закреплено в `AGENTS.md`.
Переведены последние отчёты и комментарии Files, добавлены пояснения сложной
логики. Исполняемый код и принятые образы не меняются; новая сборка/QEMU
для перевода не запускались. Хеши исходников в прежних отчётах и графе
относятся к снимку до перевода комментариев.

## 2026-10-12: принятые файловые операции

Приняты графические копирование/вырезание/вставка/переименование, корзина,
прогресс, отмена и клавиатурное выделение на закреплённом движке Yazi Local.
Открытые объекты закрепляются дескрипторами; окончательное перемещение
не заменяет назначение. Перенос между разными монтированиями отказывает
без удаления источника. Корзина хранит содержимое и исходный путь/время;
ручной возврат работает. Доступ ограничен HOME. Открытие другим приложением
и восстановление по исходному пути — следующие задачи.

SDK std::File удалял O_PATH, вызывая EACCES при закреплении корня внутри
изоляции. Анализ ELF и скриншот Niri подтвердили причину. Прямой SYS_openat
исправил её без расширения доступа к корню или /proc. Проверки F_GETFL
и файлового движка в настоящем SDK теперь обязательны для обычной сборки.
Исправлены клавиатурное выделение GTK, ввод точки/заглавных букв помощником
и ложные отказы разбора перемежающихся сообщений OpenRC.
Все исходники и пользовательские изменения сохранены; ветка/HEAD прежние,
коммитов и сброса изменений не было.

Полный канонический прогон **PASS**, `20261012-220339-357404`:
файловые действия/отмена, меню Noctalia, Ghostty/fish/Bash/sh/Yazi,
три сессии рабочего стола, выход/повторный вход, перезагрузка/выключение
и предыдущие проверки бандлов. Эталон A:
`391e18e4ffd240d81dc14da0567aeec0960307a8da18f098be1a7626adc583c5`;
предыдущий B сохранён:
`8a31147e9a9526f28f6be3c2b67b18195ba35389e7edfb08ba9703f25c903c2e`.
Входной снимок: `target/luna-files-operations-20261012/inputs-final`, 8517 объектов.
30 тестов движка на musl хоста и настоящем SDK, 3 вспомогательных теста GUI,
18 тестов пространств имён, 15 проверок упаковки, 5 проверок разбора сервисов
и 3 клавиатуры; Clippy в области изменений и независимый просмотр завершены.
Граф: 147 файлов, 3145 узлов, 8479 связей, 132 группы, без вызовов LLM.
[Подробности и ограничения](../application-bundles/FILES-OPERATIONS-STAGE.md).

## 2026-10-12: каталог зарегистрированных приложений

Принят общий ограниченный каталог рабочего стола через существующий посредник
запуска с проверкой отправителя. Менеджер проверяет запись о регистрации,
раскладку файлов и ContentIdentity; общий планировщик AppRuntime — собственный
ELF, ресурсы, аргументы и политику пользователя. Получение списка не проверяет
сокет композитора, не создаёт хранилище и не выдаёт пригодный для запуска план.
Временные записи XDG принадлежат root, создаются до Noctalia и обновляются
явной командой `luna app list`. Фиксированные записи начального набора убраны;
меню использует понятные имена TOML и запуск CLI с закреплённым содержимым.
Noctalia сохраняет обычный сканер записей системы/пользователя; меню не решает
вопросы доступа.

Необязательное неизменяемое `entry.args` передаёт аргументы по умолчанию
перед аргументами вызова. Пустое поле сохраняет прежнее представление манифеста
и ContentIdentity. Настройки Ghostty задаются манифестом; каталог не знает
специальных идентификаторов приложений. Mod+Space открывает меню запуска.
Управление установкой через IPC, собственные значки, «Показать содержимое»
и доступ из изолированного Terminal к посреднику ещё не добавлены.

Настоящее меню Noctalia и виртуальная клавиатура запустили Files, Terminal
и отдельно зарегистрированный тестовый Weston. Навигация/создание папки Files,
сопоставление PID с экземпляром (выход 0, без сигнала, успешная очистка),
fish/Bash/sh/Yazi, приватный PTY и сохранение данных после выхода/перезагрузки
проходят. Другой UID или процесс вне нужной cgroup получает отказ;
записи без разрешений и консольные приложения скрыты. Устаревший ожидаемый
хеш отклоняется до создания хранилища.

Первый помощник ошибочно выбирал wayland-0; настоящий сокет и исходники
подтвердили причину. Выбирается единственная пара Wayland/Noctalia.
Повторный `run test` использовал тот же образ без новой сборки.
Полный канонический прогон **PASS**:
`target/canonical-workbench/evidence/20261012-084226-226038/results.txt`,
включая загрузку/видеопереход, три сессии, выход/повторный вход,
перезагрузку/выключение и прежние проверки бандлов.
Эталон B того этапа:
`8a31147e9a9526f28f6be3c2b67b18195ba35389e7edfb08ba9703f25c903c2e`;
прежний A сохранён:
`f60172474267f266e971d0a67b415bc0a88501c3db74dffcde70ab3669d0953f`.
Входной снимок: `target/luna-desktop-discovery-20261012/inputs`.
132 теста musl, 13 проверок упаковки/сценариев, Clippy в области изменений
и независимый просмотр PASS; канонические 29 тестов состояния/runtime,
119 app-runtime, 4 init. Один привилегированный тест пропущен, настоящая
настройка процесса проверена QEMU. Временные диски и сборочные каталоги очищены.
361 исходный путь сохранён, ветка/HEAD прежние, коммитов/сброса не было.
[Подробности](../application-bundles/DESKTOP-DISCOVERY-STAGE.md).
Следующей задачей тогда были безопасные операции Files; они приняты выше.

## 2026-10-12: графический Files и интерактивный терминал

Принят начальный самостоятельный GUI на GTK с настоящим закреплённым
движком Yazi `yazi-fs` Local: навигация, список/метаданные, Home/Up/Refresh,
адрес, создание папки, --reveal. Это часть файлового движка, а не полный
Scheduler или готовый сервер Yazi без интерфейса. Файловые изменения
на этом этапе ещё отсутствовали; реализованы в следующем принятом этапе.
Ghostty 1.3.1, fish 4.6.0, настоящий Bash 5.3.9, BusyBox POSIX sh и Yazi 26.9.1
собраны и упакованы на native musl. Системный sh остаётся BusyBox ash.

`luna-files.lbp` / `terminal.lbp` устанавливаются App Manager в directory-v1
`.lunaapp`: метаданные TOML, обычные файлы полезной нагрузки, без вложенного
архива. Начальные записи меню и Niri Mod+E/Mod+Return используют существующий
`luna app run`. Разрешения Launch на точное содержимое, явные `user.home`
и приватный `terminal.pty` проходят через существующие runtime, namespace,
security и cgroup; новых фоновых сервисов нет. Общий каталог реализован
в следующем этапе, описанном выше.

Проверки настоящего GUI в QEMU прошли: навигация и создание папки в Files,
выход 0; Ghostty принимает ввод в fish, Bash/sh выполняют команды,
интерактивный Yazi работает на том же приватном devpts и после `q` возвращается
в оболочку. Идентификатор окна/PID Niri, UID 1000, общее пространство
монтирования/cgroup, устройство/номер объекта PTY, очистка потомков при выходе,
сохранение HOME и реестра/данных/настроек после перезагрузки подтверждены.

Полный канонический прогон **PASS**:
`target/canonical-workbench/evidence/20261012-073541-834991/results.txt`:
16 прежних отметок проверки бандлов, загрузка/видеопереход, три сессии,
выход/повторный вход и перезагрузка/выключение гостевой системы.
Эталон A того этапа:
`f60172474267f266e971d0a67b415bc0a88501c3db74dffcde70ab3669d0953f`;
предыдущий B сохранён:
`a512704c8070ae010c24241cd093461a9beabd5a36495ec19539fc717deee858`.
Входной снимок: `target/luna-terminal-files-20261011/inputs-gui-terminal`.
Временные диски/сборочные каталоги очищены. Итоговые пакеты, сведения
о происхождении, SDK/кэш и результаты сохранены; лишние диагностические
копии данных/архивов удалены после принятия. 354 исходных пути сверены,
ни один не потерян; ветка/HEAD прежние, коммитов/сброса не было.

Исправлены подтверждённые дефекты: DATA 1GiB не вмещала пакеты и mkfs.btrfs
увеличивал файловую систему за GPT; отсутствовали ресурсы XKB/Compose GTK;
без сессионного D-Bus Files/Ghostty теряли app_id. Сборщик измеряет DATA
и проверяет размер, упаковщик добавляет ресурсы, Files задаёт prgname,
Ghostty получает документированную поправку Wayland. Запись о сборке
связывает поправку, накладку, SDK, базу пакетов, ревизии и двоичные файлы
до упаковки. Очистка данных выполняется и при обычной ошибке упаковки;
закреплённый дескриптор защищает очистку сборки от подмены ссылкой.
Тесты/Clippy в области изменений и независимый просмотр завершены.
[Версии, команды, ограничения и результаты](../application-bundles/TERMINAL-FILES-STAGE.md).
Ниже сохранены исторические записи; их незавершённые задачи относятся
к прежнему состоянию, если не подтверждены более новыми этапами выше.

## 2026-10-11 installed Bundle launch checkpoint

Текущий `.lunaapp` теперь запускается в active authenticated UserSession через
существующие App Manager → App Runtime → root-mapping/security/namespace →
System Runtime supervisor. Exact-content Launch grant независим от Install;
caller определяется по SO_PEERCRED и точной desktop cgroup. Принят первый
static x86_64/musl subset: payload read/execute, отдельные persistent application
data/config, credentials пользователя, Landlock, cgroup descendants и exit status.
PID 1 остаётся `luna-init`; новый supervisor/daemon не добавлен.

Реальный Bundle acceptance в QEMU прошёл **11 сценариев/markers и три sessions**,
включая установленный Luna CLI, denied callers/content, изоляцию, exit/signal,
fork/setsid cleanup, logout/relogin и registry/data/config после guest reboot.
Evidence: `target/canonical-workbench/evidence/20261011-175953-392615/bundle-launch/results.txt`.
Полный canonical suite: **PASS**, включая свежую сборку, structure/ownership,
production/display, три независимые 60-second desktop sessions и отдельные
logout/reboot/shutdown. Новый эталон — slot `a`, SHA256
`4c0c6459712b8b4f3c85d4740b4260fef8294d5e783a5c94e9a267ef40530a10`;
slot `b` сохраняет прежний эталон. Accepted inputs:
`target/luna-app-launch-20261011/inputs-landlock`. Временные test disks и build
scratch очищены, evidence остаются отдельно. Итоги и границы:
[LAUNCH-STAGE.md](../application-bundles/LAUNCH-STAGE.md).

Проверки выявили две реальные причины отказа: DATA inode metadata обходили
fakeroot через libc nftw; в выбранном старом kernel отсутствовал Landlock.
Builder получил build-only bridge + независимый raw inode gate. Новое ядро
построено из того же recipe с Landlock/SECURITY_PATH; прежние kernel/provider
snapshot и эталон сохранены. `inputs-landlock` закрепляет новый kernel/config.
Host harness использует fakeroot при restore metadata; rejected peer responses
доставляются после bounded input drain, с отдельным denied-client budget.

90 focused musl tests, 25 state/system runtime, 4 init, 26 artifact checks и
6 negative input checks проходят; privileged unit test отдельно ignored,
его фактический process setup проверен QEMU. Scoped Clippy и independent read-only
review завершены. Полная GUI/terminal integration и dynamic musl dependency/resource
closure, desktop discovery, реальные Alpha/third-party packages, live management
IPC, signatures/glibc и legacy registry migration остаются следующими этапами.
Ниже сохранены предыдущие checkpoint; их pending launch statements исторические.

## 2026-10-11 storage and two-slot workflow checkpoint

По запросу пользователя принята ротация двух образов разработки: эталон и
эксперимент/предыдущий эталон. Реализация и команды:
[ARTIFACT-WORKFLOW.md](ARTIFACT-WORKFLOW.md). `AGENTS.md` направляет дальнейшие
canonical builds через этот controller.

Освобождено **97,08 GiB**: 58 проверенных generated build trees/test disks
(82,83 GiB) и шесть obsolete source raw images (14,25 GiB). Удаление образов
выполнено после полного принятия нового эталона, под блокировкой, со сверкой SHA.
Входные snapshots, serial/QMP/build logs, Graphify, первоначальный пользовательский
образ и все пользовательские source/harness files сохранены. Evidence:
`target/luna-storage-audit-20261011/{cleanup-result,obsolete-image-cleanup-result}.json`.
`target` теперь около 20 GiB, свободно около 98 GiB. Рабочие slots занимают
2,5 GiB allocated, evidence — 13 MiB; первоначальный пользовательский образ
`target/luna-canonical-noctalia-20261010/luna-0.1.0.img` вне автоматической ротации.

В `target/canonical-workbench` импортирован последний принятый regression-final
образ. Resource experiment заменил тот же login font через новый путь: затронуто
262144 байта SYS, без полной сборки DATA/raw image. 24 focused safety/rotation
checks PASS: symlinks/navigation/devices, failed acceptance, successive role swaps,
init recovery, unknown-file preservation, SIGKILL lock inheritance, ранние
setup failures, ошибочный QEMU exit=0 и input snapshot/lock mismatch.
Независимый read-only review завершён;
выявленные замечания исправлены.

Первый live suite `evidence/20261011-121350-448289` прошёл production/display и
три desktop login cases, но остановился до lifecycle: agent изменил выполняемый
Bash script. Эталон не был продвинут. Последующая диагностика исправила marker
инъекции lifecycle helper, qemu-io filename и выбор WAYLAND_DISPLAY по реальным
Wayland/Noctalia sockets. Финальный стабильный `run test` использовал тот же
эксперимент без rebuild: **полный PASS**, evidence
`target/canonical-workbench/evidence/20261011-123539-844060/results.txt`.
Production/display, три desktop login, logout → повторный native login/desktop,
настоящие guest reboot/shutdown, HOME/config и state revision проверены.
Новый эталон — slot `b` (SHA256 `e5794bd66bc5dbda01743b72f5fbecf9d7dae2ff2012b2d13de46859e5abd6f5`);
slot `a` хранит прежний эталон и переиспользуется для следующего эксперимента.
Из 339 исходных файлов baseline 328 byte-identical, только 11 согласованных
script/docs files изменены; отсутствующих или неожиданных изменений нет.
`target/luna-storage-audit-20261011/source-preservation.json` фиксирует сверку.

Текущий Bundle launch stage содержит незавершённые uncommitted process-host,
installed static-ELF launch plan, per-user authorization broker и CLI run.
Host compilation и state/runtime/init tests проходят после освобождения места.
Реальный Bundle launch в активной UserSession ещё не принят: QEMU storage test
использует прежний принятый userspace image. До принятия этого этапа нельзя
заявлять, что сторонние Bundles уже запускаются в текущем desktop.

## 2026-10-11 application Bundle checkpoint

The installed application representation now uses ordinary filenames such as
`luna-cli.lunaapp`, with canonical `manifest.toml`, `integrity.toml` and `payload/`.
ID/version are metadata; no transport `.lbp` is embedded. App Manager keeps its
receipt outside the package in opaque `.bundles-v2` registration slots. Existing
legacy `.bundles` is preserved and requires a separate explicit migration.

Verified: 54 Rust tests, Clippy `--all-targets -D warnings`, runtime compilation,
24 offline acceptance scenarios + COMPLETE, final independent static review.
Directory identity survives rename and removal of the input transport archive.
Evidence: `target/luna-bundles-20261011/`; current status:
[`DIRECTORY-STAGE.md`](../application-bundles/DIRECTORY-STAGE.md).

Next: verified Bundle launch in a real active UserSession through existing
app-runtime, RuntimeSpec/ELF closure, authorized mappings, persistent data/config
and exit status; then canonical QEMU and Alpha/third-party packaging. This stage
did not run QEMU or change boot/init/desktop scripts. All work remains uncommitted.

## Active checkpoint — canonical desktop regression audit

Current changes are uncommitted. The original 19 modified files and untracked
test harness were inspected and preserved; do not reset or switch this checkout.
The comparison reference is `alpine-foundation` at `1e5ece3`, inspected without
checking it out. Full findings: [GRAPHICS-BRANCH-AUDIT.md](GRAPHICS-BRANCH-AUDIT.md).

The complete canonical image now restores isolated Mesa VirGL, system fonts,
Noctalia data/dependencies and provider policies. System Runtime registers the
desktop process with UserSession, so logout returns to native login. The
elogind power path has the expected FHS aliases, service accounts and scoped
Polkit policy; PID 1 handles the final shutdown/reboot signals and syscall.
QEMU uses `virtio-vga-gl` to preserve its firmware display surface. The user
confirmed that the reported monitor-disconnected message occurred in QEMU.

Current production path:

`UEFI -> luna-boot -> kernel -> luna-init PID 1 -> immutable System Image + DATA -> luna-system-runtime -> required Luna manager readiness + boot success -> optional compatibility providers -> native login -> authenticated Niri/Noctalia`

Artifact/evidence root: `target/luna-foundation-audit-20261010/`.
The ordinary image is `image-complete/luna-0.1.0.img`; lifecycle test images are
separate and inject session actions only for acceptance. Source image checksums
are checked after the tests. No physical disk was written and no commit created.

Recheck with `tools/test-canonical-luna-disk-image.sh`, production/login OVMF
tests, `tools/test-canonical-luna-lifecycle-ovmf.sh logout|shutdown|reboot`, and
`tools/test-canonical-luna-boot-display-ovmf.sh`. Use the audit graphics/fonts
roots when rebuilding from this host's prepared artifacts.

Remaining Alpha work includes DNS/network integration, real audio and hardware
acceptance, native seat/login1 lifecycle, missing baseline applications and
installer/recovery acceptance. The old foundation DATA schema still omits the
architecturally required `system/state` and `system/volumes`; this is tracked
debt, not an accepted replacement schema.

The dated entries below are historical. In particular, OpenRC `[ok]`, a
Noctalia PID and the old inverted grep assertions did not prove functional
desktop/power integration. The audit's stricter fresh tests supersede them.

## Current objective

Use Alpine Linux 3.24.2 x86_64/musl as a known-good userspace while progressively replacing Alpine boot responsibilities with Luna components.

Immediate validation path:

`UEFI -> luna-boot -> Luna kernel -> luna-init -> canonical LUNA-SYS/LUNA-DATA -> luna-system-runtime -> Luna manager gate -> optional compatibility providers -> native Luna login -> Niri/Noctalia`

Direct-root remains a separate control experiment and is not the canonical runtime path. Do not move this design to `develop` automatically.

## 2026-10-02 canonical Luna graphical foundation — historical reported result

The current canonical disk-image experiment now reaches a complete graphical user-session lifecycle without the direct-root Alpine launcher.

Verified artifact:
- `dist/luna-0.1.0.img`
- GPT remains `EFI / LUNA-SYS / LUNA-DATA / SWAP`.
- LUNA-SYS remains ext4; LUNA-DATA remains Btrfs.
- Canonical LUNA-DATA top-level schema is `cach / system / users`; legacy `state`, `volumes`, and `system/state` / `system/volumes` are absent.
- Structural verification: `Canonical Luna disk verification: PASS`.

Verified boot/session path:
`UEFI -> luna-boot -> Luna kernel 7.2.4 -> luna-init PID 1 -> System Image + physical LUNA-DATA -> luna-system-runtime -> OpenRC -> native Luna login -> luna-graphical-session -> user D-Bus -> PipeWire/WirePlumber -> Niri -> Noctalia`.

Verified OpenRC lifecycle:
- OpenRC 0.63.2 starts the sysinit, boot, and default runlevels.
- `seatd`, system D-Bus, elogind, BlueZ, NetworkManager, Polkit, rtkit, and UPower all reach OpenRC `[ ok ]` startup.
- `luna-system-runtime: boot success confirmed` is emitted after the runlevel sequence completes.
- OpenRC owns the system daemon lifecycle; the native Luna UserSession remains the single login/session entry point.

Verified graphical login:
- Native login renderer loads the Luna font and `docs/design/visual-concepts/luna.png`.
- DRM master and QEMU KMS target are acquired at 1280x800@75Hz.
- `luna/luna` authentication succeeds.
- `luna-user-session` launches `/usr/bin/luna-graphical-session`.

Verified desktop session:
- The session launcher creates a real per-user D-Bus session; `dbus-send --session` now reports the bus as ready.
- PipeWire and WirePlumber start inside the user session.
- Niri creates the Wayland display.
- Noctalia is observed as a running process from the Niri startup configuration.
- Niri receives `/dev/input/event0`, `event1`, and `event2` through libseat.
- Production serial confirms libinput creates actual devices: `New device "event1"` and `New device "event2"`. This closes the previous input-discovery blocker.

Regression results:
- `tools/test-canonical-luna-disk-image.sh`: PASS.
- `tools/test-canonical-luna-login-ovmf.sh`: PASS.
- `tools/test-canonical-luna-production-ovmf.sh`: PASS.
- Production image checksum verification: PASS.

Current scope boundary:
- This milestone intentionally keeps Alpine/OpenRC, eudev, libseat/seatd, Niri, Noctalia, PipeWire, WirePlumber and other external providers in place so the working graphical path can be stabilized before replacement work.
- The old direct system-service spawning function remains in source as an unused transitional path and should be removed during the next cleanup phase rather than being used at runtime.
- The next phase is hardening and dependency cleanup: remove redundant transitional plumbing, make provider closures more reproducible, then replace selected external responsibilities with Luna-owned implementations one component at a time.

## Verified

- luna-boot boots Alpine without GRUB.
- Luna kernel 7.2.4 boots Alpine.
- `luna_direct_root=1` mounts the selected EXT4 root directly on `/`.
- Direct-root reaches `/sbin/init` and OpenRC with no actual initramfs/switch_root/chroot/pivot_root.
- EFI GOP stride handoff is fixed: 1280x800x32 now reports Linux `lfb_linelength=5120`.
- Alpine login works after fixing test-root account preparation.
- Test credentials: `luna/luna` and `root/root`.

## Alpine login finding

Stock Alpine initramfs contains `/etc/passwd` and `/etc/group`, but not `/etc/shadow`. Normal authentication occurs after the real rootfs is active.

Reference BusyBox is built with `CONFIG_FEATURE_SHADOWPASSWDS=y`; PAM is disabled. BusyBox login reads shadow credentials from the real rootfs.

Our failed login was caused by our rootfs shadow preparation, not by missing initramfs functionality. The helper now creates proper 9-field shadow records, generates SHA-512 hashes through Alpine BusyBox `mkpasswd`, and validates them.

Important Alpine responsibilities:
- `mkinitfs/initramfs-init.in`: console/getty setup and boot transition.
- `alpine-conf/setup-alpine.in`: root password setup, then `setup-user`.
- `alpine-conf/setup-user.in`: `adduser`, password setup, groups.
- BusyBox `login.c` / `libbb/correct_password.c`: actual authentication.
## Niri state

Native Alpine `niri-25.11-r2.apk` is in `dist/.build/alpine-niri-apk-cache/`. Cache: 85 APKs, about 105 MB.

Niri runtime needs musl, cairo, libdisplay-info, GBM, libgcc, GLib/GObject, libinput, Pango/PangoCairo, PipeWire, pixman, libseat, libudev and xkbcommon.

Kernel prerequisites already present:
- `CONFIG_DRM=y`
- `CONFIG_DRM_VIRTIO_GPU=y`
- `CONFIG_INPUT_EVDEV=y`

`niri-session` falls back to `niri --session` when systemd/dinit are absent.

The direct-root launcher accepts `LUNA_ALPINE_NIRI=1`. It extracts the cached native runtime, adds Alpine seatd OpenRC files, creates the `seat` group, prepares `/run/user/1000`, exports `XDG_RUNTIME_DIR`, and starts QEMU with `-vga none -device virtio-gpu-pci`.

Offline apk installation was tested. A local index resolves the cached set, but package hooks on this host fail with `unshare: Operation not permitted`. Therefore direct APK extraction is currently the reliable image-construction method; do not claim package hooks ran.

The cache also contains `eudev-libs`, while the base root already has `libudev-zero`. APK solving exposed these as conflicting implementations. Do not replace libudev-zero unless the niri test proves it necessary.

## Current blocker / next action

One niri-enabled image build once failed during LUNA-SYS `mkfs.ext4` with an apparent `libLLVM.so.22.1` allocation message. Manual reproduction using the same `system-root` now succeeds, so rerun after cleaning the output before changing architecture.

After successful boot:
1. login `luna/luna`;
2. check `/dev/dri`;
3. verify seatd and `seat` group access;
4. verify `/run/user/1000`;
5. run `niri --version`;
6. run `niri --session` or `niri-session`;
7. record exact DRM/seat/Wayland errors.

Do not add a display manager, GNOME portals, a full desktop stack, or the old glibc niri build yet.
## Files of record

Direct-root launcher:
`components/boot/tests/ovmf/run-alpine-luna-kernel-direct-root.sh`

Login helper:
`components/boot/tests/ovmf/configure-alpine-test-user.sh`

Direct-root kernel overlay:
`tools/apply-luna-direct-root-overlay.py`

Alpine reference tree:
`dist/sources/reference/alpine-3.24/`

Niri cache:
`dist/.build/alpine-niri-apk-cache/`

Direct-root output:
`components/boot/tests/ovmf/out-alpine-luna-kernel-direct-root/`

Successful serial log:
`components/boot/tests/ovmf/out-alpine-luna-kernel-direct-root/serial.log`

## Continuity rule

At each meaningful milestone update this file with verified results, exact current state, blocker, next concrete action, important paths/commands, and Alpine-to-Luna responsibility discoveries.

A new chat should read this file before modifying the Alpine experiment.

## 2026-09-24 status update

- Login path remains verified: `luna/luna` reaches a shell; `root/root` is also prepared for debug access.
- Native Alpine niri runtime is now materially integrated into the test root when `LUNA_ALPINE_NIRI=1`: `niri`, `niri-session`, `seatd`, cached runtime libraries, seat group, runtime directory setup, and a minimal niri config are present.
- Added `wayland-libs-server-1.25.0-r0.apk` to the project cache after the first niri test exposed that the compositor needs the Wayland server library at runtime.
- Added Luna kernel `modules.builtin` metadata to the rootfs when available; this avoids false module-probing failures for built-in DRM devices. The earlier `modprobe: can't change directory to 7.2.4` messages are no longer present in the latest boot log.
- QEMU niri mode now uses `virtio-gpu-gl-pci` with `gtk,gl=on`. This gets the DRM-capable guest GPU configured, but the host QEMU/GTK path currently emits repeated `Gdk-WARNING: eglMakeCurrent failed`. This is a host display-backend issue to resolve before treating guest rendering failures as niri failures.
- The latest GUI boot reaches the normal Alpine login prompt; QEMU is currently not meant to be left running between sessions.
- Previous manual niri launch reached niri itself and proved the binary/runtime entry point works (`niri 25.11`), but the first temporary test config had invalid KDL and the compositor then reported `NoWaylandLib`. A corrected minimal KDL config is now generated by the launcher, and the next test should use the fresh image rather than the old hand-edited config.

### Immediate next action

Run the fresh niri-enabled image with a display configuration that is known to work on this host. Prefer first `virtio-gpu-pci` + GTK without GL to separate DRM/KMS/Wayland from host EGL issues; if niri requires accelerated EGL, test `virtio-gpu-gl-pci` with `gtk,gl=on` separately. Do not add portals or a display manager yet.

## 2026-09-24 graphics milestone — verified

The previous niri rendering blocker is resolved in the direct-root test environment.

Verified end-to-end in a fresh `LUNA_ALPINE_NIRI=1` QEMU image:
- UEFI -> `luna-boot` -> Luna kernel 7.2.4 -> Alpine 3.24 direct root -> OpenRC.
- `seatd` starts and the `luna` user reaches the Alpine shell.
- Native Alpine `niri-25.11-r2` starts on `/dev/dri/card0` / `/dev/dri/renderD128`, selects QEMU `Virtual-1` at 1280x800, and creates a Wayland socket.
- Alpine 3.24 Mesa `mesa-dri-gallium` lacks the virgl Gallium driver needed by the QEMU test. A separate Mesa 26.1.6 build with `gallium-drivers=virgl,llvmpipe` is therefore kept isolated under `/opt/luna-test/mesa-virgl`.
- Niri and Noctalia now use `LD_LIBRARY_PATH`, `LIBGL_DRIVERS_PATH`, and `GBM_BACKENDS_PATH` pointing at that test-only Mesa runtime.
- Noctalia 5.1.0 was built natively against musl on Alpine 3.24 with Meson/C++ and is installed at `/usr/local/bin/noctalia` with its full assets tree.
- The Noctalia ELF closure resolves cleanly inside the Alpine rootfs; `noctalia --version` returns `noctalia v5.1.0`.
- Manual compositor startup successfully exercised Niri `spawn-at-startup`: a Noctalia process appeared as the user and created its IPC sockets, default bar, wallpaper, keyboard/pointer bindings, and EGL context.
- Noctalia renderer is verified as Mesa `virgl` on the QEMU guest: `OpenGL ES 3.2 Mesa 26.1.6`, renderer `virgl (Mesa Intel(R) UHD Graphics 630 (CFL GT2))`.
- The successful Noctalia log shows a working Wayland compositor connection, `layer-shell`, one `1280x800` output, EGL 1.5, and the default top bar/wallpaper.

Known non-blocking warnings in this minimal test environment:
- system D-Bus is not available unless the new test session wrapper starts a bus; Noctalia can still render without it, but power/portal/system integrations remain disabled.
- PipeWire/WirePlumber are now packaged into the test root for the session wrapper; their functionality must be rechecked after the next fresh auto-session run.
- Xwayland-satellite is intentionally absent for this stage.
- The host/QEMU test still reports the expected unrelated ACPI/WMI and missing optional logind properties.

Current direct-root graphical test plumbing:
- `components/boot/tests/ovmf/run-alpine-luna-kernel-direct-root.sh`
- `dist/.build/alpine-foundation-graphics/mesa/out/`
- `dist/.build/alpine-foundation-graphics/noctalia/out/`
- `dist/.build/alpine-foundation-graphics/noctalia-runtime-apks/`

Immediate next action:

Run one fresh auto-session image after the latest hook change and verify `/var/log/luna-niri.log`, `/var/log/luna-pipewire.log`, and `/var/log/luna-noctalia.log`. If all three are populated, the Alpine graphical foundation can move from manual-session validation to reproducible session startup. Do not move this design to `develop` yet.

## 2026-09-24 auto-session validation — verified

The direct-root graphical test is now reproducibly auto-starting the full user session without manually launching Niri.

Verified from guest ttyS1 after a fresh `LUNA_ALPINE_NIRI=1` image build:
- `dbus-run-session` remains alive with its `dbus-daemon` child.
- `niri --session` is alive as `luna` and listens on Wayland `wayland-1` / Niri IPC.
- `pipewire` and `wireplumber` remain alive under the same user session.
- Noctalia is launched by Niri `spawn-at-startup "noctalia"`; its log reports Wayland compositor/layer-shell binding, EGL 1.5, Mesa virgl renderer, default wallpaper, default top bar, and both IPC sockets.
- WirePlumber reaches `mixer-api ready` after adding the missing `lua5.4-libs` runtime dependency.
- Noctalia now connects to the user session bus and registers `org.freedesktop.Notifications` successfully.

Remaining environment limitations are explicit and non-fatal for the graphical foundation:
- There is no system D-Bus socket (`/run/dbus/system_bus_socket`) yet, so RTKit, logind watchers, and some system-wide integrations remain unavailable.
- WirePlumber therefore reports optional system-dbus/logind and Bluetooth SPA components as unavailable; core PipeWire mixer functionality still reaches `mixer-api ready`.
- The minimal image has no Xwayland-satellite and no CPU temperature sensor.
- Noctalia network-backed palette/template/FX-rate services cannot resolve external DNS inside this test environment; cached/default UI operation is unaffected.

The direct-root graphical path is therefore reproducible through boot, user-session creation, compositor startup, shell startup, and PipeWire/WirePlumber startup. The remaining system-dbus/logind/Bluetooth limitations are service-integration gaps, not blockers for the graphical compositor/shell path.

Next implementation step: decide whether the Alpine foundation experiment should add a real system D-Bus service and Bluetooth SPA support, or keep those as later external-system responsibilities. Do not promote the direct-root experiment to `develop` yet.

## 2026-09-25 graphical greeter + logged-in session — verified

The Alpine direct-root graphical test now reaches a complete graphical login and remains in the logged-in Luna session.

Verified with a fresh QEMU boot using `greetd` + Cage + GTKGreet:
- GTKGreet renders correctly after adding the Alpine GTK/gdk-pixbuf/glycin runtime closure, `gdk-pixbuf` loader cache generation, fontconfig cache generation, and `font-dejavu`.
- GTKGreet is run without `--layer-shell` because Cage does not provide the layer-shell path required by `gtkgreet -l` in this test.
- The greeter runtime uses `/run/greetd-runtime`; its diagnostics/logs no longer write to root-only `/var/log`.
- `/run/seatd.sock` remains `root:seat` with mode `0660`. The synthetic `greetd` user uses primary GID `seat`, while `luna` is also a member of `seat`.
- A graphical login as `luna` was verified. `greetd` starts the user session and does not return to the greeter.
- After login, `niri --session`, `pipewire`, `wireplumber`, and `noctalia` remain alive under user `luna`.
- Niri reports Wayland socket `wayland-1` and its IPC socket; Noctalia creates its Wayland IPC and dmenu IPC sockets and renders the default top bar/wallpaper.
- WirePlumber reaches `mixer-api ready` and `default-nodes-api ready`.

The login-loop root cause was an incorrect userspace redirection: the user session attempted to open `/var/log/luna-{pipewire,wireplumber,niri,noctalia}.log` as `luna`, causing the session command to fail. Session logs are now placed under `$XDG_RUNTIME_DIR`.

The seat-access root cause was also corrected: the greeter and logged-in user both use the `seat` group for the shared `seatd` socket.

Known non-blocking warnings remain: Xwayland-satellite is absent, system D-Bus/logind integration is absent, the QEMU virgl renderer is slow, and Niri reports no default Xcursor icon in the minimal image.

## 2026-09-25 desktop foundation polish — verified

The Alpine direct-root graphical foundation is now reproducible end-to-end with the current Luna kernel 7.2.4 and Alpine 3.24 userspace.

Verified system integration:
- OpenRC cgroups, dbus, elogind, bluetooth, networkmanager, polkit, rtkit, greetd, and seatd all start successfully.
- D-Bus system activation works for org.freedesktop.UPower and org.freedesktop.Accounts; Alpine SystemdService= entries are stripped in the OpenRC test image and dbus-daemon-launch-helper privileges are restored at runtime.
- WirePlumber BlueZ SPA starts without the previous UPower warning after the UPower readiness barrier was added before PipeWire/WirePlumber.
- The QEMU virtio NIC is brought up through BusyBox udhcpc because the experiment intentionally keeps libudev-zero; the guest receives DHCP and DNS works through the QEMU user-network gateway.
- The Alpine mdev init script is replaced in the test image with a modern-kernel-safe coldplug path; the obsolete /proc/sys/kernel/hotplug write is gone.
- The obsolete Alpine kernel.unprivileged_bpf_disabled sysctl is removed from the disposable test image because the current Luna kernel does not expose that sysctl node.

Verified graphical integration:
- Native musl Niri starts automatically and keeps the Mesa 26.1.6 virgl renderer isolated under /opt/luna-test/mesa-virgl.
- Noctalia 5.1.0 starts directly from the Niri config.
- Adwaita icon theme and hicolor fallback are present.
- Adwaita cursor theme is installed, a default cursor inheritance theme is provided, and Niri explicitly sets xcursor-theme Adwaita / size 24.
- gnome-keyring Secret Service is started through the logged-in PAM session.
- loginctl reports the Luna graphical session as active on seat0.

QEMU-only profile:
- Noctalia's system monitor is disabled in 00-luna-qemu.toml because this virtual machine has no physical CPU thermal sensor. This is a test-environment setting only; it does not redefine Luna desktop architecture.

Known non-blocking Niri notices retained intentionally:
1. EGL_WL_bind_wayland_display is unavailable on the current virgl/EGL path, while Niri continues to initialize the compositor and render normally.
2. org.freedesktop.locale1 is absent because this foundation uses OpenRC rather than systemd-localed; the test configuration supplies an explicit XKB layout (us), so keyboard initialization does not depend on locale1.

The current Alpine foundation stage is therefore functionally complete for the direct-root desktop experiment. Do not promote this test launcher to develop yet. The next implementation phase should first make the Mesa/Noctalia recipes and runtime closure reproducible from tracked project tooling rather than from dist/.build state.


## 2026-09-25 desktop foundation final polish — verified

A fresh QEMU boot now verifies the complete Alpine direct-root desktop foundation with KVM, four vCPUs, virtio GPU, QEMU user networking, system D-Bus, elogind, UPower, BlueZ, NetworkManager, polkit, rtkit, greetd, Cage, GTKGreet, Niri, PipeWire, WirePlumber, Noctalia, Xwayland-satellite, icons, cursors, fonts, and GNOME Keyring.

Verified:
- Graphical login remains stable after `luna` authentication and the session does not return to greeter.
- `loginctl` reports the active `luna` session on `seat0/tty1`.
- UPower starts as an OpenRC service before the user session; the earlier WirePlumber/UPower startup race is gone and the WirePlumber warning is no longer present.
- `org.freedesktop.UPower` and `org.freedesktop.Accounts` activate successfully on the real system D-Bus.
- `eth0` receives a QEMU DHCP lease (`10.0.2.15/24`) with a default route through `10.0.2.2`; DNS resolution works. The test uses BusyBox `udhcpc` because this experiment intentionally keeps `libudev-zero` and does not introduce a second udev stack.
- `Adwaita` and `hicolor` icon themes are installed, an Adwaita `default` cursor theme is present, and Niri explicitly selects Adwaita cursor theme/size 24. The greeter also uses Adwaita icons/cursors.
- GNOME Keyring starts for the logged-in user through PAM/session integration and the Secret Service component is available.
- `xwayland-satellite` is installed and `/tmp` plus `/tmp/.X11-unix` are restored to `root:root` mode `1777` before the desktop starts.
- The direct-root mdev init no longer writes the obsolete `/proc/sys/kernel/hotplug`, and the Alpine BPF sysctl incompatible with this kernel is filtered from the generated test root.
- The test launcher passes `bash -n` and `git diff --check`.

Remaining warnings are intentionally limited to capability differences that are not desktop failures:
- Niri reports that the current Mesa virgl stack does not expose `EGL_WL_bind_wayland_display`.
- Niri probes the optional `org.freedesktop.locale1` service; the foundation uses OpenRC + elogind and does not ship systemd-localed.
- Noctalia cannot read a CPU thermal sensor from the QEMU guest. The guest has no hardware thermal zone to expose, so fabricating one would hide the limitation rather than solve it.

No fake locale service, thermal sensor, EGL shim, or second udev stack was added just to silence these messages.

The graphical desktop foundation milestone is complete. The next development step should move the currently cache-only Mesa virgl, Noctalia, greeter, and runtime-closure recipes into tracked repository tooling so a clean checkout can reproduce this environment without depending on `dist/.build`. After that, review which Alpine services remain external-system responsibilities and which boot/runtime responsibilities should transition to Luna-owned components. Keep the direct-root experiment isolated from `develop`.

## 2026-09-27 canonical Luna production boot — verified

The canonical builder now creates a minimal immutable System Image plus the real LUNA-SYS boot artifact set: System Image manifest, luna-init core and manifest, boot-state, and Recovery DATA Image with manifest. Niri and its musl closure remain DATA-owned.

Production OVMF verification now passes through UEFI, luna-boot, Luna kernel 7.2.4, memory-resident luna-init PID1, logical root preparation, luna-system-runtime, native UserSession and DRM/KMS login UI. The canonical Luna font and Luna artwork load from logical /resources. The disposable luna/luna credential authenticates and luna-system-runtime launches /usr/bin/niri --session. The source image checksum remains unchanged because tests use snapshot copies.

Fixes made during this milestone include idempotent kernel-provided devtmpfs handling, explicit /resources mapping, DATA musl loader mapping at /lib/ld-musl-x86_64.so.1, removal of the physical /luna-data user-home symlink, and production regression harnesses using virtio-gpu-pci.

Current limitation: D-Bus, NetworkManager, BlueZ and UDisks are not yet packaged in canonical DATA, so the runtime reports them as unavailable. Niri launches successfully with no immediate error during the validation window, but a longer-lived Wayland session regression remains to be added.

The direct-root Alpine desktop experiment remains the control path and is not promoted to `develop`.


## 2026-09-27 — external system provider closure milestone

The canonical LUNA-DATA provider layer now carries Alpine musl builds for the runtime system D-Bus daemon, NetworkManager and BlueZ daemon. luna-system-runtime starts all three from DATA in production OVMF; their executables are not copied into the boot-critical System Image.

Provider ownership remains external: executables live under LUNA-DATA/system/apps/<provider>/, shared runtime files under LUNA-DATA/system/libs/, and provider configuration under LUNA-DATA/system/config/. The runtime namespace exposes the daemons through /usr/bin without reintroducing the retired service-manager login path.

The canonical production OVMF regression was updated to assert these three provider-start markers and still passes image, PID1, logical-root, native login-renderer and no-panic checks. Login/session regression also passes after the provider addition: luna/luna authentication is accepted and /usr/bin/niri --session is launched.

UDisks2 is intentionally still pending. Its Alpine package is available, but the required block-device/polkit dependency closure is not present in the existing local cache; no host glibc binaries are used as a substitute. The remaining validation gap is a longer-lived Wayland/Niri session regression, plus completion of the UDisks2 musl provider closure.


## 2026-09-27 — sustained graphical session regression

The canonical login regression now keeps the QEMU guest alive for 20 seconds after injecting the development login credentials. The test requires successful authentication and Niri launch, and rejects graphical desktop exit, session-child start/wait failures, login UI I/O failure and kernel panic markers.

The fresh canonical image passes this sustained session check: luna/luna authentication is accepted, /usr/bin/niri --session is launched, and no graphical desktop exit is reported during the 20-second observation window. The structural canonical disk verification also passes on the same rebuilt image; GPT remains EFI / LUNA-SYS / LUNA-DATA / SWAP with the accepted partition sizes.


## 2026-10-10 — tracked graphics build and Noctalia login acceptance

The tracked Alpine graphics build now builds Mesa VirGL and Noctalia from the
repository recipe, saves Noctalia's full asset tree beside its executable, and
places both in canonical DATA. The recipe also supplies the Alpine `stb`
headers in the directory layout expected by Noctalia's Meson build. Runtime
APK caches, Noctalia assets, and Mesa outputs share one configurable build
root so the canonical image builder and the OVMF provider test consume the
same artifacts.

The new `noctalia-session` QEMU case passed after native Luna authentication.
Noctalia connected to its D-Bus session bus and Niri's Wayland layer-shell,
initialized EGL 1.5 on Mesa VirGL, reported the QEMU Intel UHD 630 renderer,
created its wallpaper and top bar, and remained alive through the QEMU
observation window. This test verifies initialization through logs; it does
not capture guest pixels and does not validate PipeWire/audio or plugin
installation. The existing Niri-only case remains available independently.

The current build and test artifacts were placed under `/tmp` to avoid
overwriting the repository's existing `dist/.build` and canonical disk
outputs. Continue on `luna-onefile-experiment`; do not promote the experiment
to `develop` without a separate integration review.

## 2026-10-10 — canonical audit and stabilization continuation

Continue from [GRAPHICS-BRANCH-AUDIT.md](GRAPHICS-BRANCH-AUDIT.md) and
[ALPHA-STABILIZATION.md](ALPHA-STABILIZATION.md), rather than treating older
direct-root/provider-only milestones as proof of the current full image.
Actual branch is `luna-onefile-experiment`, HEAD
`a1ea27951c188320d19b067be3f071f39ab9f19b`; preserve existing tracked/untracked work
and do not commit or switch branches without a new request.

The current stage supplies tracked canonical input preparation and integrity
checks, required DATA namespaces, boot timing diagnostics, and one full QEMU
regression entrypoint. A fresh input closure exposed Niri's unlisted dlopen
dependency on `libwayland-server.so.0`; it is now an explicit package requirement.
Consult the stabilization report and actual final regression evidence for results.
The final run in `target/luna-alpha-stabilization-20261010/regression-final`
completed with PASS: 26 focused Rust tests, six input-verifier negative checks,
structure/production/display, three 60-second desktop sessions, logout, reboot
with a second authenticated desktop and persistent HOME/config, and guest
shutdown. System state revision remained 1 across reboot. The exact host syscall
behind the original sporadic timeout remains unproven; fixture copy syncing and
state-opening diagnostics are now in place.

The user explicitly chose the next stage: a real application Bundle lifecycle
for system and third-party applications through the existing `luna-bundle`,
`luna-app-manager`, and `luna-app-runtime` boundaries. This requires installation,
registration, authorized launch, data/config persistence, update/rollback and
uninstall with retained user data; loose binaries with an added label do not
complete the stage. Keep System Image, kernel and init artifact formats unchanged.

The user explicitly approved ongoing Project Luna technical notes in claude-mem
on 2026-10-10. `save_memory` confirmed note #4, titled
`2026-10-10 Project Luna: Alpha stabilization verified; next application Bundle lifecycle`,
in project `project-luna`. It records the completed stabilization and the pending
Bundle stage. The initial search/task failures were traced to a Codex plugin
13.35.0 / Claude worker 13.25.3 conflict. Both installations are now aligned to
13.35.0 after a database backup. Search retrieved note #4 through all three
layers; task writes/reads and startup rendering of the open Bundle stage passed.
Chroma connected and completed backfill. The automatic observer still requires
Claude Code authentication; explicit saves and retrieval work independently.

The user clarified that the requested graph skill is Graphify. Its official
Codex skill and references are installed from the existing CLI 0.9.57. The local
code-only graph covers 133 component files; `graphify-out` at the repository root
links to `target/luna-graphify-20261010/graphify-out`. The graph is navigation,
not an authoritative architecture or a semantic conversation history. Repair
details and limitations: [MEMORY-AND-GRAPHIFY.md](../MEMORY-AND-GRAPHIFY.md),
also recorded in claude-mem note #5.

## 2026-10-10 — application Bundle storage and CLI

The first application storage stage is implemented and verified on the same
`luna-onefile-experiment` branch and `a1ea279` HEAD, without commits. Existing
boot/init/desktop changes and untracked test harness files are preserved.
See [STORAGE-STAGE.md](../application-bundles/STORAGE-STAGE.md) for commands,
evidence, storage layout and remaining lifecycle work.

The actual musl `luna` executable plus a resource now form an LBP1 application
Bundle using `examples/bundles/luna-cli/manifest.toml` and
`tools/build-luna-cli-bundle.sh`. The CLI delegates inspect, install, independent
version update, verify, remove and interrupted-operation recovery to the existing
Bundle/AppManager/UpdateEngine/State/Security components. Management authorization
and trust in the exact content identity are separate checks; neither grants
execution capabilities. Signed Bundles and unresolved dependencies are rejected
with explicit errors until their verification/resolution exists.

The private version registry is `DATA/system/apps/.bundles/<id>/<version>`.
It retains old versions, user data/config and loose legacy providers. Transactions
use the existing `system/state/luna-state.redb`, exclusive DATA locking, complete
filesystem checkpoints, atomic directory publication and idempotent rollback.
Recovery checks the durable plan and phase before reverting an interrupted apply.
Offline CLI requires an explicitly chosen, operator-owned DATA root and must not
share its database with a live runtime.

Final evidence: 44 existing Rust tests passed; Clippy with `-D warnings` and
app-runtime/system-runtime compilation passed. All 20 storage acceptance scenarios
passed in `target/luna-bundles-20261010/runs/verified/`, including corruption,
permission/link rejection, process interruption after publication, rollback,
durable journal write failure and data retention. An independent read-only review
was completed. QEMU and physical disks were not exercised in this storage stage.

The next stage is real installed Bundle launch through the existing active
UserSession and app-runtime, including RuntimeSpec/ELF closure, logical resources,
persistent application mappings and exit status, followed by canonical QEMU
acceptance and provider/third-party packaging. Offline commands do not fake an
active session. Full application lifecycle remains open.

The root Graphify link now points to
`target/luna-graphify-bundles-20261010/graphify-out`: code-only extraction covers
134 component files, 2678 nodes, 7166 edges and 110 communities. The old snapshot
is retained; source fingerprint and AST normalization/collapse diagnostics are
saved beside the graph. It is a navigation aid with incomplete parallel relation
types, not execution evidence. Explicit claude-mem
notes and work-state writes can be saved/read independently of the automatic
observer, whose Claude Code authentication remains unavailable.

## 2026-10-11 — registered native Bundle and Wayland launch

Current branch/HEAD remain `luna-onefile-experiment` / `a1ea27951c188320d19b067be3f071f39ab9f19b`.
All earlier tracked/untracked source and test harness work is preserved; no commits,
reset or branch switch. Directory-v1 installation is `name.lunaapp` with TOML
metadata and `payload/`; ID/version stay in metadata, `.lbp` is transport only.
See [DIRECTORY-STAGE.md](../application-bundles/DIRECTORY-STAGE.md), the accepted
[directory contract](../../contracts/DIRECTORY-BUNDLE-CONTRACT.md), and
[LAUNCH-STAGE.md](../application-bundles/LAUNCH-STAGE.md) for the prior completed
offline storage and static-musl active-UserSession stages.

The next implemented subset is explicit Bundle-owned dynamic musl closure and
one authorized compositor endpoint: see
[DYNAMIC-WAYLAND-STAGE.md](../application-bundles/DYNAMIC-WAYLAND-STAGE.md).
It uses existing components and `luna-init` remains sole PID 1. Loader/library
sources require verified content and separate grants; Wayland proof is tied to
UserSession, UID, pinned inode and desktop peer PID/cgroup. Exact socket
`ResolveUnix` plus abstract UNIX scope require Landlock ABI9, fail closed.
The selected Linux 7.2.4/input snapshot already has this support and is unchanged.

Host verification: 118 focused musl tests, one privileged test ignored, scoped
Clippy `-D warnings` PASS. Independent review found and closed loader-equivalence
corner cases: repeated/forged headers, page-rounded LOAD/BSS replacement, partial
or distinct PHDR aliases, missing in-range DT_NULL and the interpreter/no-dynamic
early-return bypass. Earlier QEMU `20261011-190128-154613` verified a real Weston
registered Bundle window, UID1000/payload libraries, named/abstract UNIX denial,
logout cleanup and persistence after reboot. Its controller was cancelled before
promotion after review; it is partial evidence. Latest source was built in
`target/canonical-workbench/evidence/20261011-192344-220583`, with Bundle, production,
display, three desktop sessions and logout PASS; reboot case was interrupted by
unplanned UI input and QMP host-ui closure. Retest through EGL headless exposed a
split serial marker from concurrent provider output, while guest reached login.
Boot runlevel/readiness/revision diagnostics now format a whole line before
writing; strict acceptance assertions are unchanged. Final full latest-source
regression **PASS** at `target/canonical-workbench/evidence/20261011-225619-427520/results.txt`:
production/display, all 16 Bundle markers plus suite PASS, three desktop sessions,
logout/reboot/shutdown. EGL headless retains actual VirGL rendering and controlled
keyboard input. Weston window/UID1000/payload libraries, capability/library/UNIX
denials, descendant logout cleanup and registry/data/config reboot persistence
are verified; pixels and physical hardware were not tested.

Workbench now has reference B SHA256
`a512704c8070ae010c24241cd093461a9beabd5a36495ec19539fc717deee858`;
previous reference A SHA256
`4c0c6459712b8b4f3c85d4740b4260fef8294d5e783a5c94e9a267ef40530a10`
is retained as the next experimental/return slot. Temporary test disks and marked
build scratch are removed automatically. All 350 baseline source/link entries
remain: 328 byte-identical, 22 intended edits, no missing/unexpected edits;
three new source/doc files and one generated graph cache. All 13 frozen code
hashes match the completed test build. Both input snapshots and kernel remain
unchanged. Final checks: `target/luna-dynamic-apps-20261011/evidence/final-verification.json`.

Root `graphify-out` now links to `target/luna-graphify-dynamic-20261011/graphify-out`:
143 indexed component files, 2949 nodes, 7803 edges and 139 communities. Extraction
was local code-only with no semantic LLM; all indexed file fingerprints and old
graphs/query memory are retained. It remains partial code navigation, not runtime
proof or semantic conversation memory.

Still pending: authorized terminal stdio, registered app desktop discovery, real
Alpha/third-party application packaging and required GPU/audio/portal resources;
live management IPC, signatures/glibc and explicit journalled legacy migration.
Production does not include test grants/fixtures or install Weston automatically.
