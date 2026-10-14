# Application Bundles: этап хранения

Исторический завершённый этап 2026-10-10. Его layout с вложенным `.lbp`
заменён в текущем коде новым
[`Directory stage`](DIRECTORY-STAGE.md) от 2026-10-11; старые evidence/fixtures
сохранены. Описанные ниже результаты относятся именно к первоначальному backend.

Дата: 2026-10-10. Ветка `luna-onefile-experiment`, HEAD `a1ea279`.
Изменения не закоммичены. Предыдущие boot/desktop изменения и untracked harness
сохранены; snapshot перед работой находится в
`target/luna-bundles-20261010/evidence/`.

## Реализовано

```text
manifest + payload → luna-bundle LBP1 → inspect + ContentIdentity
  → luna-security: exact content Install trust + Application Manage
  → luna-app-manager: stage + filesystem checkpoint
  → luna-update-manager + luna-state: prepare/checkpoint/apply/verify/commit
  → immutable installed Bundle
```

`FilesystemApplicationManager` использует только явно выбранный DATA root,
проверяет owner/mode, отклоняет symlink roots и держит exclusive advisory lock.
Управляемые каталоги не доступны для записи другим пользователям или группе.
В native системе оператором этой границы должен быть системный runtime с
доверенными credentials; offline adapter позволяет работать с собственным
тестовым DATA без root.

Каталог реализации:

```text
DATA/system/apps/.bundles/
├── <BundleId>/<Version>/
│   ├── bundle.lbp
│   ├── manifest.toml
│   ├── receipt.toml
│   └── payload/
├── .transactions/<UpdateId>/operation.toml
├── .garbage/<UpdateId>/...
└── .lock
```

Bundle опубликован одним `rename`; файлы и каталоги синхронизированы перед
фиксацией. Файлы имеют `0444`/`0555`, каталоги payload — `0555`.
Это защита от обычных writes, а не обещание fs-verity или защиты от самого
доверенного владельца. Verify сверяет транспортный ContentIdentity, receipt,
manifest, каждую payload file и полный inventory, включая implicit directories,
modes, owner и отсутствие links.

Metadata checkpoint полностью готовится в `.garbage` до атомарного переноса в
`.transactions`. Staging после этого остаётся recoverable даже при прерывании.
Rollback отменяет только текущую физическую версию/операцию, не заменяет старую
версию и не посещает `users/`. Terminal checkpoint переносится в `.garbage`
до удаления, поэтому прерывание cleanup не уничтожает необходимую metadata.
При неоконченной операции registry access закрыт до `recover`.

При переносе между родительскими каталогами Linux требует write permission на
перемещаемом каталоге для изменения `..`. Поэтому backend временно открывает
только внешний каталог версии (`0755`), выполняет rename/fsync и возвращает
`0555`; payload остаётся read-only. Журнал и exclusive lock сохраняют границу
операции, recovery восстанавливает mode после прерывания.

`UpdateEngine` отклоняет повторное использование UpdateId, неизвестную фазу
журнала и recovery plan, отличающийся от durable plan. Rollback включает текущую
операцию, если `apply` успел частично изменить файловую систему перед ошибкой.

Журнал использует существующий `luna-state.redb`. Backend принимает `StateStore`
в явном API; offline CLI открывает его непосредственно. Live runtime должен
передавать свой store через существующую границу supervision/будущий IPC,
без создания второго supervisor или параллельной authority.

## Команды

```bash
bash tools/build-luna-cli-bundle.sh /absolute/output/directory
target/x86_64-unknown-linux-musl/debug/luna bundle inspect /path/org.luna.cli-0.1.0.lbp
```

Recipe собирает реальное native musl приложение `luna`, включает executable и
ресурс `share/about.txt`, использует tracked manifest
`examples/bundles/luna-cli/manifest.toml`. Существующий output не перезаписывается.
После проверки Bundle скопировать его `content_identity` в команду:

```bash
luna app --offline-data-root /absolute/test-DATA install /path/app.lbp --trust-content <content_identity>
luna app --offline-data-root /absolute/test-DATA list
luna app --offline-data-root /absolute/test-DATA verify org.luna.cli 0.1.0
luna app --offline-data-root /absolute/test-DATA update org.luna.cli 0.1.0 /path/new.lbp --trust-content <new_content_identity>
luna app --offline-data-root /absolute/test-DATA remove org.luna.cli 0.1.0
luna app --offline-data-root /absolute/test-DATA recover
```

DATA root должен существовать и принадлежать оператору. Не использовать этот
offline путь одновременно с действующим runtime на том же DATA. Ни install,
ни discover, ни verify не выполняют приложение.

## Границы этапа

- Update устанавливает новую версию независимо и сохраняет предыдущую; default
  version selection для запуска пока отсутствует.
- Remove сохраняет пользовательские data/config, существующие providers вне
  `.bundles` и остальные версии.
- Подписи пока отклоняются: section hash не подменяет Ed25519 verification.
- Зависимости пока отклоняются с диагностикой. Нет автоматической загрузки.
- Parser limit при установке: 512 MiB transport; каждую compressed секцию
  дополнительно ограничивает объявленный decoded length и лимит кодека.
- Launcher в реальной активной UserSession, RuntimeSpec/ELF closure, persistent
  application mappings, live IPC, Alpha providers и стороннее приложение ещё
  предстоит соединить. Offline CLI не создаёт фиктивную активную сессию.

## Проверки

Финальная проверка текущего кода завершилась успешно. Evidence сохраняется в
`target/luna-bundles-20261010/`:

- `evidence/rust-tests.log`: 44 существующих Rust tests, 0 failures; пакеты
  `luna-bundle`, `luna-security`, `luna-state`, `luna-update-manager`,
  `luna-app-manager`, `luna-cli`, target `x86_64-unknown-linux-musl`, offline.
- `evidence/clippy.log`: `cargo clippy --offline` для CLI/AppManager с
  `-D warnings`, PASS.
- `evidence/runtime-check.log`: `cargo check --offline` для app-runtime и
  system-runtime, PASS. Форматирование, `bash -n` recipe и `git diff --check`
  также прошли.
- `runs/verified/results.txt`: 20 acceptance сценариев и итоговый COMPLETE;
  полный журнал — `runs/verified/acceptance.log`. Реальные Bundle artifacts:
  `runs/verified/artifacts/org.luna.cli-0.1.0.lbp` и `org.luna.cli-0.2.0.lbp`.
  Harness и fixtures находятся только в ignored `target/`; существующие
  пользовательские test scripts не изменены.

Acceptance покрывает детерминированную упаковку musl executable и resource;
сохранение существующего output; deny management/trust и неверного content pin;
install/list/verify; неизменяемость старой версии; повреждение bytes, modes,
inventory и links; независимое обновление и сохранение data/config; удаление
с сохранением providers; umask `077`/`027`; rollback при verification failure;
прерывание опубликованной установки и удаления с повторным открытием журнала;
ошибку durable write; disposable garbage; частично выполненный failed apply;
валидацию UpdateId/plan/phase; exclusive lock; небезопасный DATA и symlink escape;
повреждение транспорта; RFC `dependency` alias и minimum-system requirement.

Проверки обнаружили реальные ошибки с modes implicit directories при host umask
и cross-parent rename read-only каталога. Они исправлены, полный сценарий
повторён на окончательном коде. Независимый review дополнительно выявил риск
неполного pre-journal checkpoint; подготовка metadata перенесена в `.garbage`.

QEMU boot/desktop regression и физический диск в этом этапе не запускались:
текущий diff не меняет boot/init/desktop интеграцию.

Graphify обновлён по 134 code files из `components/`, без LLM или внешнего API:
2678 nodes, 7166 edges, 110 communities. `graphify-out` указывает на
`target/luna-graphify-bundles-20261010/graphify-out`; предыдущий snapshot сохранён.
Исходные manifests/AST и fingerprint текущих файлов позволяют проверять
актуальность графа. Документы семантически не индексировались.
Полный пересчёт заменил incremental snapshot; оба сохранены. В raw AST diagnostic
есть 392 связи без явно совпадающего endpoint и 53 объединяемые пары связей.
После нормализации в итоговом графе нет отсутствующих endpoints, но он не
сохраняет все типы параллельных связей. Подробности — `GRAPH-HEALTH.txt` рядом
с графом. Для решений используются проверенные source/contracts и тесты.

Проверка сохранности (`evidence/preservation.json`) подтвердила неизменность
35 исходных dirty/untracked entries. Остальные три — намеренные обновления
двух status documents и Graphify symlink; код boot/init/desktop и пользовательские
harness/scripts остались byte-identical. Ветка и HEAD не изменены.

Технические итоги сохранены в claude-mem как note #6
`2026-10-10 Project Luna: Bundle storage and CLI verified; active UserSession launch next`;
повторный поиск прочитал запись. Рабочий список закрывает только storage/CLI,
полный application lifecycle остаётся открытым. Automatic observer по-прежнему
не авторизован; его ошибка `Claude Code reported the observer as signed out`
не устранена этим этапом.

## Следующий этап

Соединить verified installed Bundle с существующим `luna-app-runtime` в активной
UserSession: RuntimeSpec, logical mappings, user data/config и exit status.
Проверить этот путь в canonical QEMU, затем применить его к Alpha providers и
стороннему приложению. Полный application lifecycle пока остаётся открытой задачей.
