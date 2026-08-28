# luna-boot

Minimal UEFI bootloader for Project Luna.

## Status

Early prototype - implements basic boot path for x86_64.

## Features

- UEFI application (x86_64-unknown-uefi target)
- Minimal boot flow: UEFI → kernel → handoff
- B key detection for boot menu
- Simple text-mode menu
- Recovery boot target
- Fallback target selection
- UEFI memory map handling
- ExitBootServices lifecycle

## Building

```bash
# Add UEFI target
rustup target add x86_64-unknown-uefi

# Build
cd boot/luna-boot
cargo build --release

# Output: target/x86_64-unknown-uefi/release/luna-boot.efi
