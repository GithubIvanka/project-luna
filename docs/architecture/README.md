# Project Luna — Документация архитектуры компонентов

**Статус:** канонический индекс документации компонентов
**Источник архитектуры:** `docs/ARCHITECTURE.md`

Назначение этого каталога — дать отдельный, компактный и однозначный документ для каждой архитектурной границы. Благодаря этому разработчику не требуется каждый раз восстанавливать весь контекст из огромного SoT.

## Иерархия источников

```text
docs/ARCHITECTURE.md
        ↓
принятые решения / RFC / ADR
        ↓
docs/architecture/components/*
        ↓
реализация
```

Документ компонента описывает текущий контракт и не имеет права молча создавать новую архитектуру. При конфликте с `docs/ARCHITECTURE.md` сначала устраняется конфликт документации, затем продолжается реализация.

Исторические phase/archive документы нужны для traceability и не являются источником новых решений.

## Физическая и загрузочная архитектура

- `DISK-LAYOUT.md` — EFI / SYSTEM / DATA / SWAP, файловые системы, владельцы и структура хранения.
- `LUNA-BOOT.md` — `luna-boot.efi`, выбор загрузки, совместимость, fallback и Boot Menu.
- `SYSTEM-IMAGE.md` — SquashFS System Image, manifest, версии, совместимость с kernel и retention.
- `RECOVERY-FACTORY.md` — Recovery и Factory среды и их границы.

## Базовые userspace-компоненты

- `LUNA-COMMON.md`
- `LUNA-FS.md`
- `LUNA-ROOT-MAPPING.md`
- `LUNA-NAMESPACE.md`
- `LUNA-CONFIG.md`
- `LUNA-SECURITY.md`
- `LUNA-STATE.md`
- `LUNA-EVENT.md`
- `LUNA-BUNDLE.md`

## Management-компоненты

- `LUNA-APP-MANAGER.md`
- `LUNA-SYSTEM-MANAGER.md`
- `LUNA-UPDATE-MANAGER.md`
- `LUNA-KERNEL-MANAGER.md`
- `LUNA-DEVICE-MANAGER.md`

## Runtime / session / login

- `LUNA-SYSTEM-RUNTIME.md`
- `USER-SESSION.md`
- `LUNA-APP-RUNTIME.md`
- `LUNA-LOGIN.md`

## Пользовательские и аппаратные boundary

- `LUNA-CLI.md`
- `LUNA-FILES.md`
- `LUNA-AUDIO.md`
- `LUNA-NETWORK.md`
- `LUNA-BLUETOOTH.md`

## Сквозные правила

1. Компонент нельзя придумывать только потому, что реализация стала неудобной.
2. Linux utility, daemon или helper не становится Luna-компонентом автоматически.
3. `UserSession` — граница пользовательской сессии. Отдельного `luna-session` или `luna-run-session` нет.
4. `luna-system-runtime` — единственный системный runtime/supervisor и координирует `UserSession`.
5. `luna-app-runtime` владеет выполнением и lifecycle `ApplicationInstance`.
6. Manager владеет состоянием/операциями своего домена; `luna-update-manager` выполняет state-changing update transactions.
7. `luna-security` владеет authorization policy; mapping и filesystem не выдают права.
8. `luna-boot.efi` — отдельная UEFI boundary вне обычного userspace workspace.
9. System Image — SquashFS. `.lbp` — другой формат и не может использоваться вместо System Image.
10. Новая архитектурная boundary требует принятого решения до её появления в crate map.

## Статусы

- **Принято** — решение явно принято.
- **Реализовано** — в репозитории есть значимая реализация.
- **Интеграция** — реализация существует, но цепочка ещё не завершена.
- **Запланировано** — направление принято, реализация ещё отсутствует.
- **Открытый вопрос** — решение пока не фиксировано; код не должен его угадывать.

Документы обязаны различать эти статусы.
