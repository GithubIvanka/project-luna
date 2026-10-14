# Аудит графической сессии Alpine foundation и Luna

Дата: 2026-10-10.

## Область сравнения

- Контрольная ветка `alpine-foundation`: `1e5ece316c29c4d83dfcd702f33c2a181fd2b8ff`.
- Текущая ветка `luna-onefile-experiment`: `a1ea27951c188320d19b067be3f071f39ab9f19b`
  плюс существовавшие незакоммиченные изменения и исправления этого аудита.
- Ветки изучены через `git show` и `git diff`; рабочая ветка не переключалась.
- Исходный полный образ: `target/luna-canonical-noctalia-20261010/luna-0.1.0.img`.
  Его SHA256 до проверки:
  `9ec4a51ccc4981c5e854f54e781dc50a69b31ac92c796485fb4d424de6173afc`.

Сравниваются два разных способа сборки: рабочий Alpine direct-root desktop и
канонический образ Luna с минимальным System Image и DATA providers. Исторический
direct-root тест в этом аудите заново не запускался; его поведение сопоставлено
по сохранённому launcher и документированным результатам. Полные образы Luna
проверяются свежими OVMF запусками.

## Найденные различия

| Область | Рабочий Alpine direct-root | Полный Luna до исправления | Вывод и действие |
| --- | --- | --- | --- |
| Mesa loader | Специальная Mesa в `/opt/luna-test/mesa-virgl/lib`, приоритет в `LD_LIBRARY_PATH` | Специальная Mesa сначала копировалась в общий `system/libs`, затем заменялась библиотеками service provider | Восстановлен отдельный DATA каталог `system/libs/mesa-virgl` и явный приоритет loader |
| DRI/GBM | Пути указывали на ту же специальную сборку | DRI от специальной Mesa сочетался с заменёнными общими библиотеками | `LIBGL_DRIVERS_PATH` и `GBM_BACKENDS_PATH` указывают на приватную Mesa |
| Ядро и устройства | DRM, virtio GPU, input, необходимые Linux primitives | Нужные параметры включены; ядро обнаруживает card0 и renderD128 | Новая смена архитектуры или ядра для этого сбоя не нужна |
| Niri runtime | musl, Wayland server library, libinput, XKB, seatd | Эти providers и ресурсы присутствуют | Причина renderer не в пропущенном Niri executable или Wayland socket |
| Noctalia assets | Полное дерево `usr/local/share/noctalia` | В исходном образе ресурсы уже добавлены прежними незакоммиченными изменениями | Сохраняются в DATA; wrapper задаёт `NOCTALIA_ASSETS_DIR` |
| Системные шрифты | Установлены Adwaita Sans/Mono; fixture копирует `/usr/share/fonts` | Fontconfig config есть, но `system/resources/fonts` пуст; у Noctalia остаётся только bundled icon font | Восстановлена установка system fonts из APK payload; добавлены structural assertions |
| Qalculate definitions | Пакет устанавливает `/usr/share/qalculate` | Копировались библиотеки, но XML definitions пропущены | Definitions добавлены в ресурсы Noctalia и отображаются в логический `/usr/share/qalculate` |
| Noctalia dependencies | Устанавливается список runtime packages и их зависимости | Исходный образ имеет нужные библиотеки, но оставшийся локальный APK cache неполон | Восстановлен cache по tracked списку; сборщик проверяет пакеты и musl ELF closure |
| Desktop service packages | Полный подготовленный cache содержал OpenRC и service packages | Graphics recipe загружал только Noctalia packages, хотя disk builder требует дополнительные service APK | Добавлен `desktop-service-packages.txt`; graphics recipe явно загружает оба набора |
| udev/elogind | Полный пакет содержит `elogind-uaccess-command` по FHS пути | Правило скопировано, executable пропущен | Executable добавлен как DATA app; правило адаптировано к `/usr/bin/elogind-uaccess-command` |
| Системные группы | Alpine определяет группы из udev/OpenRC rules | В минимальном bootstrap отсутствуют video/audio/disk/uucp и другие provider groups | Восстановлены используемые provider groups; пользователь не включён в disk |
| Журнал Noctalia | Session logs сохраняются | Serial tail прекращался после первого обнаружения PID | Tail живёт до завершения сессии; сохраняется и поздняя инициализация оболочки |
| Проверка жизни | Требуется работа оболочки | `kill -0` может принять zombie за живой процесс | Проверяется состояние процесса; exit и zombie отклоняются |
| Отрицательные assertions | Исторические PASS требуют проверки фактических логов | `! grep` не вызывает выход при `set -e` | Отказы реализованы через явный `if ...; exit 1`; требуются экран, renderer и поверхности |
| Logout | Контрольный native launcher ожидает desktop и возвращает login | Runtime запускает desktop generic process API без process → UserSession registration | Использован существующий session-aware supervisor; очищаются desktop group и user runtime directory |
| D-Bus/Polkit | Единые `/usr/share/dbus-1` и `/usr/share/polkit-1` | Разные DATA apps монтируют целиком одноимённый namespace поверх друг друга | UDisks и desktop policies объединены у одного владельца ресурса |
| Power | Полные FHS applets и работающий init | Нет `/sbin/poweroff`, `/sbin/reboot`; Luna PID 1 не обрабатывает init signals | Добавлены applet aliases и минимальное завершение в persistent PID 1 |
| Provider users | APK install hooks создают polkitd/rtkit | Direct extraction не выполняет hooks | Bootstrap явно содержит locked service accounts и power policy для development wheel user |
| Экран firmware | Alpine boot path подготавливает GOP screen_info | Normal boot пропускает существующий helper; QEMU использует GPU без VGA fallback | GOP helper общий для обоих путей; QEMU использует `virtio-vga-gl`, результат проверяется отдельно |

## Доказательство причины сбоя Mesa

В исходном образе `libEGL.so.1.0.0`, `libgbm.so.1.0.0` и
`libgallium-26.1.6.so` побайтно совпадают с
`dist/.build/alpine-system-service-root/usr/lib`, но отличаются от сборки
`mesa-virgl`. Например, SHA256 `libgallium-26.1.6.so`:

- Специальная Mesa:
  `1325c076f4f706d00d9aac764018ae7478222b20175e5a34b86ec4c90989159c`.
- Исходный образ и поздний service provider:
  `c3aa5413c4dbdc50f0e529ebade3fe8ea004af59da1a07412da21e13ed614518`.

Этот порядок копирования уже присутствует в каноническом builder ветки
`alpine-foundation`. Рабочий direct-root launcher этой ветки избегает проблемы
через отдельный Mesa каталог. Поэтому сравнение не подтверждает предположение,
что именно новый manager gate или новое ядро сломали renderer.

Свежий контрольный запуск исходного образа воспроизвёл
`virtio_gpu: driver missing`, `software EGL renderers are skipped` и
`no allocator available for device`. Усиленный тест завершился с exit 1.

Первый пересобранный образ с одной лишь изоляцией Mesa уже подключил
`Virtual-1` и выбрал 1280x800. Он выявил отдельный неполный локальный cache:
Noctalia не находила sdbus-c++, libsecret, libqalculate, libical,
tomlplusplus и md4c. В исходном образе эти библиотеки присутствуют; это ограничение
оставшихся входных артефактов сборки, а не доказательство их отсутствия в старом
образе. Зависимости восстановлены из Alpine по существующему
`tools/alpine-foundation/noctalia-runtime-packages.txt`.

Чистый cache Noctalia также обнаружил скрытую зависимость disk builder от
`dbus-openrc` и других service APK из старого подготовленного cache. Они теперь
объявлены в `desktop-service-packages.txt` и загружаются graphics recipe
отдельно от Noctalia dependencies. Оба набора располагаются в одном выбранном
runtime cache; Niri/Noctalia test fixture не получает новые системные службы
как обязательный архитектурный слой.

Отрицательные проверки отдельно подтвердили отказ на неполном cache до
сборки и отбрасывание настоящего zombie-процесса. В offline ELF check учитывается
гостевой RUNPATH elogind `/usr/lib/elogind`; эта shared library уже присутствует
в DATA и не является новым пропущенным файлом.

## Падение после запуска оболочки: исследование core dump

После исправления Mesa Noctalia создавала EGL context, wallpaper и bar, но
следующая длительная проверка обнаружила её выход. Журнал ядра подтвердил
SIGSEGV; main process завершился с status 139. Корреляция с последними строками
WirePlumber не была принята за причину.

Из отдельной disposable DATA copy извлечён полный core dump. GDB подтвердил
стек `memcpy -> CairoTextRenderer::rasterizeLayout -> lookupOrRasterize -> draw
-> RenderContext::renderNode -> Surface::render`. Source pointer в `memcpy`
равен NULL. Исходники именно установленной Noctalia 5.2.0 читают
`cairo_image_surface_get_data` без проверки ошибки surface.

Сравнение package resources затем подтвердило пропуск обычных системных
шрифтов. У рабочего Alpine root и отдельного fixture есть Adwaita Sans/Mono,
а в каноническом DATA их нет. Эти fonts восстановлены через существующий
resource namespace; результат проверяется свежими 60-секундными запусками.
Noctalia исходники пока не изменены. Renderer crash не скрывается отключением
оболочки, PipeWire или setup wizard.

Wrapper теперь записывает точный PID запущенного child и его `wait` status в
Noctalia log. Monitor читает этот PID вместо поиска по имени. Журнал Niri
не обязан пересылать stdout/stderr `spawn-at-startup`, поэтому wrapper сам
направляет диагностику в файл, который уже пересылается в serial.

## Проверки и артефакты

Все новые артефакты находятся в `target/luna-foundation-audit-20261010/`:

- `evidence/working-tree-before.patch`, `status-before.txt`, `refs.txt` — исходное состояние.
- `evidence/mesa-hashes-before.tsv` — контрольные суммы сравниваемых библиотек.
- `ovmf-before/logs/canonical-login-test-serial.log` — свежий отказ исходного образа.
- `ovmf-after/logs/canonical-login-test-serial.log` — промежуточная проверка исправления Mesa.
- `graphics/` — Mesa и Noctalia из существующих Docker artifacts и полный runtime APK cache.
- `image-final/` — промежуточная сборка после восстановления Mesa/fonts.
- `image-complete/` — обычный итоговый образ, без тестовых session actions.
- `evidence/build-complete.log`, `structure-complete.log`, `production-complete.log`,
  `login-complete.log` — проверки итогового образа.
- `evidence/noctalia-full-backtrace.log`, `noctalia-crash-registers.log` —
  подтверждённый SIGSEGV и стек text renderer.
- `evidence/login-repeat1.log`, `login-repeat2.log`, `login-repeat3.log` —
  повторные 60-секундные проверки после восстановления шрифтов.
- `pixel-restore/users/luna/home/luna-desktop.png` — Niri screenshot 1280x800;
  визуально подтверждены wallpaper, top bar, текст и power menu. Capture добавлялся
  только в disposable test variant builder.
- `evidence/{shutdown,reboot,logout}-verified.log` и соответствующие
  `<action>-verified/logs/serial.log`, `qmp-events.json` — функциональные сценарии.
- `evidence/display-verified.log`, `display-negative.log`,
  `display-{verified,negative}/frames.json` — положительный и отрицательный GPU test.

Старые неуспешные промежуточные журналы сохранены. Часть disposable raw disks
и partition files удалена после проверки для освобождения места; исходный
пользовательский образ, исходники и исходный diff не удалялись.

### Подтверждённые результаты текущего кода

| Проверка | Результат | Доказательство |
| --- | --- | --- |
| musl luna-init unit tests | PASS, 3 tests | `evidence/init-lifecycle-tests.log` |
| musl System Runtime library/binary tests | PASS, 8 + 5 tests | `evidence/runtime-lifecycle-tests.log` |
| UEFI bootloader и musl release artifacts | PASS, собраны ordinary builder | `evidence/build-complete.log` |
| GPT, namespaces, точные Mesa bytes, fonts, closure, resource owners | PASS | `evidence/structure-complete.log` |
| Полный production boot и manager gate до boot success | PASS | `evidence/production-complete.log` |
| Native login → Niri/Noctalia, 60 секунд | PASS | `evidence/login-complete.log` |
| Второй последовательный 60-секундный desktop на ordinary image | PASS | `evidence/login-complete-repeat2.log` |
| Logout → login UI → второй authenticated desktop | PASS | `evidence/logout-verified.log` |
| Настоящий poweroff гостя | PASS, QMP guest SHUTDOWN | `shutdown-verified/logs/serial.log`, `qmp-events.json` |
| Настоящий reboot → второй boot success/login UI | PASS, QMP guest RESET | `reboot-verified/logs/serial.log`, `qmp-events.json` |
| Firmware display, ordinary final image | PASS, 20 samples до kernel | `evidence/display-complete.log` |
| Old GPU negative control | Ожидаемый FAIL, surface gap обнаружен | `evidence/display-negative.log` |

Ordinary image SHA256:
`82266792eebf459f1f83205a88cb9c4b749ae279eb6ac295dae347027b3069fd`.
Ни один session-action helper не включён в обычный итоговый образ. Исходный
пользовательский образ после работы имеет тот же SHA256, что до аудита.

Один запуск final login test остановился на 20-секундном ожидании UI, ещё до
первого сообщения runtime о persistent state. В это время выполнялась другая
сборка; причины timeout по этому журналу не установлены. Журнал сохранён как
`evidence/login-complete-first-serial.log`. Два последовательных повтора без
параллельной сборки прошли, deadline не увеличивался. Дополнительный повтор
записан отдельно в `evidence/login-complete-repeat2.log`.

## Logout и power: установленные причины

### Возврат login

Desktop process был запущен generic API supervisor, поэтому его выход не
переводил UserSession в Ended. Runtime теперь использует session-aware запуск
с сохранением environment и credential pre_exec. Отдельная process group
позволяет убрать оставшихся descendants; `/run/user/<uid>` очищается для
реального пользователя завершившейся сессии. Unit test проверяет переход Ended.
OVMF test выполняет logout через Noctalia IPC и второй authenticated desktop
без второго запуска PID 1.

### Выключение и перезагрузка

OpenRC `[ok]` не означал регистрацию service на D-Bus. Второй ресурс
`udisksd/resources/dbus-1` закрывал политики login1/Polkit/NetworkManager/UPower.
После объединения namespaces эти services появились на bus, а NetworkManager
принял Noctalia agent. Исправлены регистр `UPower` config и отсутствующие
polkitd/rtkit accounts из Alpine package hooks.

Следующий реальный запрос poweroff выявил `Failed to execute /sbin/poweroff`:
BusyBox applets доступны в `/usr/bin`, но elogind вызывает `/sbin`. Добавлены
aliases poweroff/reboot/halt; mount applet также явно сохраняется при упаковке.
Power policy повторяет working reference: только login1 reboot/poweroff actions
для development wheel user, без общей авторизации всех Polkit actions.

Luna PID 1 теперь принимает стандартные BusyBox init signals, reap выполняется
и при живом System Runtime. Final shutdown посылает TERM, затем KILL оставшимся
процессам, sync и Linux reboot syscall. Power сообщения записываются одной
kmsg записью: обычный stderr interleaved с kernel shutdown logs дал ложный
отказ первоначального теста, хотя QMP уже фиксировал настоящее выключение.

Функциональные tests используют `noctalia msg session <action>`, общий
`SessionActionRunner` с меню оболочки (проверено в исходниках Noctalia 5.2).
PASS требует elogind Operation finished, PID 1 sync marker и QMP event с
`guest=true`. Reboot дополнительно требует второй normal boot, manager gate,
boot success и новый login UI. Host QEMU termination не считается poweroff.

## Пропадание изображения

Normal boot не вызывал GOP screen_info helper, который уже использовал Alpine
test. Helper перенесён в BootParams и применяется обоими путями; RGB/BGR
framebuffer сохраняет реальный stride и E820 reservation. BltOnly/headless
варианты остаются допустимыми.

Одно лишь восстановление GOP helper **не** устранило QEMU разрыв. При
`virtio-gpu-gl-pci` свежие снимки каждые 100 ms показывали отсутствие CPU surface
примерно 0.42–1.54 секунды от старта наблюдения, ещё до Linux serial marker.
`virtio-vga-gl` сохраняет VGA fallback: в том же окне все firmware samples
имеют surface, затем native login запускается с прежним virtio DRM driver.
Это текущий default полных canonical OVMF tests; старый GPU доступен через
`LUNA_QEMU_GPU` для отрицательной проверки.

После DRM/GL takeover QMP screendump снова может сообщать `no surface` даже
при работающем экране. Это ограничение CPU screenshot API; оно не приравнивается
к потере монитора. Новый display test проверяет именно firmware interval.
Непрерывность сигнала на физическом мониторе/i915 этим результатом не доказана.

## Границы результата

Этот этап проверяет QEMU VirGL desktop через Luna boot, PID 1, менеджеры и
настоящую аутентификацию. Он не доказывает работу i915 на физическом ноутбуке,
аудиовыхода, Wi-Fi/Bluetooth, всей пользовательской среды или установки системы.
Noctalia assets, Wayland socket и наличие PID по отдельности не считаются
достаточным подтверждением desktop.

`luna-init` остаётся PID 1. Device Manager и Volume Manager запускаются
`luna-system-runtime`; OpenRC обслуживает внешних compatibility providers после
подтверждения готовности Luna. Изоляция Mesa — упаковка внешнего provider,
а не новый архитектурный слой.

## Оставшиеся конкретные пробелы

- DNS в полном desktop пока не работает: Noctalia HTTP requests завершаются
  `Could not resolve host`; наличие NetworkManager на D-Bus не доказывает сеть.
- Native login пока не регистрирует elogind session/PAM lifecycle: сообщения
  `NoSessionForPID` сохраняются. Проверенные logout/power paths не требуют
  подмены UserSession внешним display manager.
- Не все provider activation paths/необязательные services упакованы полностью
  (например, secrets и PowerProfiles). Это отдельная integration работа.
- PipeWire/WirePlumber запускаются, но фактический звук и физические
  Wi-Fi/Bluetooth/i915 ещё не проверены; Alpha application baseline неполон.
- На момент графического аудита foundation schema создавала `cach` и не создавала
  `system/state`/`system/volumes`. Следующий этап исправил normal/Recovery DATA
  и assertions: [ALPHA-STABILIZATION.md](ALPHA-STABILIZATION.md).
- Полная координация graceful shutdown providers через System Runtime ещё не
  реализована. Проверенный PID 1 путь выполняет bounded TERM/KILL + sync.

Документация обновлена в существующих component/status files. Первая попытка
сохранения в claude-mem была отклонена automatic approval review. После явного
разрешения пользователя 2026-10-10 выводы сохранены в проекте `project-luna`
как memory #3: `2026-10-10 Luna: canonical desktop audit and verified logout/power/QEMU fixes`.
Поиск mem_search пока возвращает `Memory retrieval failed`; автоматическое
извлечение в следующей сессии не подтверждено. Продолжение разработки также
опирается на этот отчёт и SESSION-CONTINUITY.md.
