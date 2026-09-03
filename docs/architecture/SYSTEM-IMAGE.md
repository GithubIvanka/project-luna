# Project Luna — System Image

**Статус:** принятое направление формата; детальная спецификация уточняется в Phase 0.  
**Канонический payload:** `luna-X.Y.Z.squashfs`

## 1. Определение

System Image — неизменяемый filesystem payload одной версии Luna.

Он является **непосредственно файловой системой SquashFS**.

System Image не является:

- `.lbp` Bundle;
- Bundle, содержащим SquashFS;
- произвольным контейнером, внутри которого SquashFS является payload.

## 2. Представление на диске

```text
SYSTEM/images/
├── luna-X.Y.Z.squashfs
└── luna-X.Y.Z.toml
```

Имя payload содержит версию. Соседний TOML manifest описывает metadata, которые нужны загрузке и управлению.

## 3. Содержимое image

SquashFS содержит неизменяемую системную userspace среду, необходимую для построения logical Linux root.

В зависимости от system build contract сюда могут входить:

- системные binaries и libraries;
- значения конфигурации по умолчанию;
- runtime components;
- desktop/login assets;
- неизменяемые ресурсы.

Изменяемое состояние машины, пользователя, приложений и cache должно находиться вне image.

## 4. Manifest

Manifest относится именно к своему image и семантически является источником для:

- имени и версии Luna;
- идентичности image;
- архитектуры;
- kernel compatibility;
- boot metadata;
- integrity/trust metadata, если они определены отдельной политикой.

Точная TOML-схема ещё не утверждена. Реализация не должна сама превращать удобное для неё поле в обязательный архитектурный контракт.

## 5. Совместимость с kernel

Image и kernel — независимые сущности.

```text
System Image A ── совместим ── Kernel 1
System Image A ── совместим ── Kernel 2
System Image B ── совместим ── Kernel 2
```

Совместимость должна быть явной. Нельзя автоматически выбирать самое новое ядро для любого image.

## 6. Модель доступа

Архитектура допускает lazy/hybrid доступ к SquashFS вместо обязательной полной загрузки image в RAM.

Logical-root layer может материализовывать только нужные данные. Уже материализованный активный system content нельзя освобождать только потому, что исходный image позднее удалён, если другого валидного источника нет.

## 7. Factory

Factory — сохранённый известный рабочий System Image вместе с factory kernel.

```text
Factory System Image
+
Factory Kernel
```

Обычные update/retention операции не имеют права удалять или заменять factory.

## 8. Retention

System Images версионируются и удерживаются по policy. До удаления должны оставаться current и необходимые fallback choices.

Точное количество сохранённых версий — policy, а не свойство файловой системы.

## 9. Update boundary

`luna-update-manager` выполняет state-changing update transactions. `luna-system-manager` владеет semantics системного состояния и запросов. `luna-kernel-manager` владеет inventory и compatibility queries для kernel.

Сам формат System Image не владеет update transaction.

## 10. Проверка

До того как image станет доступным для загрузки, boot/update path должен подтвердить структурную корректность и внутреннюю согласованность metadata. Authenticity/trust policy определяется отдельно и не смешивается с Bundle trust model.

## 11. Связь с `.lbp`

```text
Application / component Bundle → .lbp → установленный Bundle
Luna System Image              → .squashfs + .toml → SYSTEM image
```

Это независимые форматы и независимые lifecycle domains.

## 12. Полный контракт

Расширенный Phase 0 draft находится в:

`docs/contracts/SYSTEM-IMAGE-CONTRACT.md`