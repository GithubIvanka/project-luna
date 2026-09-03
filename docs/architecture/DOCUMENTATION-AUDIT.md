# Project Luna — аудит документации

**Дата:** 2026-09-03  
**Ветка:** `develop`  
**Источник истины:** `docs/ARCHITECTURE.md`

## 1. Что проверено

В рамках актуализации проверены и синхронизированы:

- `docs/ARCHITECTURE.md`;
- `README.md`;
- `STATUS.md`;
- `ROADMAP.md`;
- `docs/architecture/README.md`;
- `docs/architecture/CRATE-MAP.md`;
- `docs/architecture/COMPONENT-STATUS.md`;
- `docs/architecture/DECISION-MAP.md`;
- `docs/architecture/DISK-LAYOUT.md`;
- `docs/architecture/LUNA-BOOT.md`;
- `docs/architecture/SYSTEM-IMAGE.md`;
- `docs/architecture/RECOVERY-FACTORY.md`;
- `docs/architecture/OS-CAPABILITY-GAPS.md`;
- `docs/architecture/components/*` — текущие component contracts;
- `boot/luna-boot/BOOT-CONTRACT.md`.

Также добавлены Phase 0 draft contracts в `docs/contracts/`.

## 2. Основные найденные расхождения

### Boot contract

Старый `boot/luna-boot/BOOT-CONTRACT.md` описывал переход сразу к Factory при normal target failure. Это было слишком грубо по сравнению с принятым soft-fallback решением.

Исправлено: failure System Image должен, где это возможно и безопасно, использовать предыдущую совместимую версию без полного reboot; kernel-level failure может потребовать reboot; затем применяются другие usable fallback choices, Factory и Recovery.

### Текущий статус `luna-boot`

Старая документация местами описывала discovery и fallback как будущую работу, хотя текущий `STATUS.md` фиксирует наличие GUI splash, dynamic SYSTEM image/kernel discovery, manifest validation, compatible-kernel selection, ordered Boot Menu и soft fallback.

Исправлено в Source of Truth, README и STATUS.

### Runtime

Документация приведена к единой модели:

```text
luna-system-runtime
    ↓
UserSession
    ↓
luna-app-runtime
    ↓
ApplicationInstance
```

`RuntimeKind`/`RuntimeSpec` остаются свойствами execution environment. Generic `luna-runtime` не создаётся.

### Application security

Уточнена обязательная последовательность:

```text
ApplicationPlan
 ↓
MappingPlan
 ↓
luna-security
 ↓
luna-namespace
 ↓
ApplicationInstance
```

Requested permissions/capabilities не являются grants. Ошибка security приводит к fail closed.

### System Image

Все актуальные документы используют единый инвариант: System Image — непосредственно SquashFS `luna-X.Y.Z.squashfs` с соседним manifest `luna-X.Y.Z.toml`. `.lbp` и `.squashfs` не смешиваются.

## 3. Проверка component contracts

Текущие component contracts проверены на наличие четырёх обязательных аспектов:

1. назначение и ownership;
2. явные запреты/границы;
3. зависимости и интеграционный поток;
4. оставшаяся работа/open questions.

Недостающая конкретика была добавлена там, где она следовала из уже принятых решений. Новые архитектурные решения от имени отсутствующих спецификаций не придумывались.

## 4. Русификация

Текущая проектная документация, используемая для реализации, приведена к русскому языку. Имена crate, API, файлов, команд, протоколов, форматов и технических терминов сохранены в оригинальном виде там, где это необходимо для точности.

Исторические архивы не переписываются ради косметического перевода: они сохраняют исходную форму записи решений и используются для трассируемости.

RFC и decision records являются нормативными/историческими источниками своего уровня. Их перевод не должен менять их смысл; при необходимости русская навигация к ним обеспечивается через текущий SoT и карты решений.

## 5. Phase 0

Добавлены отдельные черновики:

```text
docs/contracts/SYSTEM-IMAGE-CONTRACT.md
docs/contracts/KERNEL-CONTRACT.md
docs/contracts/BOOT-STATE-CONTRACT.md
docs/contracts/BOOT-HANDOFF-CONTRACT.md
docs/contracts/FAILURE-RECOVERY-CONTRACT.md
```

Они специально помечены как черновики. Их наличие не означает автоматического изменения принятой архитектуры.

## 6. Целевое состояние документации

```text
ARCHITECTURE.md
      ↓
architecture indexes / status / roadmap
      ↓
component contracts
      ↓
Phase 0 / RFC / ADR contracts
      ↓
implementation
```

Если реализация обнаруживает конфликт с SoT, сначала меняется архитектурный контракт через явное решение.
