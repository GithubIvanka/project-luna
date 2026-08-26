# Project Luna — Phase 1.4

## Status

**In progress; accepted decisions A–T are consolidated into `docs/ARCHITECTURE.md`.**

## Accepted decisions

### 1.4-A
Accepted option A from the phase discussion.

### 1.4-B
Accepted option E from the phase discussion.

### 1.4-C
Accepted option D from the phase discussion.

### 1.4-D
Accepted option E from the phase discussion.

### 1.4-E
Accepted. Checkpoints are the mechanism for the agreed recovery/rollback purpose.

### 1.4-F
Accepted combined C/E direction.

### 1.4-G
Untrusted/externally modified application metadata must not be silently trusted. The user receives a warning and can choose to close, launch once, or create an explicit local trust/signature. Trust is an intentional user action.

### 1.4-H
Accepted.

### 1.4-I
Accepted multi-level behavior rather than a binary yes/no model.

### 1.4-J
Accepted D + E.

### 1.4-K
Accepted.

### 1.4-L
Accepted.

### 1.4-M — shared libraries
Use one shared library instance where possible, allowing multiple applications to consume it rather than duplicating identical libraries in every bundle.

### 1.4-N — volumes
`DATA/system/volumes` is the managed representation of connected volumes. The file manager gets a dedicated Volumes view, analogous to the Apps view. A volume such as `fleshka` is presented as `DATA/system/volumes/fleshka` internally and as a friendly volume in the UI. Network volumes may be added later.

### 1.4-O — removable media
External media should be immediately usable from the file manager without manual mounting. Automatic execution from USB/removable media is disabled by default or controlled by explicit policy, preventing silent execution of malicious programs.

### 1.4-P
Accepted option D.

### 1.4-Q
Accepted.

### 1.4-R
Accepted option D.

### 1.4-S — application launch ownership
Application execution is moved completely out of `luna-app-manager` and into the runtime/system-runtime chain. App Manager owns installation, update, removal, verification and migrations.

The App Manager may accept `.deb` and `.rpm` packages and convert/install them into Luna bundles with the appropriate manifest and metadata.

### 1.4-T — protected administration and user privacy
Accepted architectural direction:

- users cannot simply open other users' private data without authorization;
- an administrator/system credential can authorize administrative operations;
- a user can be downgraded from administrator to ordinary user;
- restoring administrative authority requires the appropriate credential rather than a root account.

The exact credential storage, authentication and recovery protocol remain open for the security specification.

## Organizational rule

Every accepted phase decision must be consolidated into `docs/ARCHITECTURE.md`. Phase files preserve chronology and reasoning but are not independent Sources of Truth.
