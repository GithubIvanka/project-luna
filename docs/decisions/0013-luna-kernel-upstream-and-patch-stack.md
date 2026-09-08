# ADR-0013 — Luna kernel upstream и patch stack

**Status:** Accepted
**Date:** 2026-09-08

## Context

Project Luna использует стандартный Linux `bzImage`, но требует небольшой Luna-specific kernel integration для прямого запуска memory-resident `luna-init` как PID 1 и передачи проверенного `LunaBootHandoffV1`.

Хранить полный upstream Linux kernel source tree внутри репозитория Luna нецелесообразно: это сотни мегабайт исходников и ухудшает review, clone и release workflow.

Одновременно Luna должна иметь воспроизводимую и аудируемую связь между конкретной версией upstream kernel и Luna-specific изменениями.

## Decision

1. Полный Linux kernel source **не хранится** в Git-репозитории Project Luna.
2. Build system получает exact upstream release tarball из kernel.org.
3. Версия upstream kernel является явно зафиксированным build input.
4. Luna-specific kernel integration хранится в репозитории как обычный patch stack:

```text
kernel/
├── luna-x86_64.config
└── patches/
    ├── 0001-...
    ├── 0002-...
    └── ...
```

5. `tools/build-luna-kernel.sh` выполняет pipeline:

```text
upstream tarball
      ↓
verify source/version
      ↓
apply Luna patches
      ↓
upstream x86_64_defconfig
      ↓
Luna config fragment
      ↓
olddefconfig
      ↓
bzImage + modules
      ↓
versioned kernel artifact
```

6. Kernel release и Luna patch set являются частью artifact identity.
7. Обновление upstream kernel не считается механическим изменением одной строки версии: после смены release необходимо заново проверить Kconfig symbols, patch applicability и boot integration.
8. До появления working direct-init integration patch stack может существовать как отдельный development layer; обычная сборка kernel не должна притворяться полной Luna boot implementation.

## Current upstream baseline

На 2026-09-08 текущий stable release ветки 7.2 — **7.2.4**. Build default должен быть привязан к конкретному release, а не к плавающему `latest`.

## Kernel integration boundaries

Luna kernel patches должны оставаться минимальными и иметь следующие границы:

- parse/validate Luna `setup_data` handoff;
- reserve/track loader-owned memory as needed by the handoff;
- validate `LUNA_INIT_IMAGE` digest and ELF constraints before execution;
- provide a kernel-internal anonymous boot-context object for FD 3;
- launch the validated initial userspace image through the existing Linux process/ELF machinery where practical;
- leave logical root construction, SYSTEM/DATA access policy and runtime namespace setup to `luna-init`.

Kernel patches must not reimplement the Luna userspace runtime and must not turn System Image into the long-lived root filesystem.

## Reproducibility requirements

The build must make these inputs observable:

- upstream kernel version;
- source tarball checksum;
- Luna patch list and order;
- resulting `kernelrelease`;
- final `.config`;
- resulting kernel digest.

The generated artifact must be sufficient for `kernel.toml` to describe the exact kernel identity expected by `luna-boot`.

## Rejected alternatives

### Vendor the full Linux source tree

Rejected due to repository size, review cost and duplicated upstream maintenance.

### Git submodule to Linux

Rejected as the primary distribution mechanism because the release/build pipeline still needs a precise immutable source artifact and patch application model. A submodule may be reconsidered later for specialized development workflows.

### Runtime clone of Linux git repository

Rejected because production builds should consume a fixed release source artifact rather than a mutable branch head.

### Reimplement Linux ELF loading in Luna kernel code

Rejected. The integration should reuse existing Linux executable-loading machinery wherever the kernel architecture allows it; Luna-specific code is responsible for supplying and validating the memory-backed executable object, not for creating a second general-purpose ELF loader.
