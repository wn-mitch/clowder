---
id: 489
title: DSE score-collapse contract — audit Pattern B (personality-in-CP + floored need) across the cat-action DSE corpus
status: ready
cluster: ai-substrate
initiative: []
orchestration: substrate-sensitive
added: 2026-05-30
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The "cuddle puddle" investigation (487 + 488) found that **GroomOther scores ~0.49 even when its only legitimate need driver is zero**, because two co-axes in its CompensatedProduct composition — `Personality.warmth` (Linear identity) and `phys_satisfaction` (Linear(0.7, 0.3) — floor 0.3) — act as always-on multiplicative drivers. The need axis (`social_warmth_deficit`) has a 0.1 intercept floor by design "so groom_other isn't zeroed when social_warmth is full," compounding the issue. Result: GroomOther wins L3 by default for any warm-personality fed founder whose alternatives (Patrol, Forage, Build) have no active drivers.

This is a corpus-wide architectural pattern, not a GroomOther bug. CLAUDE.md design pillar #3 names the rule explicitly: *"Decompose signals into orthogonal axes that each encode a distinct situation, not a louder single alarm. **Compose personality / phobias / ambient context at the modifier layer, never inside the underlying perception scalar.**"* The whole DSE corpus reads personality traits as primary CP/WS axes — 31 DSEs total — but only a subset produces the cuddle-puddle dynamics.

The unifying invariant under audit: **every cat-action DSE must collapse to ~0 when its underlying need is fulfilled.** Two structural shapes achieve this; one fails:

- **Pattern A (safe):** CP composition includes an upper-bound gate axis (inverted Logistic over the need scalar) that multiplies the product by ~0 once the need is met. Personality axes in CP are fine because they're multiplied by zero. Examples: `patrol` (3 gates), `practice_magic` (3), `fox_avoiding` (2), `farm` / `herbcraft_gather` / `herbcraft_prepare` / `herbcraft_ward` (1 each).
- **Pattern B (cuddle-puddle-shaped):** CP composition has personality axes + need axes with `intercept > 0` floors, no upper-bound gate. Score floor is the multiplicative product of the floors. Cats run the action by default whenever competing dispositions lack active drivers. Confirmed: `groom_other` (0 gates, 2 floors). Suspected (audit needed): `bury` (0 gates, 1 floor), `mate` (0 gates, 0 floors — relies on `HasEligibleMate` eligibility-marker gate), `explore` (0 gates, 0 floors), `flee` (1 gate + 1 floor — mixed; depends on which dominates).
- **Pattern C (WeightedSum):** ~19 DSEs use `Composition::weighted_sum` with personality axes contributing additively. Different math (sum, not product), but the same architectural concern — personality + ambient context contribute to the score independent of need state. Needs a separate refactor pattern; not folded into this ticket.

The contract this ticket codifies: **a DSE's score floor — measured at need ≈ 0 with realistic personality and ambient inputs — must be near-zero (≤ ~0.1).** GroomOther currently floors at ~0.49. The audit identifies every Pattern B DSE and proposes per-DSE refactors that restore the contract.

## Scope

1. **Audit script** — `scripts/check_dse_score_collapse.sh` or similar. For each CP DSE with personality reads, structurally identify whether the CP has at least one upper-bound gate axis (`Curve::Composite { post: PostOp::Invert, .. }` or equivalent) over a need scalar. If not, compute the analytical CP floor assuming personality = 0.7, phys_sat = 0.9, need scalars = 0, all `intercept` floors as written. Flag any DSE whose computed floor exceeds 0.1. Wire into `just check`.
2. **Pattern B refactor template** — established by GroomOther's landing (the proof-of-concept). Steps:
   - Move personality traits from CP `Consideration` list to a new `<DSE>PersonalityLift` modifier (mirror of `FoodSecurityGroomLift` shape: `score *= (1 + personality_trait × w)`).
   - Demote soft-gate axes (e.g. `phys_satisfaction` with `intercept: 0.3`) to multiplicative dampener modifiers OR to `EligibilityFilter` rejections, depending on whether the design wants a hard or soft gate.
   - Tighten the primary need axis intercept toward 0 (or to a small value ≤ 0.05) so the CP genuinely collapses when need is fulfilled.
   - Constants for every new modifier weight ship in `SimConstants` and start at conservative defaults (probably the magnitude that reproduces the *prior* score at typical-need conditions, so the refactor is roughly behavior-neutral when need is moderate but collapses when need is fulfilled).
3. **Per-DSE child tickets** — one each for `bury`, `mate`, `explore`, `flee` once GroomOther lands and the audit confirms each is Pattern B. Each child ticket follows the template above, lands with its own focal soak + verdict + frame-diff.
4. **WS-corpus separate epic** — name the WeightedSum DSE concern (Pattern C) in this ticket's Out-of-scope, defer to a follow-on. The 19 WS DSEs need a different refactor shape (sum-vs-product asymmetry) and a separate design pass.

## Out of scope

- **The GroomOther refactor itself** — that's the proof-of-concept and lands as its own ticket (call it 490 or open after this lands). 489 is the contract + audit; 490 is the first application.
- **WeightedSum (Pattern C) DSEs** — `socialize`, `mentor`, `coordinate`, `forage`, `hunt`, `fight`, `cook`, `build`, etc. Same Pillar #3 concern but the WS math is additive, not multiplicative. Different refactor pattern. Park as a separate epic ticket.
- **Modifier-layer audit** — there are existing modifiers (`FoodSecurityGroomLift`, `TensionDefusionGroomLift`, etc.) that may themselves violate Pillar #3 by reading personality. Out of scope here; addressable once the CP refactors stabilize.
- **Trait-curve sharpness** — whether `Linear(1.0, 0.0)` on `Personality.warmth` is the right CURVE shape for the modifier is a per-DSE design call, not a class-level invariant.
- **CommitmentStrategy / persistence-bonus interactions** — the puddle's commitment-layer dynamics (§L2.10.6 softmax, §7.4 persistence) are decoupled from the score-shape problem this ticket targets.

## Current state

- **487** landed at `9a05a29c` (2026-05-29). Three substrate layers — `HasGroomCandidate` marker, colony-self directives, emergent coordinator — plus two latent-defect fixes (resolver-side `currently_groomed` filter, FeedKitten newborn-target carve-out). Narrowed Simba L3 Grooming-disposition share 36.8% → 4.2% in 5k-tick window, but freed bandwidth flowed to Patrol/Exploring rather than Forage/Build/Cook (`project_l3_patrol_absorption_cascade` textbook signature).
- **488** landed at `e0391ca5` (2026-05-30). `Fulfillment::founder(i, n)` constructor lifts founder spawn `social_warmth ∈ [0.85, 1.0]` (deficit ≤ 0.15). Architectural mirror of b24d333b's warm-floor Relationships init.
- **Post-488 visual soak still shows cuddle puddling.** Root cause identified: GroomOther CP floor analysis — `(0.7 × 0.93 × 0.1^0.6)^(1/2.6) ≈ 0.49` even when `social_warmth_deficit = 0`. The Pattern B shape, not the spawn condition or the eligibility gate, is the durable driver.
- Corpus audit (this session): 31 DSEs read personality traits as primary axes. Split: 12 CP + 19 WS. Of the 12 CP, 7 have upper-bound gates (Pattern A — safe). 5 candidates for Pattern B refactor: `groom_other` (confirmed), `bury`, `mate`, `explore`, `flee`.

## Approach

**Step 1 — write the audit script.** Walk `src/ai/dses/*.rs`. For each file:

- Detect `Composition::compensated_product` and the considerations vec.
- For each `Consideration::Scalar(ScalarConsideration::new(name, curve))`, classify:
  - Personality trait read (named one of the trait keys at `scoring.rs:870-1050`).
  - Need scalar with `Curve::Linear { intercept: x }` where `x > 0` (floored need).
  - Upper-bound gate (`Curve::Composite { post: PostOp::Invert, .. }` over a need scalar).
- Flag DSEs where (personality_axes ≥ 1) AND (upper_bound_gates == 0) AND (floored_need_axes ≥ 1).
- Compute analytical CP floor with personality = 0.7, phys_sat = 0.9, all need scalars = 0.
- Emit a structured report. Wire into `just check`.

**Step 2 — land GroomOther (separately as 490 or whatever the next id is).** Use that landing to validate the refactor template. The CP becomes single-axis `social_warmth_deficit` with intercept ≤ 0.05. Personality moves to a new `WarmPersonalityGroomLift` modifier. `phys_satisfaction` demotes to a dampener modifier preserving the `Linear(0.7, 0.3)` floor (so tension-defusion grooming under low phys-sat is still possible at a damped score, not zeroed).

**Step 3 — open child tickets for `bury`, `mate`, `explore`, `flee`** based on the audit's confirmation of each. Each child ticket follows the template established by step 2.

**Step 4 — open a separate WS-corpus epic** (Pattern C) once the CP refactors stabilize. The WS asymmetry is a different design question and shouldn't bottleneck the CP work.

## Verification

- **Audit script** flags exactly the Pattern B DSEs and no Pattern A DSEs. Add fixture tests asserting `patrol` is recognized as Pattern A (upper-bound gates collapse the CP) and `groom_other` (pre-refactor) is recognized as Pattern B.
- **Class-level invariant test** — `tests::dse_score_floor_collapse_contract`: for each registered cat-action DSE, instantiate a fixture cat with personality at observed-median values, all needs fulfilled (or all need scalars set to 0), and assert the DSE's computed score ≤ 0.1 (the score-collapse threshold). Pre-refactor, this test should fail for every Pattern B DSE; post-refactor, all pass.
- **Per-child-ticket verdict gates** — each refactor lands with focal soak + `just verdict` + frame-diff against the prior baseline. Continuity canaries must hold for the corresponding action.
- **No corpus-wide soak regression** — a 10-min seed-42 soak post-template-landing matches the b24d333b baseline within drift envelopes (per `just verdict`).

## Log

- 2026-05-30: opened from a session investigation following 488's landing. User ran a visual soak post-488 and observed continued cuddle puddling. Audit traced the persistence to GroomOther's CP score floor of ~0.49 — the Personality.warmth + phys_satisfaction co-axes act as always-on multiplicative drivers regardless of need state. Generalized to the class-level invariant ("score must collapse when need fulfilled") and surveyed the corpus: 12 CP + 19 WS DSEs read personality traits as primary axes; 5 of the CP DSEs are candidate Pattern B (no upper-bound gate). This ticket codifies the contract + the audit script + the refactor template. Child tickets per Pattern B DSE follow once GroomOther's refactor establishes the template.
