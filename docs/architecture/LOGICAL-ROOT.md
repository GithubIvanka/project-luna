# Логический корень

## Основной принцип

Физические разделы Luna не являются Linux `/` приложения или пользователя. Рабочая файловая система строится во время выполнения.

```text
LUNA-SYS + LUNA-DATA + Bundle + другие разрешённые источники
                     ↓
              mapping + policy
                     ↓
             RAM-backed logical /
```

Приложение получает обычное Linux-подобное дерево каталогов, но его физические источники остаются скрытыми.

## Источники

```text
immutable base → выбранный System Image
mutable system → LUNA-DATA/system
user state     → LUNA-DATA/users/<user>
cache          → LUNA-DATA/cache
```

Для отдельных ресурсов могут использоваться Bundle и разрешённые внешние volumes/devices.

## Персональность

Logical `/` создаётся отдельно для каждого `ApplicationInstance`. Это не один глобальный root для всех приложений.

Mount namespace каждого экземпляра изолирован. PID namespace по умолчанию не используется.

## Материализация

`luna-init` строит системный RAM-backed logical `/` для раннего запуска. `luna-namespace` строит и материализует логический `/` конкретного приложения на основе `RuntimeProfile` и разрешённых mappings.

В обоих случаях исходный System Image остаётся immutable source. Он не копируется целиком в RAM и не становится persistent root.

Boot-critical ресурсы материализуются заранее; дополнительные неизменяемые ресурсы могут подключаться лениво. Уже материализированный ресурс не должен зависеть от продолжения существования pathname исходного image.

## Linux runtime filesystems

`/dev`, `/proc`, `/sys`, `/run` и `/tmp` являются runtime-ресурсами и создаются соответствующими Linux/runtime механизмами. Они не являются копиями физических каталогов `LUNA-SYS` или `LUNA-DATA`.

## Видимость и доступ

Наличие логического пути не означает предоставление права:

```text
видимость пути != право доступа
request != grant
```

Доступ к host filesystem, другим приложениям, другим пользователям, устройствам и внешним сервисам предоставляется только соответствующей policy.

## Mapping

`luna-root-mapping` определяет, какой разрешённый источник может удовлетворить логический путь. `luna-security` решает, разрешено ли это. `luna-namespace` материализует уже разрешённый результат.

Физические `LUNA-SYS/...` и `LUNA-DATA/...` пути не являются API приложения.

При этом и внутренние пути System Image, и пути `LUNA-DATA/system/...` являются **источниками** для Root Mapping. Например:

```text
LUNA-DATA/system/config/luna/graphical session
                    ↓ если существует
             DATA provider config

System Image/config/luna/graphical session
                    ↓ иначе
             default config
```

После materialization эти источники могут быть представлены в logical root другими runtime-путями. Поэтому `/data/system/...` и `/etc/...` нельзя использовать как описание физической структуры System Image или LUNA-DATA.

## Ограничения

Приложение не должно видеть весь System Image или физическое дерево Luna только потому, что ему требуется `/usr`, `/lib`, `/home` или другой логический путь. Каждая ресурсная область предоставляется явно через разрешённую модель mapping/policy.