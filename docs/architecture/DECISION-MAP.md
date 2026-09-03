# Project Luna — карта решений и компонентов

**Статус:** навигационный документ для согласованности.  
**Источник:** `docs/ARCHITECTURE.md` и принятые решения.

Этот файл отвечает на вопрос: какой документ прочитать перед изменением конкретной архитектурной границы. Он не создаёт новых решений.

| Область | Принятые записи | Контракт компонента |
|---|---|---|
| Физическая модель DATA | ADR-0001 | `DISK-LAYOUT.md` |
| Boot / Recovery / Factory | ADR-0002 + актуальные boot decisions | `LUNA-BOOT.md`, `RECOVERY-FACTORY.md` |
| Logical root | ADR-0003 | `LUNA-ROOT-MAPPING.md` |
| Namespace / mappings | ADR-0004 + Phase 1.6 contracts | `LUNA-ROOT-MAPPING.md`, `LUNA-NAMESPACE.md` |
| Lifecycle приложений | ADR-0005 + runtime contract | `LUNA-APP-MANAGER.md`, `LUNA-APP-RUNTIME.md` |
| UserSession/checkpoints | ADR-0006 + graphical-session decisions | `USER-SESSION.md`, `LUNA-SYSTEM-RUNTIME.md` |
| Bundle Format v1 | RFC-0002 + ADR-0007 | `LUNA-BUNDLE.md` |
| Graphical boot/login | решения от 2026-09-01 | `LUNA-LOGIN.md`, `USER-SESSION.md`, `LUNA-SYSTEM-RUNTIME.md` |
| Runtime taxonomy | runtime contract 2026-09-01 | `LUNA-APP-RUNTIME.md`, `LUNA-SYSTEM-RUNTIME.md` |
| Git/branch workflow | решение Git workflow 2026-09-01 | `docs/development/` |

## Историческая оговорка

В репозитории есть хронологические ADR и более поздние решения с датами. Поздний документ может уточнять реализацию, не отменяя несвязанные старые решения. Явное устаревание должно быть указано в новом решении.

## Перед изменением компонента

Сначала прочитайте component contract, затем перечисленные здесь decision records. Если документы противоречат друг другу, конфликт нельзя разрешать молча: его нужно явно зафиксировать и решить документированным изменением.