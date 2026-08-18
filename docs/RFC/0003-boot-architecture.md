# RFC-0002: Luna Boot Architecture

- Status: Draft
- Type: Architecture
- Version: 1
- Date: 2026-08-09

## Abstract

This document defines the boot architecture of Project Luna.

The Luna boot architecture is designed around a minimal UEFI bootloader,
independent system images, independent Linux kernels, atomic boot selection,
boot confirmation, rollback, and recovery.

The bootloader must remain small and independent from the installed Luna
system.

System Images and Linux kernels are versioned independently.

A failed System Image may be replaced by a previous known-good System Image
without reboot when the kernel and early runtime remain operational.

A failed kernel requires a reboot and is handled by the UEFI bootloader
during the next boot cycle.
