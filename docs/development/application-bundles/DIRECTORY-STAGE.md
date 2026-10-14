# Application Bundles: directory stage

Дата: 2026-10-11. Ветка `luna-onefile-experiment`, HEAD `a1ea279`.
Изменения не закоммичены. Текущий этап реализует поправки пользователя к
установленному приложению; полный application lifecycle ещё не завершён.

## Что готово

```text
luna-cli.lbp → verify + exact Install trust + Application Manage
  → stage → UpdateEngine journal → immutable registration
  → luna-cli.lunaapp/{manifest.toml,integrity.toml,payload/}
```

Вложенного `.lbp` больше нет. Обычное имя не содержит добавленного ID/Version;
metadata имеет формат TOML. `share/about.txt` — только пример обычного ресурса.
Приватный receipt с UpdateId и exact identity находится снаружи `.lunaapp`.
Версии сосуществуют в opaque private slots и сохраняют одинаковое обычное имя.
Удаление по-прежнему сохраняет user data/config и существующие loose providers.

`luna-bundle::directory` проверяет самостоятельную directory representation,
сохраняя RFC-0002 ContentIdentity. Общий canonical TAR writer используется для
LBP и потокового directory digest. Reader закрепляет пути через `openat`,
проверяет inode/type/owner/mode/links/size, отклоняет special files через `O_PATH`
до открытия для чтения и ограничивает actual bytes, depth и inventory.
Short stream отклоняется, хотя TAR library сама размер не проверяет.

`luna bundle inspect` понимает transport и directory, показывает `name` и
`representation`. Suffix сам по себе не означает валидный Bundle, регистрацию
или разрешение запуска. CLI использует существующие backend APIs.

Контракт: [`DIRECTORY-BUNDLE-CONTRACT.md`](../../contracts/DIRECTORY-BUNDLE-CONTRACT.md).
История решения: [`INSTALLED-BUNDLE-PROPOSAL.md`](INSTALLED-BUNDLE-PROPOSAL.md).
Первый storage этап и старые evidence: [`STORAGE-STAGE.md`](STORAGE-STAGE.md).

## Проверки

Evidence: `target/luna-bundles-20261011/`.

- `evidence/rust-tests.log`: **54 Rust tests**, 0 failures. Включает 10 новых
  regression tests: 9 directory tests и exact-size TAR stream test. Native musl,
  locked, offline. Старые тесты не удалены и не ослаблены.
- `evidence/clippy.log`: CLI/AppManager, `--all-targets -- -D warnings`, PASS.
- `evidence/runtime-check.log`: app-runtime/system-runtime compilation, PASS.
- `runs/verified/results.txt`: **24 acceptance scenarios + COMPLETE**.
  Полный журнал: `runs/verified/acceptance.log`.
- `bash -n` build recipe, форматирование изменённых Rust files и
  `git diff --check`: PASS.
- `evidence/recipe.log` и `evidence/demo.json`: build recipe создаёт настоящий
  `demo-artifacts/luna-cli.lbp`; отдельный `demo-DATA` содержит установленный и
  проверенный `luna-cli.lunaapp` с той же ContentIdentity.
- Независимый read-only review обнаружил и помог исправить portable-read race
  и short TAR stream. Финальный повторный review новых значимых дефектов не
  обнаружил; reviewer не запускал тесты и не менял файлы.

Acceptance проверяет реальный musl CLI Bundle, deterministic pack/no overwrite,
trust/Manage deny, collision, install/list/verify, bytes/modes/inventory/links,
independent update, удаление с сохранением data/config, umask `077`/`027`,
verification rollback, interrupted published install/remove, durable journal
failure, pre-journal garbage, exclusive lock и unsafe DATA roots.

Дополнительные проверки подтверждают ordinary filename, отсутствие `.lbp` в
registered tree, одинаковую identity у исходного transport, установленного и
переименованного standalone directory; отклонение папки с suffix, неверных
format/version/digest, неожиданного transport entry, сохранение legacy storage.
Исходные `.lbp` acceptance fixtures удалены **только в собственном новом run**
перед последней проверкой — обе directory copies продолжают проходить verify.
Старые artifacts/fixtures 2026-10-10 сохранены.

## Воспроизведение

```bash
bash tools/build-luna-cli-bundle.sh /absolute/new-output-directory
# output: luna-cli.lbp; существующий output не перезаписывается
luna bundle inspect /absolute/new-output-directory/luna-cli.lbp
luna app --offline-data-root /absolute/new-test-DATA install /absolute/new-output-directory/luna-cli.lbp --trust-content <content_identity>
luna app --offline-data-root /absolute/new-test-DATA list
luna app --offline-data-root /absolute/new-test-DATA verify org.luna.cli 0.1.0
luna bundle inspect /absolute/new-test-DATA/system/apps/.bundles-v2/<slot>/luna-cli.lunaapp
```

DATA root должен заранее существовать и принадлежать оператору. Это offline
adapter; действующий runtime не должен пользоваться той же state database.
Команды не создают фиктивную UserSession и не исполняют payload.

## Ограничения и следующий этап

- Старое `.bundles/<id>/<version>` сохранено. При legacy installation/pending
  checkpoint новый manager останавливается с диагностикой; автоматическая
  journalled migration ещё не реализована.
- Первый directory v1 поддерживает application с canonical TAR текущего writer.
  RESOURCES/SIGNATURE, иной валидный TAR encoding и unresolved dependencies
  отклоняются явно, без замены ContentIdentity и без молчаливых downloads.
- Установка из standalone directory, ассоциация `.lunaapp`, Files discovery,
  иконки и Show Contents ещё не реализованы.
- Следующий основной этап: active UserSession → verified Bundle → RuntimeSpec
  и ELF closure → MappingPlan → authorization → luna-app-runtime, persistent
  data/config и exit status. Затем canonical QEMU acceptance и упаковка Alpha
  providers/стороннего приложения. В этом этапе QEMU/hardware не запускались.

Этот следующий static-musl launch этап ведётся отдельно:
[LAUNCH-STAGE.md](LAUNCH-STAGE.md). Его реализация и доказательства не входят
в исторические 54 tests/24 offline scenarios этой страницы.

## Сохранность работы

Перед началом сохранены hashes 331 tracked/untracked files и dirty diff:
`evidence/before-files.json`, `evidence/before.patch`. Итоговая сверка —
`evidence/preservation.json`. Изменения ограничены Bundle/AppManager/CLI,
build recipe, связанными contracts/status docs и актуализацией navigation graph.
Boot/init/desktop scripts и пользовательские harness не изменялись.
Новых commit, reset, branch switch и физических disk operations нет.

Graphify обновлён на текущие sources: 136 code files, 2740 nodes, 7356 edges,
111 communities, 0 dangling endpoints, 0 LLM tokens. Root `graphify-out`
указывает на `target/luna-graphify-bundles-20261011/graphify-out`; предыдущий
snapshot и query memories сохранены. Это structural navigation, не semantic
memory всех обсуждений.

Ручная claude-mem запись #7 сохранена и прочитана обратно; рабочие задачи
обновлены. Automatic observer по-прежнему сообщает signed out и не сохраняет
сессию автоматически. Сервис не перезапускался и credentials не изменялись.
