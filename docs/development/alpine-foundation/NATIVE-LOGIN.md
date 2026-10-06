# Native Luna Login UI

## Purpose

The Alpine foundation experiment now uses the Luna-owned `luna-user-session` as the default graphical login path. Alpine `greetd`, GTKGreet and Cage are optional reference/fallback components only.

## Current native path

OpenRC
  -> luna-session
      -> root luna-user-session --login-launch /usr/local/bin/luna-graphical-session
          -> AccountDirectory
          -> SessionUi
              -> SeatController
              -> DRM/KMS
              -> evdev keyboard + pointer
          -> shadow/crypt authentication
          -> root supervisor
              -> user desktop child (Niri / PipeWire / WirePlumber / Noctalia)

The login UI owns DRM master while the authentication screen is active. Before the user desktop starts, UserSession restores the pre-login CRTC state and releases the temporary framebuffer. The parent UserSession process remains privileged and supervises the user desktop child, so a desktop logout returns to the native login UI instead of terminating the login boundary.

## Verified 2026-09-26

A fresh Alpine/QEMU boot verified:
- Luna kernel 7.2.4 boots Alpine without an initramfs/switch-root chain.
- OpenRC starts Luna UserSession instead of greetd.
- Native UserSession acquires DRM master.
- Native UserSession selects the QEMU display at 1280x800.
- Native UserSession draws the first login frame immediately after KMS activation, before input discovery.
- Native UserSession discovers keyboard and pointer input through evdev.
- Test keyboard input reaches the native UI.
- QEMU mouse movement and button events select user cards and focus the password field.
- In single-user mode the account is selected automatically and the password-only login path succeeds.
- In multi-user mode three local accounts are rendered as selectable avatar cards; the second account was selected by mouse and authenticated successfully.
- Real /etc/shadow authentication succeeds for the selected disposable Alpine test account.
- The credential transition succeeds.
- The native session reaches the graphical desktop launch stage.
- The login renderer loads the canonical Luna font and `docs/design/visual-concepts/luna.png` as low-opacity login artwork.
- Relative pointer movement is accelerated locally for the direct evdev login UI; the desktop compositor keeps its own normal pointer path.
- Forced termination of the Niri child leaves the root UserSession supervisor alive and causes it to reacquire DRM/KMS and return to the native login UI.
- ACPI RSDP is forwarded from the UEFI configuration table into Linux boot_params; the previous ACPI BIOS/RSDP and `wmi-bmof` startup errors are no longer present in the fresh test boot.

The QEMU graphics surface is not directly capturable from the current remote execution environment, so visual pixel verification remains a separate display-capture task.

## Implementation boundaries

- accounts.rs owns local login-account discovery.
- auth.rs owns password verification and conversion to UserCredentials.
- session_ui.rs owns graphical login state, lifecycle and direct input coordination.
- renderer.rs owns the framebuffer renderer, layout geometry and font resource resolution.
- graphics.rs owns the narrow DRM/KMS UAPI required by UserSession.
- input.rs owns the narrow evdev contract required by UserSession.
- keymap.rs owns keyboard modifiers and layout translation; the first implementation is US.
- seat.rs describes the seat/resource view used by UserSession; it is not a second seat daemon.

## Current interaction

User selection:
- one eligible local account: selected automatically, login screen opens directly on the password field
- multiple eligible accounts: local accounts are shown as selectable avatar cards with display name and username
- Enter or a mouse click opens the password screen for the selected account
- Esc from the password screen returns to account selection when multiple accounts exist

Keyboard:
- password entry on the password screen
- US key layout abstraction
- Shift and Caps Lock modifier state
- Backspace
- Tab changes field
- Enter submits
- password content is masked

Pointer:
- relative movement from evdev
- local acceleration for the low-level login pointer
- native cursor
- click selects a user card on the multi-user screen
- click focuses the password field
- click on the round login button submits the password

Visuals:
- visual direction follows `docs/design/visual-concepts/concept_2.png`: deep navy background, blue/cyan accents, sparse controls and orbital Luna branding
- `docs/design/visual-concepts/luna.png` is loaded as a subdued left-side artwork layer
- the renderer draws the Luna orbital logo natively instead of baking the login screen into a single bitmap

Fonts:
- canonical resource path: `/resources/fonts/JetBrainsMonoNerdFontMono-Regular.ttf`
- Alpine foundation mirrors the same asset to `/usr/share/fonts/truetype/luna`
- font loading uses `fontdue`; bitmap glyphs remain only as a boot-safe fallback

## Next native-login work

1. Add complete keyboard-layout selection and non-US layouts.
2. Add richer pointer hover/focus semantics and pointer acceleration policy.
3. Add visual pixel capture verification for the native framebuffer.
4. Separate remaining Alpine service/runtime closure from the legacy greeter package closure.
5. Eventually replace the remaining seatd/elogind dependencies with Luna-owned system responsibilities.

The Alpine foundation remains experimental and must not be promoted to develop until the native login stack is complete.
