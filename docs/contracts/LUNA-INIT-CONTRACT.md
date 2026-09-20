# Контракт luna-init

`luna-init` — direct initial-userspace bootstrap, выбранный `luna-boot.efi`.

Вход:

```text
FD 3 = read-only LunaBootHandoffV1
```

## Ответственность

- проверить boot context;
- определить boot mode и выбранный target;
- разрешить физический или виртуальный DATA;
- открыть выбранный неизменяемый источник System Image;
- построить RAM-backed logical root;
- создать необходимые runtime resources;
- материализовать boot-critical ресурсы;
- подготовить доверенные входные данные runtime;
- запустить `luna-system-runtime` как дочерний процесс и сохранить за собой PID 1.

`luna-init` остаётся PID 1. Он запускает `luna-system-runtime` как дочерний system-wide runtime/supervisor; `luna-system-runtime` никогда не является PID 1.
