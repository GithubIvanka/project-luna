# Phase 1.6 — Architecture Consolidation, Repository Reset & Crate Planning

**Status:** ACCEPTED / CONSOLIDATED
**Phase:** 1.6
**Project:** Project Luna
**Source of Truth:** `docs/ARCHITECTURE.md`
**Record purpose:** Preserve the accepted decisions of Phase 1.6 independently from chat history.

---

## 1. Purpose of Phase 1.6

Phase 1.6 moves Project Luna from architecture discussion toward an implementation-ready repository structure.

The phase establishes:

- consolidation of the architectural decisions accumulated through Phase 1.5;
- a clean repository instead of carrying obsolete empty crates;
- a small, deliberate foundation crate model;
- rules for designing the final crate map from the architecture rather than from the historical repository layout;
- the beginning of the implementation boundary between architecture, APIs, and code.

The architecture remains the authority. A crate must not be created merely because an old component name existed in an earlier phase.

---

# 2. Accepted decision ledger

The following decisions were explicitly accepted during Phase 1.6. The letter identifiers are preserved because they correspond to the Phase 1.6 discussion in the project history.

## 1.6-A → 1.6-HZ

### 1.6-A
Accepted.

### 1.6-B
Accepted.

### 1.6-C
Accepted.

### 1.6-D
Accepted.

### 1.6-E
Accepted.

### 1.6-F
Accepted.

### 1.6-G
Accepted.

### 1.6-H
Accepted.

### 1.6-I
Accepted.

### 1.6-J
Accepted.

### 1.6-K
**B — accepted.**

### 1.6-L
Accepted.

### 1.6-M
Accepted.

### 1.6-N
Accepted.

### 1.6-O
Accepted.

### 1.6-P
Accepted.

### 1.6-Q
Accepted.

### 1.6-R
Accepted.

### 1.6-S
Accepted.

### 1.6-T
Accepted.

### 1.6-U
Accepted.

### 1.6-V
Accepted.

### 1.6-W
Accepted.

### 1.6-X
Accepted.

### 1.6-Y
Accepted.

### 1.6-Z
Accepted.

### 1.6-AA
**B — accepted.**

### 1.6-AB
Accepted.

### 1.6-AC
Accepted.

### 1.6-AD
Accepted.

### 1.6-AE
Accepted.

### 1.6-AF
Accepted.

### 1.6-AG
Accepted.

### 1.6-AH
Accepted.

### 1.6-AI
Accepted.

### 1.6-AJ
Accepted.

### 1.6-AK
Accepted.

### 1.6-AL
Accepted.

### 1.6-AM
Accepted. The obsolete historical components may be removed and recreated later from a clean slate according to the final architecture. Existing code is not assumed to remain architecturally correct merely because it already exists.

### 1.6-AN
Accepted.

### 1.6-AO
Accepted.

### 1.6-AP
Accepted.

### 1.6-AQ
Accepted.

### 1.6-AR
Accepted.

### 1.6-AS
Accepted.

### 1.6-AT
Accepted.

### 1.6-AU
Accepted.

### 1.6-AV
Accepted.

### 1.6-AW
Accepted.

### 1.6-AX
Accepted.

### 1.6-AY
Accepted.

### 1.6-AZ
Accepted.

### 1.6-BA
Accepted.

### 1.6-BB
Accepted.

### 1.6-BC
Accepted.

### 1.6-BD
Accepted.

### 1.6-BE
Accepted.

### 1.6-BF
Accepted.

### 1.6-BG
Accepted.

### 1.6-BH
Accepted.

### 1.6-BI
Accepted.

### 1.6-BJ
Accepted.

### 1.6-BK
Accepted.

### 1.6-BL
Accepted.

### 1.6-BM
Accepted.

### 1.6-BN
Accepted.

### 1.6-BO
Accepted.

### 1.6-BP
Accepted.

### 1.6-BQ
Accepted.

### 1.6-BR
Accepted.

### 1.6-BS
Accepted.

### 1.6-BT
Accepted.

### 1.6-BU
Accepted.

### 1.6-BV
Accepted. The proposed repository/crate organization is to be implemented only after the architectural contract is consolidated.

### 1.6-BW
Accepted.

### 1.6-BX
Accepted.

### 1.6-BY
Accepted.

### 1.6-BZ
Accepted.

### 1.6-Ca
Accepted.

### 1.6-Cb
Accepted.

### 1.6-Cc
Accepted.

### 1.6-Cd
Accepted.

### 1.6-Ce
Accepted. If a client requires it, a separate client crate may be created rather than forcing client-specific functionality into a shared foundation crate.

### 1.6-Cf
Accepted.

### 1.6-Cg
Accepted.

### 1.6-Ch
Accepted.

### 1.6-Ci
Accepted.

### 1.6-Cj
Accepted.

### 1.6-Ck
Accepted.

### 1.6-Cl
Accepted.

### 1.6-Cm
Accepted.

### 1.6-Cn
Accepted.

### 1.6-Co
Accepted.

### 1.6-Cp
Accepted.

### 1.6-Cq
Accepted.

### 1.6-Cr
Accepted.

### 1.6-Cs
Accepted.

### 1.6-Ct
Accepted.

### 1.6-Cu
Accepted.

### 1.6-Cv
Accepted.

### 1.6-Cw
Accepted.

### 1.6-Cx
Accepted.

### 1.6-Cy
Accepted.

### 1.6-Cz
Accepted.

### 1.6-Da
Accepted.

### 1.6-Db
**Accepted: Bin + lib.**

### 1.6-Dc
Accepted.

### 1.6-Dd
Accepted.

### 1.6-De
Accepted.

### 1.6-Df
Accepted.

### 1.6-Dg
Accepted.

### 1.6-Dh
Accepted.

### 1.6-Di
**Accepted: Tokio.**

Tokio is the selected asynchronous runtime direction for the Rust implementation where an async runtime is required.

### 1.6-Dj
Accepted.

### 1.6-Dk
Accepted.

### 1.6-Dl
Accepted.

### 1.6-Dm
Accepted.

### 1.6-Dn
Accepted.

### 1.6-Do
Accepted.

### 1.6-Dp
Accepted.

### 1.6-Dq
Accepted.

### 1.6-Dr
Accepted.

### 1.6-Ds
Accepted.

### 1.6-Dt
Accepted.

### 1.6-Du
Accepted.

### 1.6-Dv
Accepted.

### 1.6-Dw
Accepted.

### 1.6-Dy
Accepted.

### 1.6-Dz
Accepted.

### 1.6-Ea
**C — accepted.**

### 1.6-Eb
Accepted.

### 1.6-Ec
Accepted.

### 1.6-Ed
Accepted.

### 1.6-Ee
Accepted.

### 1.6-Ef
Accepted.

### 1.6-Eg
Accepted.

### 1.6-Eh
Accepted.

### 1.6-Ei
Accepted.

### 1.6-Ej
Accepted.

### 1.6-Ek
Accepted.

### 1.6-El
Accepted.

### 1.6-Em
Accepted.

### 1.6-En
Accepted.

### 1.6-Eo
Accepted.

### 1.6-Ep
Accepted.

### 1.6-Eq
Accepted.

### 1.6-Er
Accepted.

### 1.6-Es
Accepted.

### 1.6-Et
Accepted.

### 1.6-Eu
Accepted.

### 1.6-Ev
Accepted.

### 1.6-Ew
Accepted.

### 1.6-Ex
Accepted.

### 1.6-Ey
Accepted.

### 1.6-Ez
Accepted.

### 1.6-Fa
Accepted.

### 1.6-Fb
Accepted.

### 1.6-Fc
Accepted.

### 1.6-Fd
Accepted.

### 1.6-Fe
Accepted.

### 1.6-Ff
Accepted.

### 1.6-Fg
Accepted.

### 1.6-Fh
Accepted.

### 1.6-Fi
Accepted.

### 1.6-Fj
Accepted.

### 1.6-Fk
Accepted.

### 1.6-Fl
Accepted.

### 1.6-Fm
Accepted.

### 1.6-Fn
Accepted.

### 1.6-Fo
Accepted.

### 1.6-Fp
Accepted.

### 1.6-Fq
Accepted.

### 1.6-Fr
Accepted.

### 1.6-Fs
Accepted.

### 1.6-Ft
Accepted.

### 1.6-Fu
Accepted.

### 1.6-Fv
Accepted.

### 1.6-Fw
Accepted.

### 1.6-Fx
Accepted.

### 1.6-Fy
Accepted.

### 1.6-Fz
Accepted.

### 1.6-Ga
Accepted.

### 1.6-Gb
Accepted.

### 1.6-Gc
Accepted.

### 1.6-Gd
Accepted.

### 1.6-Ge
Accepted.

### 1.6-Gf
Accepted.

### 1.6-Gg
Accepted.

### 1.6-Gh
Accepted.

### 1.6-Gi
Accepted.

### 1.6-Gj
Accepted.

### 1.6-Gk
Accepted.

### 1.6-Gl
Accepted.

### 1.6-Gm
Accepted.

### 1.6-Gn
Accepted.

### 1.6-Go
Accepted.

### 1.6-Gp
Accepted.

### 1.6-Gq
Accepted.

### 1.6-Gr
Accepted.

### 1.6-Gs
Accepted.

### 1.6-Gt
Accepted.

### 1.6-Gu
Accepted.

### 1.6-Gv
Accepted.

### 1.6-Gw
Accepted.

### 1.6-Gx
Accepted.

### 1.6-Gy
Accepted.

### 1.6-Gz
Accepted.

### 1.6-HA
Accepted.

### 1.6-HB
Accepted.

### 1.6-HC
Accepted.

### 1.6-HD
Accepted.

### 1.6-HE
Accepted.

### 1.6-HF
Accepted.

### 1.6-HG
Accepted.

### 1.6-HH
Accepted.

### 1.6-HI
Accepted.

### 1.6-HJ
Accepted.

### 1.6-HK
Accepted.

### 1.6-HL
Accepted.

### 1.6-HM
Accepted.

### 1.6-HN
Accepted.

### 1.6-HO
Accepted.

### 1.6-HP
Accepted.

### 1.6-HQ
Accepted.

### 1.6-HR
Accepted.

### 1.6-HS
Accepted.

### 1.6-HT
Accepted.

### 1.6-HU
Accepted.

### 1.6-HV
Accepted.

### 1.6-HW
Accepted.

### 1.6-HX
Accepted.

### 1.6-HY
Accepted.

### 1.6-HZ
Accepted.

---

# 3. Repository policy established by Phase 1.6

The repository must reflect the current architecture rather than historical component names.

Old empty crates may be deleted. New crates are created only when their responsibility is defined by the architecture and their API boundary is understood.

The current repository was intentionally reduced to the surviving `luna-common` implementation so that it can be audited before being replaced or reused.

Existing code is considered historical until it passes the architecture audit.

---

# 4. `luna-common` policy

`luna-common` remains as the current foundation crate for audit purposes.

It must remain small and must not become a dumping ground for unrelated functionality.

The existing implementation is not automatically considered final. Its useful concepts and code may be retained, redesigned, or removed after the Phase 1.6 repository audit.

A separate client crate may be introduced when client-specific functionality is actually required.

---

# 5. Async implementation direction

The project explicitly accepts Tokio as the Rust async-runtime direction where an async runtime is needed.

This does not mean every crate must depend on Tokio. Dependencies remain responsibility-driven. Lower-level crates should not acquire a runtime dependency merely because another higher layer uses one.

---

# 6. Crate API shape

Where a crate serves both a library/backend role and an executable client role, the accepted direction is **bin + lib** rather than duplicating the implementation.

The library owns reusable functionality and the binary is a thin entry point/client where appropriate.

---

# 7. Next phase boundary

Phase 1.6 architecture decisions are complete through **1.6-HZ**.

The next work is not another architecture-question loop. The implementation workflow is:

1. audit the actual repository;
2. audit `Cargo.toml` and the surviving `luna-common` code;
3. reconcile README/STATUS/ROADMAP with the actual repository state;
4. derive the new crate map from `docs/ARCHITECTURE.md`;
5. define crate responsibilities and API boundaries;
6. only then create the first new implementation crate.

`RFC-0002` / Bundle Format v1 remains a separate design task and must not be accepted merely because an earlier proposal exists.

---

# 8. Source-of-truth rule

This file preserves the Phase 1.6 decision history.

`docs/ARCHITECTURE.md` remains the architectural Source of Truth.

If a later decision changes a Phase 1.6 decision, the architecture must be updated and the change must be recorded in the relevant phase/change record rather than silently rewriting history.
