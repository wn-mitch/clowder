---
id: 079
title: "`check_iaus_coherence.sh` process gate against MacGyvered pins"
status: done
cluster: null
landed-at: 8edbae3
landed-on: 2026-04-29
---

# `check_iaus_coherence.sh` process gate against MacGyvered pins

**Landed:** 2026-04-29 | **Commits (1):** 8edbae3 (script + justfile wire-in + transient EXEMPT marker on the 078 pin)

**Why:** The planning-substrate hardening sub-epic (071) pins every defense inside the IAUS engine — `Consideration × Curve`, `Modifier` in §3.5.1 pipeline, or `EligibilityFilter` — and explicitly disallows post-hoc resolver-body overrides ("MacGyvered pins"). 027b Commit B introduced one such pin at `socialize_target.rs:193` (`if pairing_partner == Some(target) { return 1.0; }`); ticket 078 backports it to a `target_pairing_intention` Consideration. Without a process gate, the same anti-pattern reappears next time a contributor wants to "just override the score for this case." This ticket lands the grep-based gate so the discipline holds going forward.

**What landed:**

1. **`scripts/check_iaus_coherence.sh`** (executable). Two passes over `src/ai/dses/*.rs`:
   - Pass 1 (single-line): `^\s*if\s+[^{]+\{\s*return\s+(1\.0|0\.0)\s*;?\s*\}`.
   - Pass 2 (3-line block): same shape across `\n` via `rg -U --multiline-dotall`, post-filtered to the `if` line so the reported file:line points at the right place.
   - **Allowlist marker:** the line immediately preceding an offending `if` containing `IAUS-COHERENCE-EXEMPT` skips the check. Reserved for genuine out-of-economy cases (e.g. constructor-body gate semantics, narrative-injected overrides).
   - Exits 0 with `iaus-coherence: ok …` when clean, exits 1 with offender list + a one-paragraph rationale pointing at ticket 071 + the three engine primitives.
   - Bash 3.2-compatible (no `mapfile`); deduplicates offenders portably.

2. **`justfile`** — `check` recipe extended to chain `bash scripts/check_iaus_coherence.sh` after the existing step-resolver and time-unit linters, mirroring `scripts/check_step_contracts.sh`'s wiring.

3. **Transient EXEMPT marker** at `src/ai/dses/socialize_target.rs:193`. Ticket 078 (running in parallel) removes the underlying pin and the marker disappears with it; coordination-noted with the orchestrator. The marker reads `// IAUS-COHERENCE-EXEMPT: 027b Commit B's MacGyvered Pairing-Intention pin; ticket 078 backports to a target_pairing_intention Consideration and removes this marker.`

**Verification:**

- `just check` green in worktree (script reports `iaus-coherence: ok (1 exempted via // IAUS-COHERENCE-EXEMPT marker)`).
- `just test` green.
- **Regression test (manual, in worktree):** appended a synthetic `if x { return 1.0; }` block to `src/ai/dses/idle.rs` → `just check` exited 1 with `iaus-coherence: MacGyvered pin(s) detected … src/ai/dses/idle.rs:198 (block if-return override)` plus the rationale. Restored cleanly.
- Single-line variant test (`if x { return 0.0; }` on one line) similarly caught and reported as `(single-line if-return override)`.
- Allowlist marker test (synthetic block preceded by `// IAUS-COHERENCE-EXEMPT: <reason>`) → script counted 2 exempted (the real 078 pin + the synthetic), exit 0.

**Out of scope:** Catching MacGyvered patterns outside `src/ai/dses/*.rs` (e.g. `src/systems/goap.rs`); AST-based static analysis. Grep is sufficient for the shape we're enforcing — if false positives accumulate, upgrade later.

---
