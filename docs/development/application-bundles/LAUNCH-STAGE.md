# Запуск установленного Bundle — 2026-10-11

Исторический checkpoint первого static-musl subset. Текущее продолжение:
[DYNAMIC-WAYLAND-STAGE.md](DYNAMIC-WAYLAND-STAGE.md).

Ветка `luna-onefile-experiment`, HEAD `a1ea279`; работа остаётся uncommitted.
Предыдущий этап: [DIRECTORY-STAGE.md](DIRECTORY-STAGE.md).
Контракты: [Directory Bundle](../../contracts/DIRECTORY-BUNDLE-CONTRACT.md) и
[Application launch](../../contracts/APPLICATION-LAUNCH-CONTRACT.md).

## Реализованный путь

```text
authenticated Niri UserSession
  → luna app run ID VERSION -- ARGS
  → локальный socket luna-system-runtime
  → App Manager: receipt + immutable .lunaapp + ContentIdentity
  → installed ApplicationPlan + static ELF closure + MappingPlan
  → luna-security: отдельные exact-content Launch grants
  → luna-namespace: mappings, ограничения, user credentials
  → ApplicationInstance + существующий System Runtime supervisor
```

App Runtime владеет экземпляром; System Runtime создаёт и наблюдает процесс
через `ApplicationProcessHost`. Новый daemon, application init или PID namespace
не добавлен. `luna-init` остаётся единственным PID 1.

Запрос в `/run/luna/application.sock` содержит ID, Version и args. Пользователь,
session identity, credentials и выбранный DATA определяются runtime. `SO_PEERCRED`
проверяет UID и PID; PID должен принадлежать точной cgroup текущего desktop.
Процесс того же пользователя за пределами этой UserSession отклоняется.
Размер TOML-запроса, число клиентов, args и время чтения ограничены. До 16
authenticated clients и до четырёх denied clients имеют отдельные budgets;
отклонённые peers не занимают места разрешённых запросов. Denied input дренируется
в ограниченных пределах перед ответом, без TOML parsing или launch.

Manager повторно проверяет registered directory и держит registry lock до
синхронного spawn/exec, закрепляющего разрешённые mounts. Install trust не даёт
Launch trust. Отсутствие/несоответствие grants отклоняет запуск до создания
private data и namespace.

## Явная политика запуска

Текущий локальный adapter читает root-owned файл
`LUNA-DATA/system/config/application-launch.toml` через закреплённый directory fd.
Config parent и файл не должны допускать group/other writes; файл должен быть
regular, без symlink и не более 64 KiB. Никакие grants не создаются при
install автоматически. Пример для заранее установленного статического Luna CLI:

```toml
format = 1

[[application]]
user = "luna"
id = "org.luna.cli"
version = "0.1.0"
content_identity = "<64 lowercase hex characters from verified Bundle>"
runtime = "musl"
execute = true
private_data = true

[[application.path]]
logical = "/bin/luna"
access = ["read", "execute"]

[[application.path]]
logical = "/data"
access = ["read", "write"]

[[application.path]]
logical = "/config"
access = ["read", "write"]
```

Placeholder identity нужно заменить проверенным digest. Для user/ID/version/content
требуется ровно один grant. Adapter передаёт grants существующему
`StaticPolicyAuthority`; policy authority остаётся `luna-security`.
Обновление содержимого требует нового exact-content Launch grant.
Management-команды остаются offline: выполнять их на DATA действующего runtime
нельзя из-за ownership persistent state. Live install/update IPC ещё отсутствует.

## Изоляция и lifecycle

- Первый subset: x86_64, static ELF без interpreter/shared dependencies,
  native `RuntimeSpec::Musl`, application без unresolved Bundle dependencies и
  capabilities. Тестовые программы действительно собраны musl. Сам статический
  ELF header не доказывает, какой libc пользовался произвольный сторонний writer.
- Payload доступен только через объявленные read/execute mappings. Служебные
  targets `/data`, `/config`, `/proc`, `/sys`, `/dev`, `/tmp`, `/run`, `/.luna`
  зарезервированы. Внешний caller не передаёт physical source paths.
- `/data` и `/config` указывают на
  `LUNA-DATA/users/<user>/{data,config}/<application-id>`; новые application
  directories имеют `0700` и user UID/GID. Существующее содержимое не chown-ится.
  DATA/users должен принадлежать root, user container и data/config — пользователю.
- Каждый процесс получает отдельный mount namespace, минимальный runtime profile,
  credentials пользователя, `no_new_privs`, ограничения namespace backend и
  очищенное окружение. Non-stdio descriptors закрываются на exec.
- System Runtime выделяет cgroup v2 каждому desktop и каждому приложению.
  `cgroup.kill` завершает также fork/setsid descendants; освобождение namespace
  следует после опустошения группы. Logout завершает приложения до нового login.
  Без доступной descendant boundary Bundle broker не открывается.
- CLI ждёт результат: обычный exit code либо `128 + signal`; launch/authorization
  error возвращает `1`. Stdio приложения пока `/dev/null`: интерактивные терминалы
  и GUI resource/socket handoff здесь не реализованы.
- Ошибки cleanup сохраняются отдельно от process outcome. Failed instance с живым
  handle остаётся до завершения cleanup; доставленные failed-start metadata удаляются,
  сохраняя первую диагностику до ответа caller.

## Проверка

QEMU acceptance актуального userspace выполняется через двухслотовый workbench:

```sh
python3 tools/canonical-luna-workbench.py run build
# Повторить проверку того же экспериментального образа:
python3 tools/canonical-luna-workbench.py run test
```

`tools/test-canonical-luna-bundle-launch.py` устанавливает три fixture Bundles
только в disposable qcow2 layer. Test-only policy и bootstrap не попадают в
production image. Сценарий проверяет настоящие musl ELF и Luna CLI, deny по
UID/session/content без private-data side effects, credentials/изоляцию/fds,
exit/signal, detached descendant cleanup, logout/relogin и guest reboot с
installed registry и persistent data/config. Исходный raw image сверяется SHA256.

Installed Bundle UserSession acceptance: **PASS**, 11 сценариев/markers и три
настоящие authenticated sessions. Evidence:
`target/canonical-workbench/evidence/20261011-175953-392615/bundle-launch/results.txt`
и `serial.log`. Установка, denied callers/content, настоящее исполнение probe и
Luna CLI, credentials/namespace/Landlock, exit/signal, detached descendant cleanup,
logout/relogin и reboot с registry/data/config приняты. Исходный raw image сохранён;
disposable test overlay удалён.

Полный canonical suite: **PASS**. Приняты input/negative/artifact/runtime/init
gates, свежая сборка, raw structure/ownership, production boot, firmware display
(17 samples), Bundle launch, три независимых 60-second Niri/Noctalia sessions,
logout/relogin, настоящие guest reboot/shutdown и source-image checksum.
`target/canonical-workbench/evidence/20261011-175953-392615/results.txt` фиксирует
все gates. Новый эталон — slot `a`, прежний эталон сохранён в slot `b`.
SHA256 нового образа:
`4c0c6459712b8b4f3c85d4740b4260fef8294d5e783a5c94e9a267ef40530a10`.
В production image нет test-only fixtures/grants/bootstrap: они проверялись
в отдельном disposable layer. Временные overlays/build trees очищены workbench;
serial/QMP/results и проваленные диагностические прогоны сохранены.

Host verification текущего прогона: 90 focused musl tests (один privileged test
ignored), 25 state/system runtime tests, 4 init tests; 26 artifact safety/rotation
checks, 6 negative input checks. Scoped Clippy `--all-targets --no-deps -D warnings`
для app-runtime/system-runtime/CLI проходит. Отдельно прошли 91 dependency tests.
Ignored privileged unit test не выдаётся за host PASS: реальный namespace/credential
путь проверяет QEMU payload. Independent read-only review не нашёл блокеров в
последней дельте; diagnostic FD закрывается при exec, denied peers не получают
launch и не занимают authorized budget.

Первый прогон выявил неверные DATA inode owners: `mkfs.btrfs --rootdir` использовал
libc `nftw` metadata, обходившие fakeroot. Builder получил build-only stat bridge,
`umask 022` и независимую проверку фактически записанных Btrfs inode owners/modes
до публикации `.img`; structure test повторяет её из итогового диска.
Мост не включён в Luna userspace и не меняет существующие DATA. Host extraction
для проверки содержимого использует fakeroot, без реального chown к guest UID.

Следующий диагностический прогон дошёл до filesystem enforcement и выявил
отключённый Landlock в старом выбранном kernel. `CONFIG_SECURITY_LANDLOCK=y`
добавлен в fragment; kernel builder проверяет результат `olddefconfig`.
Новая конфигурация отличается только Landlock и его зависимостью `SECURITY_PATH`.
Новый kernel собран отдельно; старые kernel/inputs и эталон сохранены.
SHA256 нового `bzImage`:
`cde0d24822e3090c71cc0fbec410b8d5f2d76e2e351e41175a638f283cc4bb8c`.
Snapshot `target/luna-app-launch-20261011/inputs-landlock` содержит прежние
проверенные provider inputs и новое ядро/config. Старые hardlinks перед заменой
kernel/lock разорваны; первоначальный snapshot не изменён.
Ограничения App Runtime не ослаблены: неполный Landlock ABI приводит к отказу.

Сохранённые неуспешные прогоны в `target/canonical-workbench/evidence/`:

| Прогон | Проверенная причина |
| --- | --- |
| `20261011-133308-433951` | Реальные DATA owners/modes не соответствовали root-owned registry/config contract. |
| `20261011-134331-997783` | Host structure harness пытался реально восстановить guest UID через `btrfs restore -mS`; исправлен fakeroot restore. |
| `20261011-173232-176148` | Установка/deny прошли; authorized spawn вернул EINVAL, требовалась точная child-setup диагностика. |
| `20261011-173701-480202` | CLOEXEC diagnostic FD показал strict Landlock incompatibility выбранного kernel; rejected peers также выявили socket reset до ответа. |

Каждый отказ оставлял эталон и исходный raw неизменными; overlays удалялись.
Эти evidence не являются принятием запусков приложений.

Финальная сверка: из 343 baseline source/harness entries 314 byte-identical,
29 намеренно изменены текущим этапом; отсутствующих и неожиданных изменений нет.
Семь новых source/test/doc files относятся к этому этапу. Branch/HEAD сохранены,
commits/reset/branch switch не выполнялись. Новые и прежние input snapshots
повторно verified; оба raw SHA совпадают с accepted/retained values. Синтаксис
Bash/Python, relative doc links и `git diff --check` проходят.
Evidence: `target/luna-app-launch-20261011/resume/evidence/final-verification.json`.
Два рабочих slots занимают около 3,7 GiB allocated, их evidence — 27 MiB;
свободно около 83 GiB. Исторический пользовательский образ вне ротации сохранён.

## Что остаётся

Динамический musl dependency closure с provenance/authorization, GUI/terminal
resource handoff, desktop discovery и Files presentation; затем реальные
Alpha application/provider Bundles и стороннее приложение. glibc compatibility,
signatures, dependency resolution и journalled migration legacy `.bundles`
остаются отдельными этапами. Hardware, Recovery boot и physical installation
не входят в эти доказательства.
