# `luna-bluetooth`

**Статус:** domain boundary реализована; provider integration неполная.

## Назначение

Предоставляет Luna Bluetooth domain и lifecycle без связывания архитектуры с конкретным daemon.

## Владеет

- моделью Bluetooth device;
- discovery state;
- pairing/trust state на границе Luna;
- операциями подключения и отключения;
- provider abstraction.

## Не владеет

Внутренностями BlueZ, общим device manager, authorization policy, GUI widgets или UserSession lifecycle.

## Provider

BlueZ является допустимым Linux provider. Его наличие в image не считается полной реализацией Luna boundary.

## Зависимости

`luna-device-manager`, security/session context и выбранный Linux Bluetooth stack в пределах соответствующих контрактов.

## Открыто

D-Bus integration, pairing/trust persistence, device authorization и desktop controls.