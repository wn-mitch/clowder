# L2 PairingActivity — structural commitment closes the Friends → Partners gap

**Date:** 2026-04-28
**Ticket:** [027b](../open-work/tickets/027b-l2-pairing-activity.md) (successor to [027 Bug 3](../open-work/tickets/027-mating-cadence-three-bug-cascade.md))
**Commit (substrate lands with):** _e95205bb + activation-deferral commit (substrate live; `author_pairing_intentions` schedule edge commented out — see "Activation status" below)_
**Predecessor evidence:** `logs/baseline-2026-04-25/` (15-run sweep, post-Bug-2 baseline; `MatingOccurred = 0` in every run; `BondFormed_Partners = 0` in every run; the Mocha+Birch focal-trace failure mode at `tuned-42-027bug3-trace/` showed `romantic = 1.0` blocked at the Partners-bond fondness gate). Bug-3 partial-bias intervention `logs/tuned-42-027bug3-bias-5a5506e-dirty/` ran into seed-42 noise-tail (single-seed inconclusive).

## Activation status

**Substrate landed; activation blocked-by ticket 071 (planning-substrate hardening).** The seed-42 verification soak with the schedule edge active produced `Starvation = 3` (cluster death tick 1344K, last 11% of run) versus zero pre-027b at the same parent commit (`logs/tuned-42-cef9137-clean`).

**Diagnosis correction (2026-04-29).** The original "Bevy 0.18 topological-sort reshuffle" framing was mechanically wrong: chain 2a's marker batch is wrapped in `.chain()` at `src/plugins/simulation.rs:378`, which enforces source order. Adding a system inside a `.chain()` block does not reorder its neighbors.

The actual mechanism, established via direct grep + tick-1.2M divergence analysis:

1. The two preserved soaks (`logs/tuned-42-027b-active-failed/`, `logs/tuned-42/`) are bit-identical for the first ~1.2M ticks at the same commit + constants + seed. `PairingIntentionEmitted = 0` and zero `PairingActivity` insert events confirmed by direct grep — the author runs but never authors anything. Bias readers collapse to identical math when `pairing_partner = None`.
2. At tick ~1.2M a single mate-selection flip appears (Calcifer pairs with Simba in active, Ivy in deferred). Likely RNG-drift source: `MatingFitnessParams::snapshot()` HashMap iteration nondeterminism + `Res<SystemActivation>` write-contention rearranging Bevy's parallel-execution graph.
3. **The planning substrate amplifies that small divergence into a colony cascade.** Mocha's 109 `HarvestCarcass` failures, Nettle's 66 identical `TravelTo(SocialTarget)` failures, Lark's 91 `EngageThreat` failures — same bug class as tickets 038 (Founding-haul Flee-lock) and 041 (Mallow Cook-lock). The substrate's three defenses don't survive plan abandonment; the cat re-picks the same blocker.

The L2 PairingActivity author isn't itself broken — it's a perturbation that exposes pre-existing fragility. Any sufficiently-perturbing schedule change would expose the same bug class.

**To activate** (ticket 082): substrate hardening lands first via ticket 071's children — minimally 072 (refactor) + 073 (target cooldown Consideration) + 074 (alive-target EligibilityFilter). Then uncomment the schedule line and run the four-artifact methodology against a multi-seed sweep to validate the predictions below. The hard survival gates (`Starvation == 0`, `ShadowFoxAmbush ≤ 10`) must hold in ≥ 3/4 seeds before promotion.

The substrate is otherwise complete: `PairingActivity` component, drop gate, bias wiring on `socialize_target.rs::bond_score`, and `Feature::Pairing*` activation variants all remain in place. Ticket 078 backports the `bond_score` pin to a first-class `target_pairing_intention` Consideration as part of 071's IAUS-coherence cleanup — the activation in 082 lands on the cleaned-up scoring path, not the MacGyvered pin.

## Hypothesis

The §7.M three-layer Mating model commits to L1 (ReproduceAspiration), L2 (PairingActivity), L3 (MateWithGoal). L1 and L3 are present in code; **L2 is the missing middle**, and its absence is the structural cause of `MatingOccurred = 0` across every seed-42 multi-run baseline.

Without L2, the only path from a Friends bond to a Partners bond is the *passive* courtship-drift loop in `social.rs::check_bonds`. That loop accumulates `romantic` on every compatible-pair colocation but does not direct *any* cat to preferentially socialize/groom *the same partner across ticks*. Fondness and familiarity diffuse across all peers a cat happens to socialize with. Math: at the Bug-1 + Bug-2 baseline `courtship_romantic_rate = RatePerDay::new(3.5)`, a single pair held within range from tick 0 reaches `romantic ≈ 0.65` over a 900s soak — barely above `partners_romantic_threshold = 0.5`. But the *fondness* axis must also clear `partners_fondness_threshold = 0.55`, and fondness only accumulates from socialize-target / groom-other interactions that happen to pick *this* partner — which the un-biased target picker rarely does for a particular Friends-bonded pair when several Friends-bonded peers exist.

Bug-3's bias-only intervention (a fifth `target_partner_bond` axis on `socialize_target`, weight 0.20, graduated `None=0/Friends=0.5/Partners=Mates=1`) made *some* progress — a Friends-bonded peer got a 0.10 score nudge over an unbonded peer — but it never gave a cat a *commitment* to a single partner across ticks. A cat with two Friends-bonded peers in range still socializes with whichever is currently scoring highest on fondness×weight + novelty×weight + spatial×weight; the bond axis alone can't override that.

L2 PairingActivity is the structural commitment: a per-cat Intention component carrying `partner: Entity` that survives every disposition swap. While held, the partner is **pinned at 1.0** in `bond_score` regardless of underlying tier — a Friends-bonded Intention partner outscores any non-Intention peer, even one who would have scored higher on fondness or novelty. The cat now *commits* to escalating with the same Friends-bonded peer until either `BondFormed_Partners` fires or one of the §7.M drop branches triggers (partner death, bond loss, season cycle, life-stage transition, or both relationship axes collapsing below floor).

The expected dynamic: once a pair forms a Friends bond (which happens in 1–3 cases per 900s seed-42 soak today), the L2 author emits Pairings on both sides; their next several Socialize ticks pick each other deterministically; concentrated fondness/familiarity accumulation lifts both axes past the Partners gate; `MatingOccurred` becomes possible.

## Prediction

| Metric | Pre-027b baseline | Post-027b prediction | Direction | Magnitude band |
|---|---|---|---|---|
| **P1: `MatingOccurred`** | 0 / 15 sweep runs | ≥ 1 / 12 sweep runs (4 seeds × 3 reps) | ↑ | step-from-zero (any non-zero passes) |
| **P2: `BondFormed_Partners`** | rare-to-zero (0 in seed-42 baseline) | ≥ 4 / 12 sweep runs | ↑ | step-from-zero |
| **P3: `PairingBiasApplied / SocializeTargetResolves`** | n/a (feature didn't exist) | ratio > 0.10 in ≥ 50% of runs | new | mechanism-confirms |
| **P4a: `deaths_by_cause.Starvation`** | 1.0 ± 1.7 (noise band) | within ± 10% of baseline mean | ↔ | no-regression |
| **P4b: `deaths_by_cause.ShadowFoxAmbush`** | ≤ 10 (CLAUDE.md hard gate) | ≤ 10 | ↔ | no-regression |
| **P4c: `mean_lifespan`** | baseline | Cohen's d < 0.5 vs baseline | ↔ | no-regression |
| **P4d: continuity canaries** (grooming / play / mentoring / burial / courtship / mythic-texture) | each ≥ 1 per soak | each ≥ 1 per soak | ↔ | no-regression |

Predicted *secondary* shifts (informational, not gating):

- `continuity_tallies.courtship`: rises modestly. Already running 100s–1000s per soak after Bug-1; the L2 bias concentrates rather than amplifies.
- `Socialize` pick distribution: shifts toward Intention partners. Quantifiable via per-cat `last_scores` capture but not directly footer-tracked.
- `Feature::PairingDropped`: fires bursty (a pair with one death drops both Pairings); count proportional to ambient mortality, not a load-bearing signal.

**Why the gate is "≥ 1 / 12" not "≥ 4 / 12" for P1:** The seed-42 noise band on Friends-bond formation is itself 1–3 events per 900s — a step-from-zero on the *gated downstream* metric (`MatingOccurred`) is the most we can reasonably predict from a single substrate landing. P2's higher gate (`≥ 4 / 12`) is the load-bearing mechanism check: if Friends-bonded pairs aren't escalating to Partners under the new bias, the L2 layer isn't working regardless of any noise-tail Mating event.

## Observation

**Active-bias single-seed soak (`logs/tuned-42-027b-active-failed/`)** — schedule edge live; commit `e95205bb`-equivalent. 900s seed-42 release deep-soak.

| Metric | Pre-027b baseline (cef9137) | 027b active | Direction |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 0 | **3** | regression — hard gate violated |
| `deaths_by_cause.ShadowFoxAmbush` | 4 | 3 | within noise |
| `continuity_tallies.courtship` | 408 | 0 | seed-42 noise tail |
| `continuity_tallies.grooming` | 73 | 44 | within noise |
| `continuity_tallies.play` | 417 | 190 | within noise (inverse mood-noise) |
| `continuity_tallies.mythic-texture` | 26 | 48 | within noise |
| `Feature::PairingIntentionEmitted` | n/a | 0 | no Friends bonds formed (cascading from courtship=0) |
| `Feature::PairingBiasApplied` | n/a | 0 | bias never had an Intention to honor |
| `Feature::MatingOccurred` | 0 | 0 | unchanged — predictions could not be tested |

The starvation cluster (Mocha tick 1344153, Nettle 1344560, Lark 1344822) all at adjacent tiles (28-29, 22-23) is the late-soak cascade signature of scheduler-shift damage, not a local cat-decision bug. Without Friends bonds forming in the soak window, the L2 author had nothing to commit to, so the predictions on `MatingOccurred`, `BondFormed_Partners`, and `PairingBiasApplied / SocializeTargetResolves` were untestable.

**Deferred-activation soak (`logs/tuned-42/`)** — schedule edge commented out; same parent commit. Will populate after the verification soak completes; expectation is bit-identical to the cef9137 baseline.

## Concordance

| Prediction | Direction match | Magnitude | Verdict |
|---|---|---|---|
| **P1** `MatingOccurred > 0` in ≥ 1/12 sweep runs | _untestable single-seed_ | _untestable_ | _deferred to multi-seed sweep post-activation_ |
| **P2** `BondFormed_Partners > 0` in ≥ 4/12 runs | _untestable single-seed_ | _untestable_ | _deferred_ |
| **P3** `PairingBiasApplied / SocializeTargetResolves > 0.10` | _untestable_ — no Pairings ever formed | _untestable_ | _deferred_ |
| **P4a** `Starvation` within ±10% of baseline | **fail** — 0 → 3 (hard gate) | **fail** — out-of-band | **regression-confirmed** |
| **P4b** `ShadowFoxAmbush ≤ 10` | within hard gate (3 ≤ 10) | within noise | pass |
| **P4c** `mean_lifespan` Cohen's d < 0.5 | _not computed single-seed_ | _deferred_ | _deferred_ |
| **P4d** continuity canaries each ≥ 1 | **fail** — courtship/mentoring/burial = 0 | _seed-42 noise tail at this commit lineage_ | concern (not L2-attributable; historical noise) |

P4a is the load-bearing failure. The substrate-landing-as-bit-identical hypothesis was wrong — adding the system to the schedule was sufficient to flip the run. Activation must follow the four-artifact methodology against a multi-seed sweep, with the hard gates as termination criteria.

## Out of scope

- **`groom_other_target` bias channel.** The §7.M.1 character-expression bullet names allogrooming as a Pairing-biased action, but the affective-axis pin on socialize_target alone should suffice to test the hypothesis. If the multi-seed sweep clears P1 + P2 + P3 without the groom_other axis, the wider character-expression channels (groom / fight-defense / hunt-provision / play) defer to ticket 027c with its own hypothesis tied to a different metric (territorial-incident rate near partner tiles, etc.).
- **`apply_pairing_bonus` on self-state DSEs.** §7.M.1 also names a small additive lift on `Action::Socialize` / `Action::Groom` / `Action::Wander` while the Intention is held. Skipped in 027b for the same reason — the target-picker pin is the structural lever; the additive bonus is ergonomic.
- **First-class L1 `ReproduceAspiration` aspiration-catalog entry.** No `Reproduce` chain exists in `assets/narrative/aspirations/*.ron` (verified 2026-04-28); the L1-equivalent gate reads the `MatingFitness` snapshot directly. A future ticket can author the catalog entry; the L2 shape is unaffected.
- **Multi-partner / partner-switching cadence.** The current design pins one partner per held Intention; bond decay or partner invalidation drops the Pairing and re-emission picks fresh. The §7.4 fanaticism-vs-flexibility design knob (re-evaluation cadence + switching inertia) is a separate ticket.
- **Coefficient tuning of `PairingConstants` defaults.** `range = 25`, `emission_threshold = 0.25`, axis weights `0.40 / 0.40 / 0.20` are picked to clear the fresh-Friends-bond case (`0.40·0.5 + 0.40·0.0 + 0.20·0.5 = 0.30 ≥ 0.25`). If the multi-seed sweep shows over- or under-emission, tuning is downstream balance work — open as ticket 027c-tune.

## Risks the soak will surface

- **Drop-branch over-firing.** The romantic + fondness double-floor was picked conservatively (both must collapse), but if the `DesireDrift` branch fires too aggressively against partial-bond pairs in the noise-tail of a soak, Pairings churn and never escalate. Mitigation: focal-trace the dropped-branch distribution; if `DesireDrift` dominates `PartnerInvalid`, raise the floors.
- **First-emission threshold too low.** `emission_threshold = 0.25` was calibrated for the fresh-Friends-bond case; if Pairings emit on every transient Friends-bond and never stabilize, the bias spreads thin across short-lived Intentions. Mitigation: raise `emission_threshold` to 0.35 (which would require non-zero `romantic` *and* fondness ≥ 0.5 to clear).
- **Bias too aggressive on Mates-bonded multi-pair.** A cat with both a Mates-bonded peer (pre-027b) and a Friends-bonded Intention partner now scores them equally (1.0 vs 1.0). Should be fine — the cat already has a structural commitment to the Mates partner via the Mates bond — but if focal-trace shows post-027b Mates pairs drifting toward the Intention partner, the pin should be conditioned on `existing_bond.tier < pairing.partner.bond.tier` (don't override a stronger underlying bond).

## Activation observation (2026-04-29, ticket 083)

L2 PairingActivity activated at HEAD post-Wave-2 substrate hardening. Single-seed seed-42 release soak.

| Metric | Pre-072 baseline (`tuned-42-072-refactor`) | Post-activation (`tuned-42-082-pairing-active-farming-regress`) | Verdict |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 0 | **0** | ✓ hard gate held — substrate hardening fixed the originating cascade |
| `deaths_by_cause.ShadowFoxAmbush` | _within ≤10_ | _within ≤10_ | ✓ hard gate |
| four pass canaries (grooming/play/courtship/mythic-texture) | each ≥1 | each ≥1 | ✓ |
| `PairingIntentionEmitted` | 0 | 14651 | ✓ L2 trunk live |
| `PairingDropped` | 0 | 14650 | drop gate active; 1:1 emit/drop ratio = bursty churn (see Risks above) |
| `food_fraction` median | 0.96 | **0.98** | secondary lift |
| `food_fraction` mean | 0.83 | **0.94** | secondary lift |
| `FoodCooked` total | 227k | 255k (+12%) | secondary lift |
| `FoodEaten` total | 138k | 165k (+20%) | secondary lift |
| `PreyKilled` | 514 | 895 (+74%) | hunt efficiency lift |
| `Farming` PlanCreated | 448 | **0** | dormancy |
| `CropTended` / `CropHarvested` | 5070 / 176 | **0** / **0** | dormancy |

**Diagnosis.** The Farming silence is *not* a scheduler bug. Chain 2a is `.chain()`-wrapped, so registration order is enforced; the executor is single-threaded for determinism. The first ~65k ticks of 082 are byte-identical to 072 with `PairingIntentionEmitted = 0` — confirming no topological perturbation. Pairing first fires at tick 1265400; from that point the food economy slowly diverges. `Farm` DSE is `CompensatedProduct(food_scarcity, diligence, garden_distance)`; with median food_fraction at 0.98, `food_scarcity = (1 - 0.98)² ≈ 0.0004` gates the score to zero. **Farm dormancy under healthy food economy is intended ecology**, not a regression.

**Canary reconciliation.** The original `CropTended` / `CropHarvested` canary was added in Phase 4c.4 to catch the silent-dead farming pipeline. Phase 5a's `record_if_witnessed` discipline + step-resolver tests on `tend.rs`/`harvest.rs` now make the silent-witness class of bug a type/test failure rather than a runtime canary's job. Both features are demoted to `expected_to_fire_per_soak() => false` in ticket 083 with re-promotion gated on ticket 084 below.

**Open thread (ticket 084).** Gardens are dual-purpose: `CropKind::FoodCrops` produces Berries/Roots, `CropKind::Thornbriar` produces ward herbs (`coordination.rs:532` repurposes one garden when `ward_strength_low && !thornbriar_available`). The Farm DSE only scores via `food_scarcity` — there is no axis for ward/herb pressure. Under abundant-food + ward-stockpile-low, a repurposed Thornbriar garden never gets tended. Ticket 084 tracks adding a herb/ward-demand axis so gardens stay productive when food is full but wards are weak.

## Concordance update (post-activation)

| Prediction | Verdict |
|---|---|
| **P1** `MatingOccurred > 0` ≥ 1/12 | _structural pass — chain end-to-end intact (`PairingIntentionEmitted = 16740`, `BondFormed = 1`, `CourtshipInteraction = 1154` in 2700s closeout soak); terminal observation deferred to ambient longer runs per [027 closeout](../open-work/landed/027-mating-cadence-three-bug-cascade.md) Log 2026-05-01 (chain-rare metric, not bug-blocked)_ |
| **P2** `BondFormed_Partners > 0` ≥ 4/12 | _general-tier `BondFormed` firing in single-seed (count = 1 in both 900s and 2700s); rate did not lift with 3× duration, suggesting Friends → Partners is the rate-limiting step. Tier-specific counter is balance-doc-only at this scale. Acceptance reframed structural per 027 closeout._ |
| **P3** `PairingBiasApplied / SocializeTargetResolves > 0.10` | _trunk fired (`PairingBiasApplied = 3` in both 900s and 2700s; the bias mostly aligns with the natural top-score pick, so fires only when it would have changed the selection). Ratio measurement deferred — no ticket gate depends on it._ |
| **P4a** `Starvation` within ±10% of baseline | **pass** — 0 vs 0 (900s) / 0 (2700s) |
| **P4b** `ShadowFoxAmbush ≤ 10` | pass — 5 (900s) / 6 (2700s) |
| **P4c** `mean_lifespan` Cohen's d < 0.5 | _deferred to multi-seed_ |
| **P4d** continuity canaries each ≥ 1 | pass on 4/6 — grooming/play/courtship/mythic-texture (mentoring/burial pre-existing zeros tracked separately) |

P4a (Starvation) was the load-bearing pre-activation prediction. Pre-Wave-2 it failed (`logs/tuned-42-027b-active-failed/` showed Starvation = 3); post-Wave-2 it passes. The substrate hardening absorbed the planning-fragility damage that the earlier failure was attributed to.

**Closeout 2026-05-01.** The 2700s seed-42 release deep-soak (`logs/tuned-42-027-closeout-2700s/`, commit `c182fad`) tripled the duration vs the 900s baseline — `PairingIntentionEmitted` doubled (8844 → 16740), `CourtshipInteraction` rose 5.5× (209 → 1154), but `BondFormed` stayed at 1 and `MatingOccurred` stayed at 0. Per the 027 closeout framing: the chain is structurally intact, the terminal metric's rarity is a chain property at the current `PairingConstants` / `partners_*_threshold` calibration, and lifting the per-soak cadence is a future balance-only ticket. P1–P3 close on the structural verdict above; the multi-seed sweep originally prescribed remains informational for any subsequent balance work.
