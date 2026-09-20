# Project Luna — текущий статус

**Revision:** 2026-09-19

Статус отражает фактическое состояние локальной ветки `alpha-development` и подтверждённые проверки. Архитектурный источник истины: `docs/ARCHITECTURE.md`.

## Текущая стадия

Project Luna находится на стадии **Alpha integration / bring-up**. Архитектурная модель и основные Rust-компоненты уже сформированы; текущая работа сосредоточена на сборке воспроизводимого bootable образа и замыкании end-to-end пути.

## Подтверждено

- Rust workspace из 23 пакетов проходит `cargo check --workspace`.
- Workspace tests проходят; privileged namespace/Landlock checks остаются `ignored`, когда окружение не предоставляет необходимые права.
- `cargo clippy --workspace --all-targets -- -D warnings` проходит.
- `cargo fmt --all -- --check` и `git diff --check` проходят.
- `luna-boot.efi` собирается для `x86_64-unknown-uefi`.
- Компонентный OVMF smoke на свежем kernel artifact подтверждает `UEFI -> luna-boot -> Linux kernel -> LunaBootHandoffV1 -> luna-init -> luna-system-runtime`.
- Kernel принимает handoff и memory-resident `luna-init`; direct-PID1 путь подтверждён на свежем собранном artifact. Это ещё не E2E-проверка полного `luna-pc.img`.
- PC image builder создаёт GPT с `EFI`, `LUNA-SYS`, `LUNA-DATA` и `SWAP`; System Image является непосредственным SquashFS с соседним TOML manifest.

## Текущие блокеры

- Свежий kernel artifact из текущего `alpha-development` HEAD уже собран и прошёл компонентный OVMF smoke; следующий обязательный шаг — загрузить через OVMF именно собранный `luna-pc.img`.
- Полный graphical path и настоящий E2E для собранного `luna-pc.img` ещё не закрыты. Предыдущий компонентный OVMF fixture доходил до `luna-system-runtime`, после чего получал `greetd backend is missing`, потому что в fixture не входили реальные desktop-пакеты.
- Полный PC image теперь является обязательной основой для настоящего end-to-end OVMF smoke path; старые `dist` artifacts больше не используются как доказательство current HEAD.
- Production hardening `luna-app-runtime`, полноценное enforcement security, update/rollback, recovery и hardware validation остаются следующими этапами.

## Исправлено в текущей уборке

- Удалён неиспользуемый `luna-init/src/bootstrap.rs`; bootstrap semantics теперь находятся в одном implementation path.
- Удалён пустой `components/system/luna-login` и соответствующая битая документационная ссылка.
- `dist/` очищен от старых kernel worktrees, старых PC images, QEMU logs и повторных build trees; сохранён только актуальный cache/source слой, необходимый для пересборки.
- Дальнейшая проверка строится на fresh artifacts, а не на старых вариантах из `dist`.

## Следующий вертикальный slice

Физическая структура компонентов теперь разделена по архитектурной роли:

```text
components/core/
components/system/
components/apps/
components/external/providers/
components/external/libraries/
```

Для boot/GUI следующий vertical slice:

```text
current HEAD
  -> kernel
  -> luna-boot
  -> luna-init
  -> System Image
  -> LUNA-DATA
  -> luna-pc.img
  -> QEMU/OVMF
  -> luna-system-runtime
  -> UserSession
      -> native InputBackend
      -> native SeatController
      -> native DRM/KMS backend
      -> minimal graphical SessionUI/compositor
  -> optional DATA desktop provider (Niri + Noctalia)
```

Niri/Noctalia остаются полноценными DATA providers; их не требуется урезать до размеров boot-critical GUI.
## Agent infrastructure

Luna Agent использует sequential task queue, Git-tree guard и allow-listed verification. Локальный Graphify graph используется как навигация и impact-analysis; source code и current contracts остаются нормативными.

Текущая задача очереди — `DESKTOP-000`; autonomous runner должен возобновляться только после фиксации текущих repository changes в Git.
