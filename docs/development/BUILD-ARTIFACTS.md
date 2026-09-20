# Сборочные артефакты Project Luna

**Статус:** нормативное правило для локальной Alpha-разработки
**Revision:** 2026-09-18

## Назначение

`dist/` является локальным рабочим пространством для кэшей, промежуточных файлов и свежих проверенных артефактов. Содержимое `dist/` не является источником кода и не должно использоваться как архив исторических сборок.

## Канонические выходы

Видимая часть `dist/` должна отражать объекты Project Luna, а не временные директории сборочной системы:

```text
dist/
├── luna-sys/                     # собранное дерево физического LUNA-SYS
├── luna-data/                    # собранное дерево физического LUNA-DATA
├── luna-<version>.squashfs       # непосредственный System Image
├── luna-pc.img                   # полный PC image: EFI + LUNA-SYS + LUNA-DATA + SWAP
├── kernel/
│   ├── linux-<version>.tar.xz   # cache исходников kernel
│   ├── <release>/bzImage         # свежий kernel artifact
│   └── current -> <release>
├── sources/                      # внешние исходники
├── logs/                         # свежие диагностические логи
└── .build/                       # только временные build inputs и промежуточные файлы
```

`dist/.build/` — техническая рабочая область и не является частью архитектуры Luna. В неё помещаются desktop payload staging, development sysroots, package cache, временные filesystem images и OVMF variables. Имена `desktop-root`, `system-root`, `data-root`, `system-partition`, `dev-root` и аналогичные не должны появляться как самостоятельные верхнеуровневые каталоги `dist/`.

Видимые `luna-sys/` и `luna-data/` должны соответствовать каноническим структурам из `docs/ARCHITECTURE.md`. System Image остаётся непосредственным `luna-X.Y.Z.squashfs`; служебные partition filesystem files не являются публичными Luna artifacts.

Исторические `kernel-*`, старые QEMU logs, OVMF VARS snapshots и старые PC/System images не хранятся в `dist/` после завершения диагностики.

## Правило воспроизводимости

Тест, который должен описывать текущий repository HEAD, обязан использовать только artifacts, созданные после текущей сборки. Старый artifact из `dist/` нельзя считать доказательством текущего source state.

Перед новой полной Alpha-сборкой допустимо удалить `dist/.build/` и старые конечные images, сохранив внешние source caches.

## Безопасность

Сборочные скрипты не должны писать в физические диски хоста. Установка `luna-pc.img` на устройство остаётся отдельной ручной операцией.

## Graphify

Локальный Graphify graph используется для dependency/impact navigation перед изменениями в boot/runtime коде. Graph является навигацией, а не нормативным источником: вывод всегда сверяется с текущими source files и контрактами.
## Verification

Минимальный порядок перед публикацией Alpha artifact:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
bash tools/check-luna-init-static.sh
bash tools/check-luna-static.sh
bash tools/test-luna-boot-ovmf.sh
```

После существенных изменений kernel/userspace выполняется новая сборка PC image; результаты предыдущего запуска не переносятся автоматически.
