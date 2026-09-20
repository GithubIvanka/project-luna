# Project Luna Components

Компоненты группируются по смыслу и уровню ответственности.

```text
components/
├── core/                # основная система Luna
├── system/              # системные приложения и инструменты
├── apps/                # обычные пользовательские приложения Luna
└── external/
    ├── providers/       # Luna-facing adapters внешних подсистем
    └── libraries/       # дополнительные внешние/support libraries
```

## Правило

- `core/` — то, без чего работает основа ОС: boot/init/runtime, session, filesystem, state, security, namespace и внутренние system contracts.
- `system/` — приложения, которые обслуживают или администрируют ОС, но не являются ядром её runtime.
- `apps/` — обычные пользовательские приложения.
- `external/providers/` — Luna-facing boundary поверх внешнего provider. Provider не становится Luna core только потому, что его functionality нужна ОС.
- `external/libraries/` — сторонние или вспомогательные библиотеки, которые не должны загрязнять core hierarchy.

Названия пакетов `luna-*` сохраняются независимо от каталога: каталог показывает архитектурную роль, package name — конкретный компонент.