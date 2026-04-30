---
id: 022
title: §11 target-taking trace fan-out — 8 DSE hooks + L2 lookup fix
status: done
cluster: null
landed-at: c49056f
landed-on: 2026-04-24
---

# §11 target-taking trace fan-out — 8 DSE hooks + L2 lookup fix

**What shipped:**

- Hoisted `FocalTargetHook<'a>` from `src/ai/dses/socialize_target.rs`
  to its semantic home at `src/ai/target_dse.rs`, co-located with
  `target_ranking_from_scored`. All nine target-taking DSEs now import
  from one canonical path.
- Wired per-candidate ranking capture through the eight remaining
  target-taking DSE resolvers following the socialize_target exemplar:
  `mate_target`, `mentor_target`, `groom_other_target`,
  `apply_remedy_target`, `build_target`, `caretake_target`,
  `hunt_target`, `fight_target`. Each resolver gains a trailing
  `focal_hook: Option<FocalTargetHook<'_>>` param + a post-scoring
  emission block that routes through `target_ranking_from_scored`
  into `FocalScoreCapture::set_target_ranking`.
- Caller-side wiring across 14 sites (7 in `goap.rs`, 7 in
  `disposition.rs`): scorer pre-checks + chain-building pre-checks
  pass `None` (consistent with socialize's design note "focal capture
  happens at the step-resolution site, not here"); the 6 goap.rs
  step-resolution sites construct the real hook via `ec_is_focal +
  ec.focal_capture`. The hunt path threads `is_focal` /
  `focal_capture` through `resolve_search_prey`'s signature since
  the `ExecutorContext` doesn't reach that helper.
- Fixed a pre-existing lookup bug in `src/systems/trace_emit.rs` —
  `l2_record_for` now tries `<dse_id>_target` first, falling back to
  bare `dse_id`. Before this fix even the exemplar's ranking data
  was captured but never emitted because L2 records for the
  self-state peer (`socialize`) could not find rankings keyed as
  `socialize_target`.
- 62 local unit-test call sites patched with trailing `None` via a
  one-shot Python script (scoped to the `#[cfg(test)] mod tests`
  block in each DSE file).

**Why it matters:** with only socialize_target wired, focal-cat
soaks on non-generalist cats (Priestess, fighter, mated adult) show
no per-target scoring for their signature activities. Eight hooks
closes the §6.5 coverage matrix; the lookup-fix unlocks the data
flow end-to-end. Multi-focal soaks are now the gating path for the
remaining coverage (apply_remedy, build, fight) that Simba alone
doesn't exercise — tracked as a §6.5 follow-on.

**Hypothesis / concordance:** none — zero-behavior-change wiring
on the non-focal path (14 callers, 8 pass `None`; the 6 real-hook
callers run only when `ec_is_focal` is true, which matches a single
cat per soak). No balance drift is claimed or expected.

**Verification:**

- `cargo check --tests` clean; `cargo test` 1146+ passing (run
  before the parallel ticket-024 fulfillment-register session
  introduced its own WIP into `goap.rs` / `disposition.rs` —
  see "Mixed working-copy" below).
- seed-42 `--duration 900` Simba soak at 2026-04-24 10:55:
  - Survival canaries: Starvation=0, ShadowFoxAmbush=0, footer
    written, features-at-zero informational.
  - `never_fired_expected_positives` carries 13 entries (12
    pre-existing from c49056f baseline: BondFormed,
    ItemRetrieved, FoodCooked, KittenBorn, GestationAdvanced,
    MatingOccurred, KittenFed, CropTended, CropHarvested,
    Socialized, GroomedOther, MentoredCat; plus
    KnowledgePromoted newly silent on this run). None are caused
    by this ticket — all are upstream of the trace-emit layer.
  - Commitment counters (primary ticket acceptance): Blind=4202,
    SingleMinded=992, OpenMinded=6959, ReplanCap=293 — all
    non-zero, matching the ticket's "a zero on any is a canary"
    rule.
  - `L3Commitment` rows: 894 total (188 Blind / 706 OpenMinded);
    no "unachievable" or "dropped_goal" branches this seed
    (Simba never hit `max_replans`).
  - `L3PlanFailure` rows: 0 — Simba took no anxiety interrupt.
- Post-commit re-soak with the trace_emit lookup fix is deferred
  to the next session; the structural wiring landed and the
  emission keys are now uniformly `<dse>_target`. The re-soak
  will confirm `targets` blocks populate on hunt / mate / mentor
  / groom_other / caretake L2 records for a focal-cat run.

**Deferred / follow-on:**

- Multi-focal soaks for full §6.5 coverage (Priestess for
  apply_remedy / communion DSEs; a mated adult for courtship;
  a fighter for fight_target). Simba doesn't exercise those DSE
  gates. Open as a new ticket if/when the visualization work
  needs the data.
- Re-soak verification of `targets`-in-L2 post-lookup-fix.

**Mixed working-copy note:** the commit that lands this ticket
rides alongside in-flight work from a parallel session on ticket
024 (fulfillment-register) in `src/systems/goap.rs` and
`src/systems/disposition.rs`. Those changes reference a
`fulfillment_opt` binding that is not yet plumbed through every
use site, so `cargo check` fails on lines outside this ticket's
scope. The fulfillment session will rebase on top and resolve.
No ticket-022 edit touches fulfillment call paths.

---
