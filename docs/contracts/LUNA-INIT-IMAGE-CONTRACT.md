# Контракт образа luna-init

`luna-init` — самостоятельный версионированный ELF64-артефакт:

```text
LUNA-SYS/cores/luna-X.Y.Z.init
LUNA-SYS/cores/luna-X.Y.Z.toml
```

Артефакт `.init` является исполняемым файлом, а не filesystem image, Bundle или System Image.

Соседний manifest определяет core, архитектуру и совместимые kernels.

`luna-boot.efi` загружает точные байты файла в зарезервированную для загрузки память. Kernel проверяет диапазон памяти, digest и ELF constraints перед direct initial-userspace execution.

Manifest System Image выбирает совместимые init cores. Manifest init выбирает совместимые kernels. Выбранный `luna-init` является реальным членом полного boot target `System Image + luna-init + kernel`. Identity kernel остаётся независимо версионируемой.
