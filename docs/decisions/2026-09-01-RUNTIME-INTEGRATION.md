# Project Luna — Runtime Integration Decisions

**Дата исходного решения:** 2026-09-01  
**Статус:** частично SUPERSEDED  
**Архитектурный SoT:** `docs/ARCHITECTURE.md`

Этот файл сохраняет решения текущего перехода к интегрированному runtime. Формулировки boot path, которые зависели от `switch_root`, заменены принятой RAM-backed `luna-init` моделью. Файл не заменяет Source of Truth.

## 1. Единственный владелец process supervision

`luna-system-runtime` является единственным владельцем `ProcessSupervisor`.
`luna-app-runtime` не содержит собственного supervisor; он хранит связь `ApplicationInstance ↔ ProcessId` и обращается к `SystemRuntimeService` за spawn/poll/terminate.

## 2. ApplicationInstance и PID

`ApplicationInstanceId` не равен PID. Процесс — технический runtime resource, `ApplicationInstance` — доменная сущность Luna. Текущий bring-up использует один основной process handle; модель допускает несколько процессов в одном instance.

Принятая текущая PID-модель:

```text
PID 1 → luna-system-runtime
ApplicationInstance → обычный system process с non-1 PID
```

Отдельного `luna-app-init` нет. PID namespace не является обязательным механизмом isolation.

## 3. Namespace boundary

Security authorization и Mapping validation завершаются до materialization. Текущий child-side namespace setup — временный integration backend; до production/multithreaded use он должен быть заменён безопасным dedicated child-creation primitive.

Mount namespace isolation остаётся обязательной. Другие namespaces выбираются policy/runtime profile.

## 4. Bootable userspace

Старый путь:

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

**Устарел.** Принятый путь:

```text
luna-boot.efi
→ Linux kernel
→ luna-init
→ internal immutable SquashFS source
→ RAM-backed logical /
→ luna-system-runtime (PID 1)
→ UserSession
→ shell / graphical session
```

## 5. Development storage

Тестовый QEMU disk содержит отдельные EFI, SYSTEM и DATA области. DATA подключается в logical runtime root согласно текущему mapping/bootstrap contract.

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

`luna-system-runtime` как system PID 1 владеет основным process supervision lifecycle. Development-поведение с respawn пользовательского shell допускается только как bring-up/debugging механизм и не изменяет архитектурную модель application execution.

## 8. Status discipline

Наличие scripts/harness в GitHub не означает фактическую проверку QEMU/OVMF на машине пользователя. До реального запуска статус остаётся `development path`.

## 9. Rust ownership rule

Владелец системного process supervision один — `system-runtime`. Остальные компоненты работают через typed API и не дублируют ownership системных процессов.

## 10. Scope

Эти решения уточняют implementation/integration boundaries и не изменяют принятые фундаментальные архитектурные решения, RFC-0002 или модель `EFI / SYSTEM / DATA / SWAP`.

## 11. Durable System State ownership

`luna-system-manager` является владельцем логического состояния текущей/заводской System Image и current/factory kernel. Это состояние хранится через `luna-state` в `DATA/system/state`.

`luna-system-runtime` может владеть подключённым `PersistentSystemManager`, чтобы runtime работал с актуальным System State, но update execution по-прежнему остаётся ответственностью `luna-update-manager`.
