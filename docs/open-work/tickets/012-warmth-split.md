---
id: 012
title: Warmth split — temperature need vs social-warmth fulfillment axis
status: in-progress
cluster: null
added: 2026-04-21
parked: null
blocked-by: []
supersedes: []
related-systems: [warmth-split.md]
related-balance: []
landed-at: null
landed-on: null
---

**Status:** phase 1 (design) committed; phases 2–4 pending.

**Why it matters:** `needs.warmth` currently conflates physiological
body-heat (hearth/den/sleep/self-groom) with affective closeness
(grooming another cat,
`src/steps/disposition/groom_other.rs:47`). A cat near a hearth is
immune to loneliness at the needs level. The warring-self dynamic
of `docs/systems/ai-substrate-refactor.md` §7.W.2 requires a cat to
be able to be physically warm and socially starving at the same
time — otherwise the losing-axis narrative signal is drowned out by
shelter.

**Design captured at:** `docs/systems/warmth-split.md` (phase 1).
Cross-linked from `ai-substrate-refactor.md` §7.W.4(b).

**Phase 2 — mechanical rename.** Rename `needs.warmth` →
`needs.temperature` and all `*_warmth_*` constants across ~30 call
sites enumerated in the design doc. No behavior change. Verify
with `just check`, `just test`, and byte-identical
`sim_config`/`constants` header on seed 42 vs pre-rename baseline.
Safe; a single commit.

**Phase 3 — `social_warmth` implementation.** Gated on §7.W
Fulfillment component/resource landing. Adds `social_warmth` as a
fulfillment axis; modifies `groom_other.rs:47` to feed both parties'
`social_warmth` instead of the groomer's temperature; adds
isolation-driven decay; adds UI inspect second bar. Small expected
balance impact.

**Phase 4 — balance-thread retune.** New
`docs/balance/warmth-split.md` iteration log. Hypothesis: removing
social-grooming from temperature-inflow reduces well-bonded cats'
temperature refill by ~10–20%; without compensating drain-rate
reduction, cold-stress rises 1.5–3× on seed 42. Full four-artifact
acceptance per CLAUDE.md balance methodology. Starvation and
cold-death canaries must remain 0.

**Dependencies:** phase 2 is independent and can land any time.
Phase 3 is gated on §7.W (Fulfillment component) landing. Phase 4
is gated on phase 3.
