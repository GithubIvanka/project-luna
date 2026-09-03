# Контракт передачи управления из `luna-boot`

**Статус:** черновик для Phase 0.

## 1. Цель

После работы UEFI-загрузчика Linux kernel должен получить самодостаточный набор данных для перехода в ранний userspace без обращения к UEFI Boot Services.

## 2. Что передаётся

В handoff должны быть однозначно представлены:

- выбранный System Image;
- выбранный kernel;
- командная строка ядра;
- initramfs с `luna-init`;
- сведения о памяти, доступные после `ExitBootServices`;
- необходимые данные для определения SYSTEM/DATA;
- boot metadata, которые нужны `luna-init` для выбора logical root.

Точный ABI handoff фиксируется отдельным boot implementation contract.

## 3. Граница ответственности

До `ExitBootServices` отвечает `luna-boot.efi`. После передачи управления kernel и ранний userspace отвечают за дальнейшую загрузку.

```text
UEFI Boot Services
      ↓
luna-boot.efi
      ↓ ExitBootServices
Linux kernel
      ↓
luna-init
      ↓
logical Linux root
```

## 4. SYSTEM Image

`luna-boot.efi` не обязан монтировать SquashFS. Выбранный image передаётся kernel/`luna-init` как boot context, а `luna-init` открывает SYSTEM, получает `.squashfs` и строит логический `/`.

## 5. Post-ExitBootServices invariant

После `ExitBootServices` запрещены обращения к UEFI Boot Services, UEFI filesystem protocols, firmware allocator и console APIs, если соответствующий интерфейс не относится к уже сохранённым данным/другому допустимому runtime protocol.

Все данные, необходимые kernel handoff, должны быть подготовлены заранее.

## 6. Ошибки

Если handoff нельзя безопасно сформировать, загрузчик должен остановить текущую попытку загрузки и перейти к предусмотренному fallback/recovery пути. Нельзя передавать частично заполненный или неоднозначный boot context.