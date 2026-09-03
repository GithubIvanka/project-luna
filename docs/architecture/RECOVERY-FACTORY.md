# Project Luna — Recovery and Factory

**Status:** accepted architecture

## Recovery

Recovery is a functional Luna environment used when the normal system cannot safely operate.

Recovery must be able to:

- diagnose system failures;
- repair installed DATA components where supported;
- disable/remove a broken driver;
- recover user data;
- access external media;
- restore a usable normal boot state.

Recovery does not depend on normal persistent DATA being available. Its recovery identity/state is temporary and lives in RAM unless a future explicit design says otherwise.

Recovery is not merely a diagnostic shell. TTY/serial access may exist for development/diagnostics/recovery, but is not the normal user login path.

## Factory

Factory is the original known-good installation state:

```text
Factory System Image
+
Factory Kernel
```

Factory remains preserved and immutable. Normal update/removal/retention operations must never destroy it.

Factory is distinct from Recovery: Factory boots the preserved original system state, while Recovery is the repair environment.

## Failure policy

The system should diagnose and repair failures where possible before reaching an unrecoverable emergency state.

Boot fallback is owned by `luna-boot`; recovery functionality is provided by the recovery system environment and relevant Luna management components.

## Boundaries

Recovery must not become a second ordinary system runtime, application manager or bootloader. It uses the same architectural contracts where possible while operating with reduced/temporary persistent state.
