# Project Luna — Runtime Integration Decisions

**Дата:** 2026-09-01  
**Статус:** принято как текущая реализационная фиксация  
**Архитектурный SoT:** `docs/ARCHITECTURE.md`

Этот файл фиксирует решения, принятые при переходе от отдельных backend-прототипов к интегрированному runtime/bootable userspace. Он не заменяет Source of Truth и не вводит новый архитектурный слой.

## 1. Единственный владелец системного process supervision

`luna-system-runtime` является единственным владельцем `ProcessSupervisor`.

`luna-app-runtime` не содержит отдельного process supervisor. Он хранит только связь `ApplicationInstance ↔ ProcessId` и обращается к `luna-system-runtime` для spawn/poll/terminate.

## 2. ApplicationInstance и процесс

`ApplicationInstanceId` не равен PID.

Процесс является техническим runtime resource, а `ApplicationInstance` — доменной сущностью Luna. Один instance потенциально может содержать несколько процессов; текущий bring-up использует один основной process handle.

## 3. Namespace boundary

Security authorization и Mapping validation должны завершиться до materialization.

Текущий Linux prototype использует child-side namespace preparation перед `exec`. Это временный integration backend и должен быть заменён безопасным dedicated child-creation primitive до production/multithreaded use.

## 4. Bootable userspace bring-up

Разработческий QEMU путь проверяет:

```text
luna-boot.efi
→ Linux kernel
→ early userspace
→ SYSTEM
→ SquashFS System Image
→ DATA
→ switch_root
→ luna-system-runtime
→ UserSession
→ shell
```

## 5. DATA в development image

QEMU development disk содержит отдельные EFI, SYSTEM и DATA области. DATA монтируется как `/data` внутри logical root.

## 6. System Image в development image

Development System Image является прямым SquashFS artifact. Он не меняет production System Image specification.

## 7. Runtime process lifecycle

Завершение application process приводит к обновлению `ApplicationInstance`:

```text
Running
  ↓
process exit
  ↓
Stopped  (exit success)
или
Failed   (non-zero/abnormal exit)
```

Staging namespace resources удаляются после завершения процесса.

## 8. PID 1 development behaviour

`luna-system-runtime` как development init поддерживает продолжение runtime после завершения пользовательского shell и может создать новую UserSession/shell.

`LUNA_NO_RESPAWN=1` разрешена только для bring-up/debugging.

## 9. Status discipline

Наличие скриптов в репозитории не означает, что QEMU/OVMF фактически проверен на машине пользователя. До реального запуска статус остаётся `development path`.

## 10. Rust implementation rule

Process ownership остаётся на уровне `system-runtime`. Остальные компоненты работают через typed API/borrowed service boundary и не дублируют ownership системных процессов.
