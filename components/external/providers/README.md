# External Providers

Здесь находятся Luna-facing компоненты, которые задают интерфейс Luna для внешних систем и библиотек.

Текущие providers:

- `luna-audio` — audio boundary, внешний provider: PipeWire/WirePlumber.
- `luna-network` — network boundary, внешний provider: NetworkManager.
- `luna-bluetooth` — Bluetooth boundary, внешний provider: BlueZ.
- `luna-files` — file-manager boundary, внешний provider: Yazi/другой внешний file manager.

Provider adapter не является частью `components/core/`. Его можно заменить другой реализацией без изменения core contracts.
