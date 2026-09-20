# Модель безопасности

## Разделение ответственности

```text
запрос
  ↓
план
  ↓
mapping
  ↓
authorization
  ↓
materialization
  ↓
процесс
```

`luna-root-mapping` определяет логическое mapping. `luna-security` принимает решения о доверии и разрешениях. `luna-namespace` применяет разрешённый результат средствами Linux. `luna-app-runtime` координирует запуск и lifecycle `ApplicationInstance`.

## Запрос и grant

Bundle может запрашивать доступ к ресурсам и capabilities, но наличие запроса ничего не разрешает:

```text
request != grant
```

Некорректный, неоднозначный или запрещённый запрос отклоняется с fail closed.

## Permissions

Базовые измерения разрешений:

```text
Visibility
Read
Write
Execute
Device Use
Manage
```

`Ask` означает необходимость явного подтверждения. `Constrained` содержит структурированные ограничения, а не произвольную строку.

Per-instance policy может ужесточить policy приложения, но не может ослабить уже установленный deny.

Решение authorization может быть связано с конкретной revision/snapshot политики.

## Trust Bundle

Для Bundle существуют три независимых вопроса:

```text
действительна ли подпись?
        ↓
доверяем ли мы содержимому/источнику?
        ↓
разрешены ли нужные действия этому запуску?
```

```text
signature validity != trust != authorization
```

Trust особенно нужен для внешних и переносимых Bundles и supply-chain policy. Trust не выдаёт filesystem/device/capability permissions.

Trust связывает как минимум identity Bundle (`BundleId` + `ContentIdentity`) с trust scope. Решение о trust принадлежит `luna-security`.

Валидная Ed25519-подпись подтверждает соответствие подписи содержимому, но сама по себе не означает, что Bundle доверен или может быть запущен.

## Capability

`CapabilityRegistry` знает capability и его provider. `CapabilityGrant` появляется только после authorization. Provider не принимает policy decisions и не может расширить выданный grant.

## Файловая изоляция

Приложение получает логические пути, а не физические `LUNA-SYS/...` или `LUNA-DATA/...` пути.

Mount namespace для каждого `ApplicationInstance` обязателен. PID namespace по умолчанию не создаётся.

Landlock, credentials, capabilities, cgroups и другие Linux primitives применяются в соответствии с авторизованной policy. `CAP_SYS_ADMIN` и эквивалентный host-level доступ не выдаются приложению по умолчанию.

## Граница materialization

`luna-namespace` получает только уже авторизованный результат и не имеет права самостоятельно расширять mapping, capability или лимиты.

Если разрешение нельзя точно материализовать, запуск отклоняется.

## Статус

Базовые authorization/capability types реализованы. Production trust store, policy storage, подтверждение пользователя, provider IPC и полное kernel enforcement ещё находятся в разработке.