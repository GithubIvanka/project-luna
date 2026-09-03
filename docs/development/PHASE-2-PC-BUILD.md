# Phase 2 — передача PC build

Это текущая заметка по линии Phase 2.

## Реализованная цепочка

```text
UEFI
 ↓
luna-boot.efi
 ↓
Linux x86_64 kernel
 ↓
ранний initramfs
 ↓
SYSTEM
 └── versioned SquashFS System Image
 ↓
DATA
 └── durable Luna state
 ↓
luna-system-runtime
 ↓
UserSession
 ↓
Wayland → niri → Noctalia
```

Системный userspace runtime собирается с musl. Выбор runtime приложения является типизированным свойством execution environment: `luna`, `glibc` или `bundle`.

## Что делает PC build

`tools/build-pc-image.sh` создаёт x86_64 UEFI/GPT image с EFI, SYSTEM и DATA, упаковывает `luna-boot.efi`, versioned SquashFS System Image, initramfs и Linux kernel, а также создаёт persistent DATA filesystem.

Сборщик никогда не пишет на физический диск. `tools/install-pc-image.sh` — отдельное явно подтверждаемое destructive действие.

## Пользовательский путь

Development image сохраняет обычный тихий boot path. После раннего userspace управление передаётся `luna-system-runtime`, который создаёт `UserSession`.

Отдельного `luna-session` компонента нет и не должно появляться. Графическая граница принадлежит `UserSession`:

```text
luna-system-runtime
  ↓
UserSession
  ↓
Wayland
  ↓
niri
  ↓
Noctalia
```

Наличие development fallback-инструментов не превращает shell или serial console в архитектурную границу Luna.

## Инварианты

- SYSTEM неизменяем и версионирован; DATA изменяем.
- System Image — непосредственно SquashFS.
- `.lbp` остаётся Bundle transport/archive format.
- `luna-system-runtime` — единственный системный runtime/supervisor.
- `luna-security` — authority политики.
- `luna-root-mapping` — слой mapping.
- `luna-namespace` — Linux namespace/materialization layer.
- `UserSession` — совмещённая пользовательская/сессионная сущность.
- System Image и kernel обновляются независимо.
- TTY/serial не является штатным desktop entry path.

## Следующие крупные проходы

1. Проверить PC image в QEMU и на реальном UEFI hardware.
2. Завершить runtime materialization для `glibc` и Bundle-private runtimes.
3. Добавить тонкую авторизацию устройств и фильтрованный `/dev`.
4. Завершить реальный graphical payload niri + Noctalia.
5. Довести установку Bundle до end-to-end `ApplicationInstance` launch.
6. Заменить development `pre_exec` setup на production-safe child-creation primitive.

Эта заметка дополняет `docs/ARCHITECTURE.md` и не переопределяет принятые архитектурные решения.
