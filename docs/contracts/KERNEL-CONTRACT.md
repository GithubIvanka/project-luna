# Контракт Linux kernel

Kernel Luna остаётся Linux с минимальной интеграцией прямого запуска `luna-init`, необходимой для `LunaBootHandoffV1`.

## Ответственность

- разбор Luna `setup_data` handoff;
- проверка handoff и границ памяти;
- проверка находящегося в памяти ELF `luna-init` и BLAKE3 digest;
- создание внутреннего kernel memory-backed executable object;
- повторное использование Linux ELF/binfmt execution;
- передача проверенного handoff в FD 3.

## Не входит в ответственность

Kernel не выбирает System Images, не разрешает политику DATA, не строит logical root, не управляет UserSession и не реализует authorization приложений.

## Граница direct-init

`luna-boot.efi` помещает точные байты `.init` в зарезервированную память. Kernel проверяет их и передаёт выполнение в обычный путь загрузки Linux executable.

## Сборка

Изменения kernel, специфичные для Luna, поддерживаются как небольшой patch stack в `kernel/` и применяются к закреплённой upstream-версии Linux средствами сборки.
