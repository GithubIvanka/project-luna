# Контракт System State

`luna-system-manager` владеет смысловой моделью persistent system targets. `luna-state` владеет механизмами долговечного хранения.

## Идентичности target

```text
current  = System Image + luna-init + kernel
factory  = System Image + luna-init + kernel
fallback = System Image + luna-init + kernel (если определён)
recovery = System Image + luna-init + kernel + Recovery DATA Image
```

`luna-init` является частью полного boot target, а его конкретный artifact/version входит в identity target. Совместимость устанавливается через manifest System Image, который выбирает init, затем manifest init, который выбирает совместимый kernel.

## Совместимость

```text
System Image manifest
  ↓ compatible init
luna-init manifest
  ↓ compatible kernel
selected kernel
```

Сохранённый target пригоден к использованию только если двухэтапное разрешение даёт один валидный полный runtime target.

## Хранение

Текущий backend долговечного состояния хранится под `LUNA-DATA/system/state` и управляется через `luna-state`.

System State отделён от boot-attempt marker. Он не превращается в журнал каждой загрузки.

## Изменение

Изменения target выполняются транзакционно и с учётом revision. Новый current target становится долговечным только после валидации как `System Image + luna-init + kernel`. Factory и Recovery являются независимыми ролями. Recovery добавляет только отдельный Recovery DATA Image и не вводит отдельный System Image.
