# RFC-0001

Status: Accepted

# Filesystem Layout

## Summary

Project Luna uses a simplified filesystem layout designed around readability, immutable system design and clear separation of responsibilities.

## Motivation

Traditional Linux distributions follow the Filesystem Hierarchy Standard (FHS), which contains many historical directories whose original purpose is no longer obvious to modern users.

Project Luna replaces this layout with a smaller and more understandable hierarchy while preserving compatibility where practical.

## Design

The root filesystem consists of the following directories:

```
/
├── system
├── platform
│   ├── apps
│   ├── libs
│   ├── runtime
│   └── config
├── users
├── data
├── cache
├── proc
├── sys
├── dev
├── run
└── tmp
```

### system

Immutable operating system.

Contains only files that belong to the operating system itself.

### platform

Everything installed on top of the operating system.

Applications, runtimes, libraries and shared configuration.

### users

User home directories.

### data

Shared application data.

### cache

Temporary cache data.

### proc, sys, dev, run, tmp

Runtime virtual filesystems.

These directories are created during boot and are not stored permanently.

## Goals

- Human-readable layout.
- Immutable operating system.
- Minimal root directory.
- Clear separation between system and user data.
- Compatibility with Linux userspace where reasonable.

## Alternatives

Keeping the traditional Linux FHS.

Rejected because it introduces unnecessary historical complexity.

## Future

The exact contents of each directory may evolve without changing the overall filesystem philosophy.
