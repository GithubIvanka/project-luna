# Project Luna — Recovery и Factory

**Статус:** принятая архитектурная модель; детали recovery state machine ещё уточняются.

## 1. Factory

Factory — сохранённая известная рабочая комбинация:

```text
Factory System Image
+
Factory Kernel
```

Она создаётся при первоначальной установке и не уничтожается обычными update/retention операциями.

## 2. Normal fallback

```text
current image/kernel
        ↓ failure
previous compatible choice
        ↓ failure
next usable fallback
        ↓
Factory
```

При отказе System Image допускается soft fallback без полного reboot, если ошибка обнаружена в userspace и новая попытка безопасна. Kernel-level failure может требовать reboot.

## 3. Recovery

Если обычная и Factory загрузка недоступны, `luna-boot.efi` должен перейти в Recovery Environment.

Recovery — отдельный boot mode, а не обычный TTY.

## 4. Recovery capabilities

Минимальный набор будущего recovery:

- просмотр image/kernel inventory;
- проверка metadata и состояния;
- rollback;
- выбор Factory;
- отключение проблемного компонента;
- диагностика;
- экспорт диагностических данных.

Recovery не должен зависеть от работоспособности обычного desktop userspace.

## 5. DATA

Recovery должен быть способен стартовать без обязательного доступа ко всему DATA. Доступ к DATA определяется конкретной диагностической или восстановительной операцией и должен быть ограниченным.

## 6. Failure classes

Нужно различать:

- malformed boot artifact;
- incompatible image/kernel;
- System Image startup failure;
- kernel failure;
- early-userspace failure;
- исчерпание usable fallback.

Одна причина не должна автоматически считаться другой.

## 7. Rollback

Rollback возвращает последнюю подтверждённую рабочую комбинацию либо другую валидную fallback choice. Пользовательские данные не должны удаляться как побочный эффект обычного rollback.

## 8. Открыто

До отдельного контракта остаются: точная state machine, health confirmation, счётчик попыток, формат recovery state и безопасные операции над driver/application state.