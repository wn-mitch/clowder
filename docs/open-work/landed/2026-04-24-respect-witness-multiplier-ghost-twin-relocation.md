---
id: 2026-04-24
title: Respect witness-multiplier — ghost-twin relocation
status: done
cluster: null
landed-at: 608c6e3f
landed-on: 2026-04-24
---

# Respect witness-multiplier — ghost-twin relocation

**Landed:** 2026-04-24 | **Balance:** `docs/balance/respect-restoration.md` iter 1 (post-landing fix)

**Diagnosis.** Commit 608c6e3f (upper-need fillers) added the respect
witness-multiplier writes to `resolve_disposition_chains` in
`src/systems/disposition.rs` at lines 2745–2756 and 2862–2880. That
function is registered only in test schedules (`tests/integration.rs:289`,
`tests/mentor.rs:49`). Production schedules (`src/plugins/simulation.rs:223`,
`src/main.rs:478`) register `resolve_task_chains` from
`src/systems/task_chains.rs:26` instead — which destructures `_needs`
with an underscore prefix, so respect is unreachable. The iter-1
writes never ran in soaks. Batch 2 seed-42 900s post-landing showed
respect mean = 0.291, essentially unchanged from the pre-iter-1
baseline of 0.287.

**Fix.** Moved the witness-multiplier block to `resolve_goap_plans`
plan-completion block (`src/systems/goap.rs:~1812`), right next to
the existing `respect_for_disposition` baseline write — so both the
baseline and the multiplier fire from the same live schedule. Uses
`snaps.cat_positions` (already built by `resolve_goap_plans` for
other purposes), so no new snapshot field required. Made
`count_witnesses_within_radius` public in disposition.rs. Removed
the ghost-twin writes from disposition.rs and the now-unused
`ChainStepSnapshots.positions` field. Test-only schedules (mentor +
integration) still exercise `count_witnesses_within_radius` via the
`respect_witness_tests` module.

**Hypothesis.** Witness-multiplier at plan completion raises colony
respect mean from 0.29 (seed-42 v2 baseline) toward the 0.5–0.7 band
by making nearby-witness plan completions feed the esteem tier.

**Prediction.** Respect mean ↑ ≥ 0.2 on seed-42 15-min soak. Respect
`=0%` (percentage of snapshots at zero) ↓ from 67.1% toward < 30%.

**Observation.** Pending — to be filled in on the next post-rebuild
seed-42 soak.

**Files:** `src/systems/goap.rs` (+19), `src/systems/disposition.rs`
(-44), `docs/balance/respect-restoration.md` (updated iter-1 with
relocation note).

---
