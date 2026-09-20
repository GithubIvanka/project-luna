# Указатель архитектуры компонентов

Эта папка содержит актуальное детальное описание каждого активного архитектурного компонента Project Luna.

Каждый документ должен описывать:

- назначение и границу компонента;
- обязанности и то, чем он не занимается;
- жизненный цикл;
- архитектурные и Cargo-зависимости;
- публичные интерфейсы и контракты;
- взаимодействие с другими компонентами;
- поведение при отказах;
- текущий статус реализации.

## Модель владения

Документация компонентов описывает архитектуру, принадлежащую Luna. Внешнее программное обеспечение, используемое для реализации конкретной границы, описывается как внешний provider/dependency и не считается компонентом Luna.

Каноническая группировка репозитория:

```text
components/core/               → основная система Luna
components/system/             → системные приложения
components/apps/               → обычные пользовательские приложения
components/external/providers/ → Luna-facing adapters внешних providers
components/external/libraries/ → дополнительные внешние/support libraries
```

Каталог определяет архитектурную роль. Package name `luna-*` определяет конкретный компонент. Наличие отдельного crate не означает отдельный процесс или daemon.

Например, `luna-audio` определяет границу аудиоподсистемы Luna, тогда как PipeWire/WirePlumber остаётся внешним provider. Тот же принцип применяется к `luna-bluetooth`/BlueZ, `luna-network`/NetworkManager и `luna-files`/Yazi.

## Загрузка

- [LUNA-BOOT](LUNA-BOOT.md)
- [LUNA-INIT](LUNA-INIT.md)
- [LUNA-SYSTEM-RUNTIME](LUNA-SYSTEM-RUNTIME.md)
## Сессии и приложения

- [LUNA-USER-SESSION](LUNA-USER-SESSION.md)
- [LUNA-APP-RUNTIME](LUNA-APP-RUNTIME.md)
- [LUNA-APP-MANAGER](LUNA-APP-MANAGER.md)
- [LUNA-BUNDLE](LUNA-BUNDLE.md)
- [LUNA-ROOT-MAPPING](LUNA-ROOT-MAPPING.md)
- [LUNA-SECURITY](LUNA-SECURITY.md)
- [LUNA-NAMESPACE](LUNA-NAMESPACE.md)

## Системные службы и основы

- [LUNA-SYSTEM-MANAGER](LUNA-SYSTEM-MANAGER.md)
- [LUNA-UPDATE-MANAGER](LUNA-UPDATE-MANAGER.md)
- [LUNA-KERNEL-MANAGER](LUNA-KERNEL-MANAGER.md)
- [LUNA-DEVICE-MANAGER](LUNA-DEVICE-MANAGER.md)
- [LUNA-STATE](LUNA-STATE.md)
- [LUNA-CONFIG](LUNA-CONFIG.md)
- [LUNA-EVENT](LUNA-EVENT.md)
- [LUNA-FS](LUNA-FS.md)
- [LUNA-COMMON](LUNA-COMMON.md)

## Пользовательские и платформенные компоненты

- [LUNA-NETWORK](LUNA-NETWORK.md)
- [LUNA-AUDIO](LUNA-AUDIO.md)
- [LUNA-BLUETOOTH](LUNA-BLUETOOTH.md)
- [LUNA-FILES](LUNA-FILES.md)
- [LUNA-CLI](LUNA-CLI.md)
