# Luna Principles

These principles define the architecture and long-term direction of Project Luna.

## 1. Documentation Before Implementation

Architecture is designed before code is written.

Every significant change begins with documentation or an RFC.

---

## 2. Simplicity Over Complexity

Every component should remain as simple as possible.

Complexity must always be justified.

---

## 3. One Component — One Responsibility

Each system component solves one clearly defined problem.

---

## 4. Immutable Core

The operating system itself remains read-only during normal operation.

---

## 5. Human Readability

Directories, configuration files and system structure should be understandable without reading documentation.

---

## 6. Explicit Over Implicit

Hidden behavior should be avoided.

The system should always behave predictably.

---

## 7. Recovery First

Every system update must be reversible.

Recovery is part of the architecture, not an afterthought.

---

## 8. Rust First

All new system software is written in Rust whenever practical.

---

## 9. Wayland Only

Project Luna is designed exclusively for Wayland.

Legacy graphical systems are outside the project's scope.

---

## 10. Small Core

Only the operating system belongs inside the immutable system image.

Applications, libraries and user data remain outside the core system.

---

## 11. Architecture Before Compatibility

Compatibility is valuable.

Architecture is more valuable.

Legacy behavior should never prevent a better design.

---

## 12. Long-Term Maintainability

Every decision should make the system easier to maintain five years from now.
