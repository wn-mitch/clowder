---
id: 032
title: Starvation rebalance — align with IRL cat biology, interesting not cutthroat
status: ready
cluster: null
added: 2026-04-26
parked: null
blocked-by: []
supersedes: []
related-systems: [needs.md]
related-balance: [healthy-colony.md]
landed-at: null
landed-on: null
---

## Why

Two converging signals say the starvation pipeline needs a systematic look:

1. **Survival canary brittleness.** `deaths_by_cause.Starvation` is the project's hardest gate (target: 0). Recent seed-42 soaks have ranged 0–9 across re-runs of the same commit (Bevy parallel-scheduler variance). The gate fires often enough to mask a real regression underneath the noise — and survival-tier dominance is precisely what the §7.W fulfillment refactor and `social-target-range.report.md` flagged as the *cause* of the colony's narrow behavioral range.
2. **Reproduction collapse via hunger-floor gating.** `social-target-range.report.md` iter-2 documented that `Mate` is gate-starved on the scoring layer because `breeding_hunger_floor=0.6` is rarely satisfied — cats spend too much time hungry to compose the AND-gate of (hunger>0.6 ∧ energy>0.5 ∧ mood>0.2 ∧ Partners-bond ∧ ...). The cause is upstream: the colony lives in survival mode, never accumulates the welfare slack that makes higher-tier behaviors viable.

Result: starvation isn't a *narrative pressure* — it's an *attractor* the colony settles into. Per `docs/systems/project-vision.md` ("Honest world, no director"), real-world cat biology should drive realistic mortality without forcing every colony into the same brittle survival lock.

## Real cat biology — what we should be modeling

Quick reference (from veterinary literature, summarize before tuning):

- **Adult cats** survive ~1–2 weeks without food (pure starvation) but lose body condition fast — 5–10 days into a fast, fatty liver disease (hepatic lipidosis) becomes a serious risk and recovery requires intervention. **Kittens** survive far less — days, not weeks — and are far less robust to acute hunger.
- Cats are **obligate carnivores**: brief fasts followed by feeding cycles are normal, prolonged caloric deficit is not. They don't graze; they eat-rest cycles.
- **Hunting success** in the wild is ~30–50% per attempt for a healthy adult. Cubs/kittens fail far more.
- **Field cat mortality** is dominated by predation, disease, and accidents — *not* starvation. Starvation is a contributory factor (a cat in poor body condition is more vulnerable to all three) more than a primary cause.
- **Body condition** as a state matters more than discrete hunger. A cat at hunger=0.4 fed ad lib for two days returns to baseline; one at hunger=0.0 for two days is health-compromised even after feeding resumes.

Implication for the sim: starvation as a primary `deaths_by_cause` should be **rare and load-bearing** (a cat that starves is a story, not a statistic), and the *intermediate* states (mild caloric deficit, body-condition decline) should drive most of the welfare-tier knock-on effects — *not* the all-or-nothing `hunger == 0` cliff currently in `src/systems/needs.rs:92`.

## Scope

Each numbered item is a discrete tuning hypothesis. Land them as separate `just hypothesize` runs against a stable baseline.

1. **Soften the starvation cliff.** Today: `if needs.hunger == 0.0` triggers the full health-drain + safety-drain + persistent-mood-penalty cascade. Real biology: fasting cats lose body condition gradually, not at a single threshold. Replace the cliff with a graded `body_condition` axis (or scale `starvation_*_drain` by `(1 - hunger)^2` instead of `hunger == 0`). Predict: `deaths_by_cause.Starvation` *down* by 60–90%; `welfare_axes.acceptance.mean` *up* (cats not in panic mode); mating cadence *up* (hunger floor gate easier to satisfy intermittently).

2. **Stage-stratify mortality.** Kittens should be far more vulnerable to starvation than adults; elders also more vulnerable than prime adults. Today the constants are flat. Add per-life-stage multipliers to `starvation_health_drain` (kitten: 2.0×, juvenile: 1.3×, adult: 1.0×, elder: 1.5×). Predict: kitten mortality *up* during food-pressure events but adult survival *up* overall; the colony tells more stage-driven stories.

3. **Decouple survival from reproduction floor.** Today: `breeding_hunger_floor=0.6` × hunger-decay × starvation cascade collapses the mating window to near-zero on every soak. Lower the floor to 0.4 and observe whether mating cadence rises into the band predicted by `social-target-range`. Predict: `continuity_tallies.courtship` *up* by 50–200%; `kittens_born_total` *up*. Watch for unintended: cats trying to mate while too hungry for the encounter to succeed (gate is real, not arbitrary).

4. **Hunting success rate audit.** If real cats hit ~30–50% per attempt and the sim's `EngagePrey: lost prey during approach` averages 3675 ± 4990 plan failures per 15-min soak (per `healthy-colony.md`), the apparent failure rate is far higher than that — but only because each plan-step is a sub-attempt, not a discrete hunt. Validate: convert the failure tallies into per-discrete-attempt success rate via the events log, compare to the 30–50% target, decide whether prey-targeting needs any change.

5. **Body-condition welfare-axis.** Add a slow-moving `body_condition` welfare axis (akin to `Fulfillment.social_warmth`) that decays under hunger and recovers under feeding, and use it (not raw hunger) as the input to gates that should care about long-term health (mating, mentoring, ward-placement endurance). Predict: gates fire more reliably *across* hunger oscillations; less brittleness from per-tick hunger noise.

## Out of scope

- Any change to the food-economy production side (prey density, kill-yield, cooking).
- Any change to magic / corruption / shadowfox.
- New `deaths_by_cause` causes.
- Per-cat trait modifiers (e.g. "this cat has a slow metabolism").

## Approach

Use the new tooling end-to-end as the canonical workflow:

1. Promote a clean post-substrate-refactor seed-42 soak as the active baseline: `just promote logs/tuned-42 baseline-pre-starvation-rebalance`.
2. For each scope item, draft a hypothesis YAML (template at `docs/balance/hypothesis-template.yaml`), run `just hypothesize <yaml>`, inspect the drafted balance doc.
3. Verify each treatment with `just verdict logs/sweep-<treatment>/42-1` and `just fingerprint logs/sweep-<treatment>/42-1`.
4. Append iterations to a single `docs/balance/starvation-rebalance.md` thread (not separate files).
5. Land each accepted change as a one-line `sim_constants.rs` patch (or, for items 1 + 5, a small Rust change in `src/systems/needs.rs`).

The acceptance bar per item is: predicted-direction match + magnitude in band + no survival canary regression + at least one continuity canary either holding or improving.

## Verification

- `just verdict` and `just fingerprint` both `pass` on the post-rebalance seed-42 soak.
- `deaths_by_cause.Starvation` drops below the noise band (mean ≤ 0.5).
- `continuity_tallies.courtship` ≥ 5 (rising from current ~0).
- No survival canary regresses (Starvation == 0 hard, ShadowFoxAmbush ≤ 10).
- Welfare-axis means (acceptance, respect, mastery, purpose) all *rise* — the test that the colony has slack to express higher-tier behaviors.

## Log

- 2026-04-26: Ticket opened. Triggered by tooling pass surfacing two intersecting bugs: (a) `social.bond_proximity_social_rate` mis-spelled but `nearest` suggested the right path; (b) `fingerprint`'s silent-subsystem check now flags the ward-pipeline collapse on iter2 logs. Both in turn made the starvation-as-attractor pattern visible across the colony's whole continuity register.
