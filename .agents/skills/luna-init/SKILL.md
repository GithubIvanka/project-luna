---
name: luna-init
description: Implement and debug luna-init as the real persistent PID 1 and bridge from boot handoff to system runtime.
---
# Luna Init
`luna-init` is PID 1 for the entire normal system lifetime.
Validate the read-only FD 3 boot handoff before using its contents.
Use the selected System Image, DATA binding, kernel identity, and boot mode from the handoff.
LUNA-SYS is ext4; LUNA-DATA is Btrfs.
Build the logical root from approved System Image and DATA resources without whole-image copying.
Start `luna-system-runtime` as a child; never replace PID 1 with it.
Provide correct shutdown/reboot lifecycle and child reaping.
Recovery uses the normal System Image/init/kernel plus virtual DATA from a Recovery DATA Image.
