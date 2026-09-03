# `luna-bundle`

**Статус:** domain и RFC-0002/LBP1 codec реализованы; дальнейшая интеграция продолжается.

## Назначение

Представляет, валидирует, читает и записывает Luna Bundles и их принятую транспортную форму `.lbp`.

## Владеет

- Bundle identity и metadata;
- manifest model и validation;
- Bundle resource representation;
- LBP1 reader/writer;
- детерминированным payload encoding;
- BLAKE3 content identity;
- codec/verification boundary для Ed25519;
- hardening и path validation.

## Форматный инвариант

`.lbp` — транспортное/archive представление Bundle. Это не System Image и не установленная runtime representation.

System Image остаётся `luna-X.Y.Z.squashfs` плюс соседний manifest.

## Manifest

Mappings являются логическими Bundle-relative declarations. Manifest не должен кодировать физические пути `DATA/system/apps/...` или `DATA/users/...` как mapping targets.

Capabilities и access fields являются запросами. Grant выдаёт `luna-security`.

## Не владеет

Install/update/removal policy, trust policy, namespace creation, process lifecycle или UEFI boot.

## Зависимости

Только необходимые shared identifiers/version types и format/serialization primitives. `luna-bundle` не должен зависеть вверх от manager/runtime компонентов.

## Интеграция

`luna-app-manager` отвечает за install transaction. Runtime потребляет валидированные Bundle semantics. Внешний Bundle должен быть проверен до install/launch.

## Открыто

Supply-chain/repository trust и delta update механизмы находятся вне RFC-0002.