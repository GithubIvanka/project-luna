# Контракт отказов и восстановления

## Отказ System Image или раннего userspace

Если уже загруженный kernel остаётся работоспособным, Luna может попробовать другой System Image без перезагрузки. Кандидат должен заново разрешить полную совместимую цепочку:

```text
System Image
    ↓
совместимый luna-init
    ↓
совместимый уже загруженный kernel
```

Нельзя самостоятельно менять только `luna-init` или kernel, взятые из другого target.

Soft fallback действует только пока текущий kernel остаётся рабочим.

## Отказ ядра

Kernel panic является отказом уровня reboot. Следующий запуск `luna-boot.efi` обнаруживает незавершённый `LunaBootAttempt` и выбирает предыдущий совместимый полный target согласно durable boot state.

После этого `luna-init` снова разрешается через manifest System Image и manifest `luna-init`.

## Ручной выбор

Выбор всегда иерархический:

```text
System Image
  ↓
совместимые luna-init
  ↓
совместимые kernels выбранного init
```

Пользователю нельзя показывать kernel как совместимый с System Image, пока выбранный init не подтвердил эту совместимость.

## Recovery

Recovery является полным target:

```text
System Image
+ совместимый luna-init
+ совместимый kernel
+ Recovery DATA Image
```

Отдельного Recovery System Image нет. Recovery DATA Image материализуется в RAM как `VirtualData` и содержит схему `luna-data` для recovery-пользователя и программное окружение диагностики/восстановления.

Recovery может стартовать без обычной физической `LUNA-DATA`.

## Factory

Factory — сохранённый известный хороший target:

```text
System Image + luna-init + kernel
```

Factory не является shell fallback и не является отдельным runtime-компонентом.

## Правило fail closed

Если отказ нельзя точно локализовать или безопасно продолжить запуск, система не должна расширять права, произвольно смешивать артефакты или добавлять новый runtime layer. Используется следующий допустимый target либо Recovery согласно boot policy.