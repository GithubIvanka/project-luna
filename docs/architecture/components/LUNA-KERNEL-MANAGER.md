# `luna-kernel-manager`

## Назначение

Доменная модель установленных и доступных kernel identity и запросов их выбора.

## Владеет

Представлением kernel references и логикой выбора kernel; предоставляет типизированные query boundaries для system/update management.

## Не владеет

UEFI parser/load implementation и компиляцией Linux kernel.

## Совместимость

При выборе kernel учитывается цепочка совместимости `System Image → luna-init → kernel`. Manager предоставляет данные о kernel, а окончательная загрузочная сборка выполняется `luna-boot.efi`.

## Статус

Базовая модель kernel references/selection существует. Полная установка артефактов и backend проверки совместимости ещё не завершены.