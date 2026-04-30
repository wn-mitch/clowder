---
id: 2026-04-20
title: Phase 2B — Scent as influence map
status: done
cluster: null
landed-at: null
landed-on: 2026-04-20
---

# Phase 2B — Scent as influence map

First §5 influence-map layer landed: scent generalized out of the
one-off wind + sensing implementation into a reusable influence-map
abstraction per spec §5.1 (base maps → templates → working maps).
Unblocks target-taking DSEs that want continuous spatial axes
(Hunt's prey_proximity, Fight's threat proximity, etc.) rather than
the lossy boolean gates used pre-refactor.

**Scope landed:**
- Generalized influence-map grid + propagation + decay primitives
  reusable by other L1 channels.
- Scent migration from `wind.rs`/`sensing.rs` ad-hoc accumulator
  onto the new substrate.

**Still outstanding** (cluster B #6 / B1): corruption field, ward
field, prey-density field, and social-attraction field are each
still one-off or not-yet-built. The L1 abstraction is in place; the
remaining work is per-layer migration + new-layer authoring.

---
