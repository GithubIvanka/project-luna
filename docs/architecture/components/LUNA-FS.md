# `luna-fs`

**Статус:** foundation реализован.

## Назначение

Предоставляет низкоуровневые filesystem primitives и metadata без внедрения Luna policy.

## Владеет

- filesystem handles/primitives;
- metadata;
- filesystem errors;
- host-backed/test implementations там, где они нужны.

Текущая API-направленность включает операции `FileSystem`, `open`, `create`, `remove`, `metadata`.

## Не владеет

- logical path mapping;
- authorization;
- application lifecycle;
- Bundle installation;
- configuration precedence;
- namespace policy.

## Контракт

Успешная filesystem operation означает только успех underlying primitive. Она не означает, что вызывающая сторона имеет Luna authorization. Для policy-sensitive операций caller обязан пройти соответствующую security boundary.

## Зависимости

Может использовать стандартные OS/filesystem mechanisms и `luna-common`, но не должен зависеть вверх от managers или runtimes.

## Ошибки

Операционные отказы должны возвращаться типизированным `Result`; filesystem absence и corruption не должны маскироваться panic.

## Открыто

Production backends, filesystem-specific optimizations и интеграция с остальными storage contracts.