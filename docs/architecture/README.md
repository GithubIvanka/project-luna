# Архитектура Project Luna

Эта директория содержит актуальные архитектурные документы Project Luna. Главным источником истины остаётся `docs/ARCHITECTURE.md`.

## Как читать документацию

```text
ARCHITECTURE.md
    ↓
тематические архитектурные документы
    ↓
контракты компонентов
    ↓
реализация crate/бинарника
```

Документы не должны переопределять принятые решения из Source of Truth.

## Главные документы

- `../ARCHITECTURE.md` — единый текущий Source of Truth;
- `CRATE-MAP.md` — карта реальных workspace boundaries;
- `COMPONENT-STATUS.md` — зрелость и состояние компонентов;
- `DECISION-MAP.md` — навигация по принятым решениям;
- `DISK-LAYOUT.md` — физическая и логическая модель хранения;
- `SYSTEM-IMAGE.md` — семантика System Image;
- `LUNA-BOOT.md` — граница UEFI bootloader;
- `RECOVERY-FACTORY.md` — Factory/Recovery;
- `OS-CAPABILITY-GAPS.md` — что ещё необходимо для полноценной PC ОС;
- `DEVELOPMENT-ROADMAP.md` — порядок дальнейшей реализации.

## Контракты Phase 0

Черновики находятся в `docs/contracts/`:

```text
SYSTEM-IMAGE-CONTRACT.md
KERNEL-CONTRACT.md
BOOT-STATE-CONTRACT.md
BOOT-HANDOFF-CONTRACT.md
FAILURE-RECOVERY-CONTRACT.md
```

Они пока не считаются принятыми архитектурными решениями.

## Компоненты

`components/` содержит контракт каждого текущего Luna component. Каждый такой файл должен отвечать на одинаковые вопросы:

1. зачем нужен компонент;
2. что ему принадлежит;
3. чего он не должен делать;
4. какие входы/выходы и зависимости существуют;
5. как обрабатываются ошибки;
6. что уже реализовано;
7. что остаётся открытым.

## Исторические материалы

`archive/`, phase records, ADR и RFC нужны для трассируемости. Они не являются заменой текущему Source of Truth. Исторические тексты не переписываются только ради перевода, если это разрушит их функцию исторической записи.
