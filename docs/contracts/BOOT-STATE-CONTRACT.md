# Контракт Boot State

`LUNA-SYS/config/boot-state.toml` хранит долговечный контекст boot target. Это не журнал стадий.

## Роли

```text
current
fallback
factory
recovery
```

`current`, `fallback` и `factory` определяют полный boot target `System Image + luna-init + kernel`.

`recovery` определяет полный target `System Image + luna-init + kernel + Recovery DATA Image`. Отдельного Recovery System Image нет. Цепочка `System Image → compatible luna-init → compatible kernel` разрешает системную часть target; Recovery DATA Image является отдельным recovery-specific источником данных.

## Разрешение

```text
System Image
  ↓ manifest образа
совместимый luna-init
  ↓ manifest init
совместимый kernel
```

Запускать можно только полностью разрешённый target.

## Минимальные записи

Обычная успешная загрузка не переписывает persistent boot state только потому, что машина запустилась. Изменения выполняются только при значимых переходах target, подтверждения, сбоя, fallback, Factory или Recovery.

## Маркер попытки загрузки

`LunaBootAttempt` — отдельный UEFI NVRAM marker. Он записывается один раз непосредственно перед `ExitBootServices` и очищается только после подтверждённого semantic boot success работающей системой.

Пошаговый прогресс попытки хранится только в RAM.
