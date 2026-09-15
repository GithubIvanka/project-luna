# Контракт System Image

## Определение

System Image — неизменяемая версионированная системная userspace-файловая система одного релиза Luna. Это источник, из которого `luna-init` материализует работающую системную среду Luna в RAM-backed logical `/`.

System Image — **непосредственно файловая система SquashFS**. Это сам файл `.squashfs`, а не Bundle и не внешний контейнер вокруг другой файловой системы.

```text
LUNA-SYS/images/luna-X.Y.Z.squashfs
LUNA-SYS/images/luna-X.Y.Z.toml
```

`.lbp` — transport/archive формат Bundle и не связан с представлением System Image.

## Внутренняя структура

Неизменяемый System Image содержит минимальную системную базу, необходимую для полного запуска Luna OS. Его внутренняя структура отражает основные классы системных ресурсов `LUNA-DATA/system`, за исключением изменяемого состояния DATA.

```text
/
├── apps/
├── drivers/
├── firmware/
├── libs/
├── config/
└── resources/
    ├── fonts/
    ├── icons/
    ├── themes/
    ├── cursors/
    ├── locales/
    └── translations/
```

`apps/` содержит минимальные системные приложения Luna, необходимые базовой ОС. `drivers/` и `firmware/` — отдельные классы ресурсов: первый содержит сущности/модули драйверов, второй — firmware payload. Firmware никогда не является дочерним элементом `drivers/`. `libs/`, `config/` и `resources/` содержат соответствующие минимальные неизменяемые ресурсы. Долговечное `state/` и управляемые `volumes/` не входят в образ и принадлежат `LUNA-DATA/system`. Дополнительная функциональность и изменяемые системные изменения являются расширениями DATA.

## Инварианты

1. Payload образа является SquashFS.
2. Соседний manifest описывает identity/version соответствующего образа.
3. Опубликованные System Images неизменяемы во время нормальной работы.
4. Образ содержит неизменяемую userspace-среду Luna и её неизменяемые ресурсы.
5. Изменяемое состояние машины, пользователя, приложений и cache находится вне образа в соответствующем DATA provider.
6. System Image не является persistent logical `/` и не копируется целиком в RAM.
7. `luna-init` открывает выбранный образ и материализует необходимые ресурсы в RAM-backed logical root; дополнительные неизменяемые ресурсы могут гидратироваться лениво.

## Совместимость

Manifest System Image объявляет совместимые версии `luna-init` core. Manifest `luna-init` отдельно объявляет совместимые идентичности kernel.

Поэтому полный boot target разрешается только так:

```text
System Image
    ↓ manifest образа
совместимый luna-init
    ↓ manifest init
совместимый kernel
```

Контракт System Image не определяет прямую совместимость image → kernel.

## Манифест

Manifest расположен рядом с образом и читается до открытия образа как файлового источника. Точная грамматика TOML определяется отдельной спецификацией контракта и не должна расширяться из соображений удобства реализации.

## Materialization

System Image остаётся неизменяемым физическим источником в `LUNA-SYS`. `luna-init` создаёт RAM-backed logical `/` и материализует boot-critical системное содержимое из выбранного образа. Весь образ не требуется копировать в RAM при загрузке. Материализованные ресурсы должны оставаться пригодными к использованию независимо от дальнейшей видимости исходного пути образа.

## Жизненный цикл

System Images имеют независимые версии и удерживаются согласно политике обычного выбора, rollback и Factory. Обновление или удаление образа само по себе не заменяет kernel или `luna-init`; итоговый target всё равно должен разрешаться как совместимая тройка `System Image + luna-init + kernel`.

## Владение

Формат System Image определяет неизменяемый filesystem payload и связь с manifest. `luna-boot.efi` обнаруживает и выбирает образ, `luna-init` использует его как неизменяемый источник materialization, а `luna-update-manager` выполняет связанные с ним update transactions.

## Recovery DATA Image

Recovery DATA Image — единственный recovery-specific image. Recovery использует обычный выбранный System Image + совместимый `luna-init` + совместимый kernel и добавляет этот версионированный DATA image как `VirtualData`.

Его логическая структура:
```text
Recovery DATA Image
├── system/
│   ├── apps/
│   │   ├── recovery-tools/
│   │   ├── data-discovery/
│   │   ├── system-diagnostics/
│   │   └── system-repair/
│   ├── drivers/
│   ├── firmware/
│   ├── libs/
│   ├── config/
│   ├── state/
│   └── volumes/
├── users/
│   └── recovery/
│       ├── home/
│       ├── data/
│       └── config/
└── cache/
```

`state/` остаётся доступным для состояния Recovery и будущей функциональности. `volumes/` сохраняется, поскольку Recovery может обнаруживать, проверять и обрабатывать внешние диски, при этом они остаются отдельными объектами относительно работающего виртуального DATA provider.

Отдельного Recovery System Image нет.
