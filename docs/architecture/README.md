# Архитектурная документация

Эта директория содержит актуальное подробное описание архитектуры Project Luna. Главный источник истины находится в `docs/ARCHITECTURE.md`.

## Порядок чтения

Сначала прочитайте `../ARCHITECTURE.md`, затем используйте документы с описанием системных потоков и отдельные документы компонентов.

## Основные документы

- `ACCEPTED-DECISIONS.md` — текущая сводка принятых решений.

## Системные потоки

- `DISK-LAYOUT.md` — физические разделы и каноническая структура каталогов.
- `BOOT-PATH.md` — полный путь от UEFI до системного runtime и пользовательской среды.
- `BOOT-STATE.md` — постоянные boot targets, NVRAM marker и подробный progress в RAM.
- `RECOVERY.md` — Recovery, Factory и Recovery DATA.
- `SYSTEM-IMAGE.md` — System Image и его модель совместимости.
- `LOGICAL-ROOT.md` — RAM-backed logical root и materialization ресурсов.
- `APPLICATION-EXECUTION.md` — запуск, безопасность и изоляция приложения.
- `RUNTIME-LIFECYCLE.md` — владение процессами, UserSession и lifecycle приложений.
- `SECURITY-MODEL.md` — trust, permissions и authorization.
- `UPDATE-LIFECYCLE.md` — lifecycle обновления и rollback.
- `COMPONENT-MAP.md` — карта компонентов и границы ответственности.
- `MINIMAL-BOOT-BASELINE.md` — минимальная модель boot/session и принцип OneFileLinux baseline.

Для правил сборки и работы с локальными artifacts используйте `docs/development/BUILD-ARTIFACTS.md`.

## Документы компонентов

Файлы в `components/` описывают по одному активному архитектурному компоненту. Каждый документ должен содержать:

- назначение и границу;
- обязанности и ограничения ответственности;
- lifecycle;
- Cargo/архитектурные зависимости;
- публичный API и контракт;
- взаимодействие с другими компонентами;
- поведение при ошибках;
- текущий статус реализации.

## Модель владения

Документы компонентов описывают Luna-owned архитектуру. Внешние программы, которые используются для реализации границы, являются внешними providers/dependencies, а не компонентами Luna.

Группировка репозитория:

```text
components/core/                  → основная система Luna
components/system/                → системные приложения и инструменты
components/apps/                  → обычные пользовательские приложения
components/external/providers/    → Luna-facing adapters внешних providers
components/external/libraries/    → дополнительные внешние/support libraries
```

Например, `luna-audio` задаёт Luna-facing audio boundary, а PipeWire/WirePlumber остаётся внешним provider. Аналогично `luna-bluetooth`/BlueZ, `luna-network`/NetworkManager и `luna-files`/Yazi.

## Правило актуальности

Каждое архитектурное понятие должно иметь одну текущую трактовку. Устаревшие альтернативы не оставляются в активном дереве как конкурирующие решения. Архив `docs/archive/2026-09-14-pre-audit/` является справочным и не имеет нормативной силы.

Если полезная информация найдена в архиве, её сначала сверяют с текущим SoT. В активную документацию переносится только то, что не противоречит принятым решениям.