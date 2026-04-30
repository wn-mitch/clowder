---
id: 2026-04-24
title: "Acceptance & mastery restoration — iteration 1"
status: done
cluster: null
landed-at: 00deb6f
landed-on: 2026-04-24
---

# Acceptance & mastery restoration — iteration 1

**What shipped (three commits planned; see commit messages for the split):**

- Frontend schema-drift fix: narrative-editor `NeedsBlock` renamed
  `warmth → temperature` to match the sim's post-warmth-split-phase-2
  field name (commit `00deb6f`), plus new `social_warmth` column
  (reads 0 until warmth-split phase 3). Updates in `types.ts`,
  `metrics.ts`, `LogsDashboard.svelte`, `CatDetailPanel.svelte`.
- Prey-by-species stacked chart: NaN guard at
  `StackedSpeciesChart.svelte:60` — missing species field no longer
  propagates NaN through the cumulative stack.
- Acceptance restorers at recipient-side, witnessed-effect gates:
  - Grooming recipient → `+acceptance_per_groomed = 0.08` (post-loop
    `grooming_restorations` consumer; same gate as the grooming-
    condition delta).
  - Kitten being fed → `+acceptance_per_kitten_fed = 0.10` (post-loop
    `kitten_feedings` consumer; same gate as the hunger restoration).
- Mastery restorers on witnessed action success:
  - `fight_threat` completion → `+fight_mastery_gain = 0.03`.
  - `survey` completion → `+survey_mastery_gain = 0.02`.

**Why it matters:** seed-42 15-min soak chart showed acceptance and
mastery both pinned at 0 for the full run — acceptance via
`needs.rs:131-133` drain with no restorer anywhere in the codebase;
mastery via drain outpacing the two existing restorers (mentor-cat
requires apprentice, aspirations milestone rare). Both pin-at-0s cascade
through Maslow level-suppression in `colony_score.rs:148-156`, dragging
colony welfare down regardless of actual health. See
`docs/balance/acceptance-restoration.md` and
`docs/balance/mastery-restoration.md` for hypotheses and concordance.

**Deferred to a follow-on iteration:**

- Apprentice-side acceptance on MentorCat — blocked on
  `unchained_skills` query signature change that collided with the
  parallel §Phase 6a split refactor work in `resolve_disposition_chains`.
- Mastery at high-cadence resolvers (`resolve_cook`, `resolve_construct`,
  `resolve_tend`, `resolve_harvest`, `resolve_gather_herb`,
  `resolve_set_ward`, `resolve_cleanse_corruption`, `resolve_repair`,
  `resolve_apply_remedy`, `resolve_prepare_remedy`) — same blocker:
  adding `&mut Needs` to signatures is risky mid-parallel-refactor.
  These are the plug-points that should deliver the bulk of mastery
  recovery; iteration 1's two plug-points are the signature-safe
  subset.
- Gossip-subject acceptance; gift-receipt acceptance — firing-
  frequency unclear, verify before wiring.

**Verification:**

- `cargo check` clean; `cargo test --lib` 1126/1126 passing.
- `just check` fails **from pre-existing clippy `needless_borrow`
  lints in `dispatch_chain_step` body (lines 3029-3756)** — those are
  artifacts of the parallel session's Phase 6a split and not from
  this work. None in my changed files.
- Seed-42 `--duration 900` release soak A/B vs c49056f baseline:
  Starvation = 0, ShadowFoxAmbush = 0, footer written, never-fired
  list identical (12). **Mastery**: 0.000 → 0.998–1.000 across all 8
  cats (direction match, **magnitude 3–10× over predicted** — follow-on
  required to drop `survey_mastery_gain` to ~0.002). **Acceptance**:
  unchanged at 0.000 because `Feature::GroomedOther` + `Feature::KittenFed`
  are in the never-fired-expected list on both runs — the witness
  gates my restorers hang on structurally don't fire in seed-42.
  Colony **welfare rose from end-of-soak 0.537 → 0.640 (+19%)** —
  mastery unblocking Maslow esteem-tier suppression in
  `colony_score.rs:148-156`.

**Iteration 2 follow-ons:**

- Drop `survey_mastery_gain` 0.02 → 0.002 to land a natural
  distribution instead of saturation.
- Diagnose why Feature::GroomedOther/KittenFed/Socialized/MentoredCat
  never fire in seed-42. Urgency interrupts (CriticalSafety
  preempting level 4/5 plans) jumped 3 → 34 in treatment; grooming
  duration is 80 ticks and cats appear to abandon partway. The
  structural question is whether the 12-feature never-fired cluster
  is a single upstream problem (DSE gating, plan completion,
  interrupt sensitivity) or independent.
- Wire acceptance-per-tick on the per-tick portion of
  `resolve_groom_other` / `resolve_feed_kitten`, not on completion
  (pragmatic workaround if the completion gate stays dormant).
