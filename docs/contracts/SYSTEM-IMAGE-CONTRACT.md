# Контракт System Image

**Статус:** черновик для Phase 0; детали materialization/hydration уточняются отдельной реализацией.
**Scope:** System Image → `luna-init` → running RAM-backed system

## 1. Назначение

Этот контракт определяет границу между версионированным System Image, его manifest-файлом, загрузчиком, `luna-init`, runtime materialization и подсистемами обновления.

## 2. Инвариант формата

System Image — непосредственно файловая система SquashFS.

Каноническая пара:

```text
SYSTEM/images/luna-X.Y.Z.squashfs
SYSTEM/images/luna-X.Y.Z.toml
```

`.squashfs` не является `.lbp`, контейнером или архивом, внутри которого лежит SquashFS.

## 3. Идентичность

Для обнаруженного образа должны быть однозначно определимы:

- имя семейства системы;
- версия `X.Y.Z`;
- архитектура;
- путь к payload;
- путь к соответствующему manifest.

Несоответствие имени, версии или manifest должно приводить к отказу от использования образа.

## 4. Manifest

Manifest является источником метаданных именно для своего образа. Минимальный набор семантик:

- идентичность и версия;
- архитектура;
- формат `squashfs`;
- совместимые ядра;
- параметры, необходимые для передачи управления ядру;
- сведения о целостности, если они определены политикой доверия.

Точная TOML-схема ещё не принята. До её утверждения поля нельзя объявлять обязательными только из-за удобства реализации.

## 5. Целостность

До активации необходимо проверить как минимум структурную корректность SquashFS и внутреннюю согласованность manifest. Проверка подлинности и доверия должна определяться отдельным security/update-контрактом.

## 6. Связь с kernel

System Image и kernel — независимые сущности. Manifest задаёт явную область совместимости. Выбор «самого нового ядра» без проверки совместимости запрещён.

## 7. Жизненный цикл

```text
обнаружение
  ↓
чтение manifest
  ↓
структурная проверка
  ↓
проверка целостности/доверия
  ↓
доступен для выбора
  ↓
активация через update-manager
  ↓
подтверждение здоровья
  ↓
допускается в retention policy
```

Удаление active, factory или необходимого fallback-образа запрещено до прохождения materialization/lifetime checks.

## 8. Загрузка и материализация

`luna-boot.efi` выбирает образ и передаёт kernel контекст. Он не обязан монтировать SquashFS как Linux `/`.

`luna-init` использует выбранный image как **immutable source** для построения рабочего окружения:

```text
SYSTEM/images/luna-X.Y.Z.squashfs
          │
          ▼
      luna-init
          │
          ├── RAM: boot-critical system base
          ├── RAM: runtime directories and pseudo-filesystems
          └── lazy hydration of additional immutable system content
                       ↓
               RAM-backed logical `/`
```

SquashFS **не является долгосрочным backing store для `/`**.

На раннем этапе в RAM материализуется минимальный системный base, необходимый для запуска `luna-system-runtime` и базовой работоспособности. Псевдофайловые системы и volatile directories (`/dev`, `/proc`, `/sys`, `/run`, `/tmp`) создаются отдельно в runtime memory.

Остальной immutable system content может материализоваться лениво по требованию. Lazy hydration должна приводить к обычным объектам logical root и не должна раскрывать приложению физические `SYSTEM/...` paths.

Уже материализованный content не должен становиться недоступным только из-за удаления исходного image. Перед удалением active image update/retention layer обязан подтвердить, что все ещё необходимые runtime resources либо уже materialized в RAM/другом валидном источнике, либо больше не требуются.

Таким образом рабочая система не является mounted SquashFS root: System Image — источник первоначальной материализации и последующей hydration, а не runtime root.

## 9. Независимость от DATA

Пользовательские данные, изменяемое системное состояние, данные приложений и cache не входят в System Image.

## 10. Открытые вопросы

- точная схема manifest;
- точный набор boot-critical RAM base;
- формат и владелец lazy-hydration cache/index;
- механизм materialization без прямого раскрытия SYSTEM приложению;
- проверка "image no longer required" перед retention removal;
- точный переход от early userspace к `luna-system-runtime`.
