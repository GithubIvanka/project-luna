# `luna-bluetooth`

## Назначение

Luna-owned доменная граница Bluetooth-устройств.

## Владеет

Идентичностью устройства, именем, состоянием подключения и backend interface.

## Не владеет

GUI сопряжения, network policy или process supervision.

## Внешний provider

Для реализации может использоваться BlueZ. BlueZ остаётся внешней зависимостью.

## Статус

Доменные типы и backend interface существуют. Полная интеграция с BlueZ ещё не завершена.