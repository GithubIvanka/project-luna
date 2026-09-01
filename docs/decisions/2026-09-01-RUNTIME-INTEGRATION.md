# Project Luna — Runtime Integration Decisions

**Дата:** 2026-09-01  
**Статус:** принято как текущая реализационная фиксация  
**Архитектурный SoT:** `docs/ARCHITECTURE.md`

Этот файл фиксирует решения текущего перехода к интегрированному runtime/bootable userspace. Он не заменяет Source of Truth.

## 1. Единственный владелец process supervision

`luna-system-runtime` является единственным владельцем `ProcessSupervisor`.
`luna-app-runtime` не содержит собственного supervisor; он хранит связь `ApplicationInstance ↔ ProcessId` и обращается к `SystemRuntimeService` за spawn/poll/terminate.

## 2. ApplicationInstance и PID

`ApplicationInstanceId` не равен PID. Процесс — технический runtime resource, `ApplicationInstance` — доменная сущность Luna. Текущий bring-up использует один основной process handle; модель допускает несколько процессов в одном instance.

## 3. Namespace boundary

Security authorization и Mapping validation завершаются до materialization. Текущий child-side namespace setup — временный integration backend; до production/multithreaded use он должен быть заменён безопасным dedicated child-creation primitive.

## 4. Bootable userspace

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

## 5. Development storage

Тестовый QEMU disk содержит отдельные EFI, SYSTEM и DATA области. DATA монтируется как `/data` внутри logical root.

## 6. Runtime process lifecycle

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

## 7. PID 1 development behaviour

`luna-system-runtime` как development init поддерживает продолжение runtime после завершения пользовательского shell и может создать новую UserSession/shell. `LUNA_NO_RESPAWN=1` разрешена только для bring-up/debugging.

## 8. Status discipline

Наличие scripts/harness в GitHub не означает фактическую проверку QEMU/OVMF на машине пользователя. До реального запуска статус остаётся `development path`.

## 9. Rust ownership rule

Владелец системного процесса один — `system-runtime`. Остальные компоненты работают через typed API и не дублируют ownership системных процессов.

## 10. Scope

Эти решения уточняют implementation/integration boundaries и не изменяют принятые фундаментальные архитектурные решения, RFC-0002 или модель `EFI / SYSTEM / DATA / SWAP`.
