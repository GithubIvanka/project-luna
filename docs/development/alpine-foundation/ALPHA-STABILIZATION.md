# Стабилизация canonical Alpha — 2026-10-10

## Область этапа

Продолжение GRAPHICS-BRANCH-AUDIT на фактической ветке
`luna-onefile-experiment`, HEAD `a1ea27951c188320d19b067be3f071f39ab9f19b`.
Существовавшие изменения и пользовательские harness-файлы сохранены; commits,
reset и переключение ветки не выполнялись.

Цель: сделать входы сборки явными, исправить DATA schema и получить один
повторяемый прогон загрузки, desktop и lifecycle до следующего этапа приложений.

## Входы сборки

`tools/prepare-canonical-luna-inputs.sh` создаёт **новый** input root:

- Alpine runtime rootfs из `alpine:3.24.2` с OpenRC/util-linux;
- APK closure Niri и внешних service providers из подписанных Alpine repositories;
- provider root, собранный из объявленных APK, включая UDisks2;
- Mesa VirGL 26.1.6 и Noctalia v5.2.0 из существующих tracked Docker recipes;
- Nerd Fonts v3.5.1 через существующий builder с проверкой release checksums;
- копию отдельно собранного Luna kernel (`LUNA_CANONICAL_KERNEL`);
- `inputs.lock.json`: SHA256 каждого файла и recipe, executable modes, каталоги
  и targets symlinks. Идентичность Docker base также записана в snapshot.

Direct provider versions объявлены в `canonical-provider-packages.txt` и
`desktop-service-packages.txt`. Transitive versions фиксируются именами APK и
контрольными суммами snapshot. Повторный fetch из upstream может получить новый
набор transitive dependencies; это **новый snapshot**, требующий полного прогона.
Повторная сборка из проверенного сохранённого snapshot не обращается к Alpine,
GitHub и прежним вручную подготовленным provider roots в `dist/.build`.

Старый builder path сохранён для существующих экспериментов. Новый путь выбирается
через `LUNA_CANONICAL_INPUT_ROOT`; проверка snapshot выполняется до изменений
выходного каталога. Lock копируется рядом с итоговым `.img` и связан с BUILD-INFO.
Проверка отвергает пропавший шрифт, изменённый бинарник, executable mode, symlink
или добавленный файл. Это воспроизводимость **входов**, не обещание одинакового
SHA256 двух GPT images: disk GUIDs, filesystem UUIDs и timestamps создаются заново.

Первый полный прогон новой preparation recipe обнаружил дополнительную dependency,
которой не было в APK recursive closure: `wayland-libs-server=1.25.0-r0`. Niri
загружает `libwayland-server.so.0` через dlopen; ELF loader check её не проверяет.
Подтверждение: panic `NoWaylandLib`, status 134 в диагностическом serial log.
Теперь package объявлен явно, builder и structure test требуют библиотеку.
Graphical launcher дренирует Niri log при ошибке до удаления runtime directory,
а обычный login test отвергает неожиданный возврат в login UI даже после
успешного первоначального desktop startup.

## DATA

Normal DATA и Recovery DATA создают `cache`, `system/state` и `system/volumes`.
Опечатка `cach` и историческое утверждение теста об отсутствии required namespaces
удалены. Recovery сохраняет пользователя `recovery`, обычная DATA — `luna`.
Persistent state продолжает принадлежать `luna-state`/`luna-system-manager`.

## Задержка открытия состояния

Runtime теперь пишет начало и длительность открытия persistent state и revision.
Первый диагностический запуск воспроизвёл 20-секундный timeout **внутри открытия
состояния**, после подготовки logical root. В спокойном traced запуске открытие
заняло 1850 ms; первый `fdatasync` QEMU занял 1.817 s. В запуске с нагрузкой
записи открытие заняло 247 ms. После предварительной синхронизации тестовой копии
обычный запуск открыл state за 18 ms и прошёл стандартный login acceptance.

Copy-before-boot harness ранее запускал QEMU сразу после копирования большого
raw image. Первый guest fsync мог заодно ожидать сброс host dirty pages этого
файла. Теперь копия синхронизируется **до** запуска QEMU и отсчёта boot deadline;
обычный login deadline остаётся 20 секунд. Полный suite выполняет QEMU cases
последовательно. Эти меры устраняют смешение подготовки fixture и времени guest
boot. Точная длительность host syscall в исходном failed запуске не записана;
связь именно того timeout с host I/O остаётся обоснованной гипотезой.

`LUNA_CANONICAL_TRACE_SYNC=1` включает только fsync/fdatasync tracing через
strace seccomp filter. Без seccomp обычный ptrace замедлял kernel startup и сам
давал timeout; такой диагностический запуск не считается acceptance результата.
Увеличенный `LUNA_CANONICAL_LOGIN_TIMEOUT_SECONDS` предназначен для исследования,
не используется стандартным suite. Entropy starvation не подтверждается:
успешные и failed kernel logs содержат ранний `crng init done`.

## Один регрессионный прогон

Из корня репозитория, после сборки принятого Luna kernel:

```sh
export LUNA_CANONICAL_INPUT_ROOT="$PWD/dist/canonical-inputs"
bash tools/prepare-canonical-luna-inputs.sh
bash tools/prepare-canonical-luna-inputs.sh --verify
bash tools/test-canonical-luna-regression.sh
```

Input root и regression root должны быть новыми. Если snapshot уже подготовлен,
повторяют `--verify` и regression, без повторного fetch/build inputs. По умолчанию
suite теперь использует два managed image slots и отдельный каталог evidence
с timestamp. Уже принятый образ сначала импортируют в workbench;
см. [ARTIFACT-WORKFLOW.md](ARTIFACT-WORKFLOW.md). Явно заданный
`LUNA_CANONICAL_REGRESSION_ROOT` оставляет прямой диагностический режим.

Suite проверяет:

1. Input integrity, шесть negative checks verifier и проверки artifact cleanup/rotation.
2. `luna-state`, `luna-system-manager`, `luna-system-runtime`, `luna-init` и
   App Runtime/Manager/UserSession/CLI tests.
3. Сборку `.img`, GPT/filesystems, System Image, normal/Recovery DATA и musl closure.
4. Production OVMF boot и managers-before-boot-success gate.
5. Сохранение firmware surface в QEMU перед kernel.
6. Installed Bundle launch в активной UserSession, credentials/filesystem
   isolation, exit/signal, detached descendants, logout/relogin и persistence
   data/config после guest reboot; fixtures только в disposable layer.
7. Три независимых native login + 60-секундные Niri/Noctalia sessions.
8. Logout → новый login → второй desktop.
9. Реальный guest reboot, второй login/desktop, сохранение HOME/config marker
   files и той же system state revision.
10. Реальный guest shutdown с QMP `SHUTDOWN`/`guest=true`.
11. Неизменность checksum обычного исходного образа.

Lifecycle action helper включён только в disposable qcow2 слои общего образа.
Suite сохраняет serial/QMP/build logs и summary в `results.txt`; test overlays
и structure-verifier partitions удаляются и при обычном сбое. Managed controller
также очищает generated build trees. Failed experiment сохраняется во втором
слоте; эталон остаётся прежним. Для прямой диагностики отдельных cases можно
задать `LUNA_CANONICAL_KEEP_TEST_DISKS=1`; managed suite использует `0`.

Evidence текущего этапа:
`target/luna-alpha-stabilization-20261010/{evidence,inputs-final,regression-final}`.
Первый failed snapshot и диагностические logs сохранены отдельно.
Итог определяется `regression-final/results.txt`, финальной строкой
`Canonical Luna regression: PASS`.

### Измеренные результаты финального прогона

Исправленный input snapshot: 8510 nodes. Focused Rust tests: 26 PASS;
negative input-verifier checks: 6 PASS. Структура image и production OVMF,
firmware display и три отдельных 60-секундных desktop sessions прошли.
Открытие state в трёх login cases: 21, 19, 18 ms, revision 1.

Logout вернул login UI и второй рабочий стол; HOME/config markers прочитаны
повторно. Reboot подтверждён QMP `RESET`, `guest=true`: второй boot success,
повторная аутентификация и второй desktop после reboot прошли. Те же HOME/config
markers сохранены; state revision 1 до и после, открытие state 43/37 ms.
Shutdown подтверждён QMP `SHUTDOWN`, `guest=true`, вместе с PID 1 sync/poweroff
marker и успешной elogind operation. Финальный suite завершился
`Canonical Luna regression: PASS`; checksum ordinary image не изменился.

Ordinary image:
`target/luna-alpha-stabilization-20261010/regression-final/image/luna-0.1.0.img`.
SHA256: `f8d1a84f65d9c582150025eaaf02dec99f131f39a8f26b05617e8794376824ed`.
Оригинальный пользовательский image сохранил SHA256
`9ec4a51ccc4981c5e854f54e781dc50a69b31ac92c796485fb4d424de6173afc`.

## Следующий согласованный этап: приложения и Bundles

Пользователь явно выбрал этот этап следующим после стабилизации. Цель — рабочая
система приложений для системных и сторонних программ. Просто добавление manifest
рядом с Linux executable недостаточно.

Использовать существующие boundaries:

```text
.lbp → luna-bundle → integrity/trust/authorization
     → luna-app-manager → stage/transaction/registration
     → UserSession → luna-app-runtime → ApplicationInstance
```

Нужен полный вертикальный сценарий: создать настоящий LBP1 Bundle с manifest,
resources и объявленным runtime; установить его в DATA; запустить через app
runtime в активной UserSession; проверить data/config persistence; обновить с
rollback при ошибке; удалить Bundle с сохранением пользовательских данных.
Затем применить этот путь к Alpha application baseline и внешнему приложению,
включая musl/native и отдельный glibc compatibility runtime при необходимости.

System Image остаётся `.squashfs`; boot-critical Luna components остаются его
immutable minimum. Kernel и `.init` не становятся application Bundles. Системные
provider приложения должны получать явные manifest/resources/runtime и lifecycle
integration, сохраняя единственный PID 1 и supervision в `luna-system-runtime`.

На момент завершения стабилизации реализация Bundle stage ещё не выполнялась.
Последующий этап хранения/CLI реализован и проверен отдельно:
[`STORAGE-STAGE.md`](../application-bundles/STORAGE-STAGE.md).
Directory installation и узкий static-musl launch реализуются следующими
самостоятельными этапами: [DIRECTORY-STAGE.md](../application-bundles/DIRECTORY-STAGE.md),
[LAUNCH-STAGE.md](../application-bundles/LAUNCH-STAGE.md). Последний документ
содержит актуальные результаты и ограничения запуска.
Hardware/audio/network
acceptance, Recovery boot и physical installation не подтверждаются этими тестами.

Проверенный начальный пробел перед этапом хранения: executable entrypoints
`components/system/luna-cli/src/main.rs`,
`components/system/luna-app-manager/src/main.rs` и
`components/core/luna-app-runtime/src/main.rs` содержали только comments.
Теперь CLI dispatch и filesystem installation backend работают через существующие
библиотеки; последующее соединение live runtime IPC с активной UserSession
описано в LAUNCH-STAGE.
Продолжать следует через эти boundaries, не создавая второй менеджер приложений.

## Память проекта

Первоначальную запись рабочего состояния в claude-mem отклонила automatic approval
review: прежнее разрешение было отнесено только к графическому аудиту. После этого
2026-10-10 пользователь явно разрешил сохранять технические итоги и решения
Project Luna в claude-mem.

`save_memory` подтвердил запись №4 в проекте `project-luna`:
`2026-10-10 Project Luna: Alpha stabilization verified; next application Bundle lifecycle`.
Она содержит проверенные результаты стабилизации, ограничения доказательств,
правила сохранения пользовательской работы и следующий согласованный этап Bundles.

Последующая диагностика установила причину ошибок поиска и HTTP 404: Codex plugin
13.35.0 обращался к worker 13.25.3, а старый Claude plugin снова запускал старый
worker. После резервного копирования базы обе установки синхронизированы на
13.35.0. Поиск прочитал запись №4 через index → context → details; задачи
стабилизации и Bundles успешно записаны и прочитаны, startup context показывает
открытый этап Bundles. Chroma подключился, backfill завершился без write failures.

Automatic observer пока не авторизован в Claude Code. Это отдельное ограничение:
явные сохранение, поиск и рабочий список проверены, автоматическое создание
заметок из общения не подтверждено. Итоги ремонта сохранены в записи №5.
Проверки и установленный Graphify описаны в
[MEMORY-AND-GRAPHIFY.md](../MEMORY-AND-GRAPHIFY.md).
