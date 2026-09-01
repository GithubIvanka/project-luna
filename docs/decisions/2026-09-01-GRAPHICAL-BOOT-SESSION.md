# Project Luna — Graphical Boot and Login Decision

**Дата:** 2026-09-01  
**Статус:** accepted implementation decision  
**Branch:** `development`  
**Architectural Source of Truth:** `docs/ARCHITECTURE.md`

## 1. Normal boot is graphical

The normal PC boot path must not present a TTY, console login, shell fallback or a second serial-oriented OS target to the user.

```text
UEFI
  ↓
luna-boot.efi
  ↓
GUI boot splash
  ↓
Linux kernel
  ↓
early userspace
  ↓
System Image + DATA
  ↓
luna-system-runtime
  ↓
UserSession
  ↓
GUI login screen
  ↓
authentication
  ↓
Active UserSession
  ↓
Wayland
  ↓
niri
  ↓
Noctalia Shell
```

TTY/serial remains a development, diagnostic and recovery facility only.

## 2. No `luna-session` component

`luna-session` is not an architectural component and must not be introduced as one.

`UserSession` is the existing domain entity owned/coordinated by `luna-system-runtime`. The graphical login surface belongs to the UserSession lifecycle. Application execution remains the responsibility of `luna-app-runtime`.

## 3. Boot splash

The normal boot path shows a minimal graphical Luna splash through UEFI GOP. The splash is intentionally tiny and uses only UEFI boot-time facilities; it does not become a desktop or GUI framework.

## 4. Verbose Boot

Verbose Boot is a Boot Menu action, not a separate boot target and not a normal operating mode.

```text
B
 ↓
Boot Menu
 ↓
Verbose Boot
 ↓
(no splash)
 ↓
full boot/kernel diagnostics
```

Verbose mode removes `quiet`, raises the kernel log level and enables `ignore_loglevel` for the selected boot. The text console remains visible throughout the diagnostic boot path.

Normal boot keeps the quiet boot parameters and shows the graphical splash.

## 5. Boot Menu

`B` remains the only normal entry into Boot Menu. The menu is a special pre-OS control surface and is allowed to be text based. It is not the normal user interface.

The accepted action order is:

```text
1. Continue to Luna
2. Verbose Boot
3. System Image selection
4. Recovery Environment
5. Factory Environment
6. Boot from USB / External Device
```

The development implementation discovers normal System Images from SYSTEM metadata and represents Recovery, Factory and External Boot as typed menu actions. Recovery and Factory are executed when their corresponding System Image targets are present; an unavailable mode is reported as unavailable rather than replaced by a fake target or TTY fallback.

## 6. Graphical login contract

The runtime creates the `UserSession` in `Authenticating` state and launches the configured graphical login surface.

The graphical login surface is represented during this integration phase by:

```text
/etc/luna/graphical-login
/usr/bin/luna-login
```

Successful login returns process success, after which `luna-system-runtime` transitions the UserSession to `Active` and launches:

```text
/usr/bin/niri-session
```

A failed login does not activate the UserSession; the runtime returns to the graphical login boundary.

This process-exit handshake is a Phase 2 integration contract. The final production login UI/IPC authentication protocol will use the shared Luna IPC/security architecture rather than treating an exit code as the permanent authentication protocol.

## 7. No shell fallback in graphical PC images

The PC image builder must not create `/usr/bin/luna-session`, `/etc/luna/session` or a graphical-to-shell fallback.

A graphical PC image requires a prepared desktop root containing at least:

```text
/usr/bin/luna-login
/usr/bin/niri-session
```

This deliberately makes an incomplete graphical image fail during image construction instead of silently producing a TTY-oriented Luna system.

## 8. Scope and next hardening

This decision defines the user-facing boot/session direction and the integration boundary. It does not yet finalize:

* the production Wayland login protocol;
* the exact GUI toolkit/renderer for `luna-login`;
* the final Security-to-login IPC schema;
* boot-success persistence;
* final recovery UI;
* Secure Boot signing.

Those remain implementation/specification work under the existing architecture.
