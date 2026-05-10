# Comment Staleness Audit — 2026-05-10

Follow-up to the 2026-04-26 audit. Scope: ~14 days of landings (tickets
186–250, Phase A1.2/A5 trace enrichment, IntentionMomentum wiring via
246/247/248).

---

## Context

Key landings since 2026-04-26 that drive the staleness:

| Landing | Ticket / commit | Effect on comments |
|---|---|---|
| IntentionMomentum scalar wiring | 246 (2026-05-08) | §7.4 persistence bonus is now live |
| L3 adoption-site lift + timing fix | 247 / 248 | Preempt formula is substrate-correct |
| BurialPerformed canary demotion | 250 | Canary set shrinks; CLAUDE.md updated |
| Phase A1.2 (A5) — at-source L2/L3 trace | 2026-04-23 | L2 `considerations`/`modifiers` are real; L3 softmax is real |
| Phase 6a — commitment gate | 2026-04-24 | Was "deferred"; did land |
| Ticket 185 PickingUp DSE | landed/ | Discarding's "deferred to 185" is stale |
| Ticket 188 Handing DSE | landed/ | Discarding's "deferred to 188" is stale |
| Ticket 014 Phase-4 follow-ons closeout | 453ea83 | Fight/mentor "open-work #14" reference is closed |

---

## Findings

### A — Headless I/O plugin docstring (headless_io.rs:8–13, :43)

**File:** `src/plugins/headless_io.rs`

Lines 8–13 describe a transitional state that no longer exists: "Phase C is
additive. The plugin … is not yet mounted by `run_headless` — phase D rewrites
`run_headless`…". Phase D has landed: `src/main.rs:388` calls
`app.add_plugins(HeadlessIoPlugin)`, and the legacy `build_schedule` /
`flush_*_entries` path is gone.

Line 43 carries the same stale framing: "The host (`run_headless`
post-phase-D) parses argv…" — the qualifier is now historical noise.

**Fix:** Drop the Phase C/D transitional paragraph (lines 8–13). Rewrite
line 43 to drop "post-phase-D".

---

### B — `discarding.rs` deferred-to 185/188 (discarding.rs:4–5)

**File:** `src/ai/dses/discarding.rs`

Module doc says "Handing (give to peer — deferred to 188), and PickingUp
(retrieve ground item — deferred to 185)." Both tickets are in
`docs/open-work/landed/`.

**Fix:** Change "deferred to" → "landed in" for both references.

---

### C — Fight/mentor target-range: closed ticket reference (fight_target.rs:98, mentor_target.rs:83)

**Files:** `src/ai/dses/fight_target.rs`, `src/ai/dses/mentor_target.rs`

Both say the candidate-pool range "is a balance decision deferred to
post-refactor per open-work #14." Ticket 014 closed on 2026-04-27 without
resolving the range value explicitly (its scope covered §6.5 target-taking
ports and §4 marker authoring; the range tuning is an open balance question,
not blocked on 014). Readers following "open-work #14" hit a closed ticket
with no pointer forward.

**Fix:** Drop the ticket reference; say the range is a balance decision
that is still open.

---

### D — §7.4 persistence bonus "not yet wired" (eval.rs:857, commitment.rs:270–271)

**Files:** `src/ai/eval.rs`, `src/ai/commitment.rs`

`eval.rs:857` says "§7.4's persistence bonus (not yet wired)". Ticket 246
wired the `IntentionMomentum` modifier's three scalars from `HeldIntention`;
ticket 248 fixed the L3 adoption-site timing so `held_score` now reflects the
modifier's lift. The bonus is wired.

`commitment.rs:270–271` says the elastic achievability channel is "deferred
to §7.4 — wiring it here without the persistence bonus risks OpenMinded
activities thrashing." The elastic channel is still not wired (still
always-true at line 360), but the stated reason ("without the persistence
bonus") is outdated — the bonus is live. The elastic channel's open status
remains under its own §7.4 balance thread.

**Fix:**
- `eval.rs:857`: update "not yet wired" → note that 246+248 wired it.
- `commitment.rs:270–271`: drop the "without the persistence bonus" rationale; state the elastic channel is still deferred as a balance follow-on.

---

### E — Trace emitter "Phase 1 shim" docstrings (trace_emit.rs:21–32)

**File:** `src/systems/trace_emit.rs`

Module doc says:
- L2: "considerations/modifiers empty. Phase 3's Dse trait lets the emitter
  capture per-consideration contributions." — Phase A1.2 (A5) landed at-source
  capture; `considerations` and `modifiers` are now populated from live
  `_with_trace` variants (confirmed at trace_emit.rs:377–426).
- L3: "placeholder softmax / momentum summaries. Phase 6 fills in real
  softmax probabilities." — Phase A1.2 filled in real softmax data.
  The **momentum** summary (`commitment_strength`) is still 0.0 — that
  remains a genuine placeholder.

**Fix:** Update L2 description to reflect real data is captured. Split L3
description so softmax (real) and momentum (still placeholder) are distinct.

---

### F — refactor-plan.md Phase 6a status (refactor-plan.md:942–958)

**File:** `docs/systems/refactor-plan.md`

The Phase 6a section opens with "**Status (2026-04-23 PM): Attempted and
deferred after soak regression.**" Phase 6a subsequently landed on 2026-04-24
(see `docs/open-work/landed/2026-04-24-phase-6a-7-commitment-gate-resolve-goap-plans-split.md`).
The status block accurately describes the investigation into the LLVM cliff
that caused the original regression, but implies the work is still deferred
when it is not.

**Fix:** Add "✅ **Landed 2026-04-24**" annotation to the status block;
preserve the regression investigation narrative as historical context.

---

### G — simulation.rs `build_schedule` + "CLAUDE.md Headless Mode section" (simulation.rs:709–710)

**File:** `src/plugins/simulation.rs`

Comment says the trace emitter is "Registered here (not just in
`build_schedule`) to satisfy the manual-mirror invariant in CLAUDE.md's
Headless Mode section." `build_schedule` is retired; CLAUDE.md has no
"Headless Mode section" (the headless mode is mentioned under "Verification"
and "Daily", with no manual-mirror invariant).

**Fix:** Drop the `build_schedule` and CLAUDE.md references; explain why
the system is registered in `SimulationPlugin` rather than `HeadlessIoPlugin`.

---

### H — groom_self.rs warmth-split forward reference (groom_self.rs:44–46)

**File:** `src/ai/dses/groom_self.rs`

Comment says "the affection axis lands when `needs.warmth` splits into
thermal + affection." The split has happened: `needs.warmth` was renamed
to `needs.temperature` (pre-flight gate 4) and `Fulfillment.social_warmth`
is now the affection axis (ticket 012). The GroomSelf DSE has not yet been
extended to use `social_warmth` as a second consideration, but the
precondition described in the comment ("when `needs.warmth` splits") has
already occurred.

**Fix:** Update to reference the existing `Fulfillment.social_warmth` axis
and note the TODO is about wiring it, not waiting for the split.

---

## Pre-existing items

**PE-002** (dead features in activation tracker) — `FoxDenEstablished`,
`FoxDenDefense`, `CombatResolved` still defined in `system_activation.rs`
with no `activation.record()` call. `expected_to_fire_per_soak()` returns
`false` for the Fox entries. Status unchanged: still open/blocked.

**PE-003** (substrate stub catalogue) — still `in-progress`. No change.

---

## Out of scope

- `docs/balance/*.md` — iteration logs, append-only
- `composition.rs:12` "no in-tree DSE registers with Max post-3c" — still
  accurate; `Composition::max` is test-only
- `trace_log.rs:220–222` "Phase 1 emits best-effort shape" for
  `MomentumSummary` — still accurate; `commitment_strength` is hard-coded
  to 0.0 in `trace_emit.rs:335`
- `pairing.rs:67` / `ai/pairing.rs:59` "ReproduceAspiration … not yet
  authored" — still accurate; no `.ron` file exists; Phase 5 owns this
- `personality_events.rs` `register_observers_world` — dead code since
  ticket 030 landed but not deleted; the function's docstring says "Retired
  in ticket 030 once headless moves to the unified App pipeline" which is
  now past-tense accurate. Removing the dead function is a code change,
  not a comment change — left for a cleanup ticket
- Retrospective sections in phase-6a-commitment-gate-attempt.md
- §13.1 "retired consideration" annotations — intentional design history

---

## Files modified

| File | Finding |
|---|---|
| `src/plugins/headless_io.rs` | A |
| `src/ai/dses/discarding.rs` | B |
| `src/ai/dses/fight_target.rs` | C |
| `src/ai/dses/mentor_target.rs` | C |
| `src/ai/eval.rs` | D |
| `src/ai/commitment.rs` | D |
| `src/systems/trace_emit.rs` | E |
| `docs/systems/refactor-plan.md` | F |
| `src/plugins/simulation.rs` | G |
| `src/ai/dses/groom_self.rs` | H |

---

## Verification

```
just check          # cargo check + clippy + step-contract + substrate-stub lints
cargo test --test integration
```

No logic changes; all edits are comment-only.
