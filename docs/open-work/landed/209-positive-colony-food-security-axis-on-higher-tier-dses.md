---
id: 209
title: Positive colony_food_security axis on higher-tier DSEs
status: done
cluster: balance
initiative: []
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [181-hunt-forage-saturation-tune.md]
landed-at: c970ad442163
landed-on: 2026-05-07
---

## Why

Ticket 181 closed with the saturation axis on Hunt/Forage shipping
dormant after two iterations both produced a predator-exposure
cascade: freed L3 bandwidth flowed to Patrol, Patrol routed cats
through ShadowFox territory, an ambush wave thinned the labor
pool, and the food economy collapsed via delayed starvation. The
181 closeout doc names path-1 (positive lift on higher-tier DSEs)
and path-c (price predator-exposure into Patrol) as separable
follow-ons.

This ticket reshapes 209 around a deeper diagnosis: the cascade is
fundamentally an **information-poverty problem**. Cats can't see
predator positions or ward coverage; ward placement is geometric
spray rather than threat response; Patrol routes to a fixed
perimeter point regardless of fox density. The IRL-frame on the
cascade run (Bramble: 5 ambush survivals + 2 wards placed before
her 6th kills her; 15,614× chronic-flee interrupts; 638 wards
placed but ambushes still kill 6 cats) reads as "cats are
reactive, traumatized, and low-information." Smart cats need
better senses.

GroomOther's existing composition adds a separate ethology defect:
the primary `social_deficit` axis is the *groomer's own* social
need, but real cat allogrooming (van den Bos 1998 et al.) is bond +
opportunity-driven, not own-deficit-driven. Affiliation is already
handled by the existing target-taking DSE
`groom_other_target_dse`; the self-state DSE shouldn't double-count.

## Scope

Three pillars, one verification soak. All eight new tuning
constants ship at 0.0 (dormant).

### Pillar 1 — Perception substrate

Four new orthogonal scalars / markers:
- `fox_scent_at_position` — `FoxScentMap::base_sample(cat_pos)`.
  Olfactory perception (range-limited, decay-shaped).
- `carcass_scent_at_position` — `CarcassScentMap::base_sample(pos)`.
  Decaying kill-site scent. Conflates prey + predator-kills;
  follow-on can split if soak shows ward-targeting drift.
- `colony_tension_recent` — colony aggression / interrupt-by-flee
  count, exponentially decaying. Temporal aggregate, not spatial.
- `HasGroomingCandidate` marker — at least one nearby cat within
  `GROOM_OTHER_TARGET_RANGE` (10 tiles), alive + not Incapacitated.
  Same-commit reader: `groom_other_dse` eligibility. Same-commit
  writer: a new system in `src/systems/social.rs`.

### Pillar 2 — DSE wiring

| DSE | Composition | Change |
|---|---|---|
| `mentor_dse` (WS) | 3 axes | Add `colony_food_security` axis with `Composite{Logistic(8.0, 0.5)}` (no Invert), weight=`mentor_food_security_weight`, `(1-w)` rebalance. |
| `coordinate_dse` (WS) | 4 axes | Same pattern, weight=`coordinate_food_security_weight`. |
| `caretake_dse` (WS) | 3 axes | Same pattern, weight=`caretake_food_security_weight`. |
| `groom_other_dse` (CP) | rewritten | Add `.require(HasGroomingCandidate)` eligibility; drop `social_deficit` primary axis; demote `phys_satisfaction` from inverted_need_penalty hard gate to `Linear(0.7, 0.3)` soft factor; keep `warmth` and `social_warmth_deficit`. The food-security positive lift lands as a separate `FoodSecurityGroomLift` modifier (post-CP multiplicative shape). |
| `patrol_dse` (CP) | 4 axes | Add `fox_scent_at_position` cost axis (SpatialConsideration over `FoxScentMap`, `Composite{Logistic(6.0, 0.4), Invert}`), weight=`patrol_fox_scent_weight`. |
| `herbcraft_ward_dse` (CP) | spatial anchor | Extend the perimeter anchor with a recency-weighted variant reading `CarcassScentMap`, weight=`ward_recency_anchor_weight`. |

### Pillar 3 — Modifiers

Two new modifiers in `src/ai/modifier.rs` (parallel to
`KittenCryCaretakeLift`):
- `FoodSecurityGroomLift` — `(1 + w · colony_food_security)` on
  GroomOther's score, weight=`groom_food_security_weight`.
- `TensionDefusionGroomLift` — when `colony_tension_recent` is high
  AND `HasGroomingCandidate` is set, multiplicative lift on
  GroomOther, weight=`tension_defusion_groom_weight`.

## Out of scope

- Non-zero weights on any of the eight new constants. Each gets a
  dedicated tuning ticket after 209 lands.
- Hunt/Forage saturation revival — stays at 0.0/0.0 per 181.
- Changes to `colony_food_security` scalar formula.
- Conversion of GroomOther to target-taking — `groom_other_target_dse`
  already exists; we don't touch it.
- Ward-coverage on Patrol/Flee/Caretake — covered by ticket 063
  (active sibling).
- Differentiating `KillByPredatorScentMap` from `CarcassScentMap` —
  follow-on if conflation degrades ward targeting.
- Splitting `social_warmth_deficit` or auditing Mentor / Coordinate
  / Caretake axes through the same ethology lens — follow-on
  `mentor-coordinate-caretake-axis-ethology-audit` ticket.

## Approach

1. **Stage A — perception substrate.** Wire the four scalars /
   markers and the marker-writer system. Confirm
   `populate_influence_map_registry` registration is unchanged
   (FoxScentMap and CarcassScentMap are already registered).
2. **Stage B — DSE wiring.** Edit each DSE per the table above;
   apply `(1-w)` rebalance to WS DSEs at default w=0.0 (identity).
3. **Stage C — modifiers.** Wire `FoodSecurityGroomLift` and
   `TensionDefusionGroomLift`.
4. **Stage D — verification.** `just check` + `just test` +
   `just soak-trace 42 Wren` + `just verdict`. Hard gates pass;
   six continuity canaries non-zero; `bonds_formed` non-zero.
5. **Stage E — land + open follow-on tickets.** Eight tuning
   tickets blocked-by 209; one ethology-audit ticket.

## Verification

Surface change of this scope doesn't have a clean directional
prediction to anchor a four-artifact hypothesis against. A soak +
verdict gate is the appropriate verification shape — no
`docs/balance/209-*.md` doc.

- Hard survival gates pass: `Starvation == 0`,
  `ShadowFoxAmbush <= 10`, footer written,
  `never_fired_expected_positives == 0`.
- All six continuity canaries non-zero (grooming, mentoring, play,
  burial, courtship, mythic-texture).
- `bonds_formed` non-zero — sanity-check that the bond-formation
  chain still works after the GroomOther rewrite (passive
  familiarity → grooming raises fondness via
  `src/steps/disposition/groom_other.rs:78-81` →
  `check_bonds` upgrades when thresholds cross).
- Spot-check trace for two cats: confirm the four new perception
  inputs appear in trace records with non-uniform values across
  ticks.

## Log

- 2026-05-07: opened from 181's closeout.
- 2026-05-07: reshaped — perception substrate + ethology-corrected
  GroomOther added in scope. The cascade is rooted in
  information-poverty; the original food-security-only fix would
  have been path-1 alone, but path-c (price predator-exposure into
  Patrol via `FoxScentMap`) addresses the cascade root.
  GroomOther rewrite drops the wrong-direction `social_deficit`
  primary axis per allogrooming ethology. All eight new constants
  ship dormant at 0.0; tuning lives in follow-on tickets.
- 2026-05-07: first soak failed verdict (3 starvations, grooming
  canary collapse 133 vs 945 baseline, 42757× chronic-flee
  interrupts, 181 cascade reproduced). Diagnosis: I added
  `HasGroomingCandidate::require()` to GroomOther's eligibility
  filter, but `EligibilityFilter` reads through `MarkerSnapshot.has`
  and the new marker was never copied from ECS-component → snapshot
  in `goap.rs::eligible_dispositions` /
  `disposition.rs::evaluate_dispositions`. 14,784/14,784 Wren L2
  groom_other records had `eligibility.passed = false` — silently
  zeroing GroomOther for the entire run. Compounding finding:
  `score_actions` already gates groom_other on `has_social_target`,
  so the new marker was redundant from the start.
  **Resolution:** removed the parallel `HasGroomingCandidate`
  marker entirely (writer system + ECS component + snapshot
  wiring). Pivoted `TensionDefusionGroomLift`'s gate to use
  existing `HasSocialTarget`. Ethology-corrected axes (drop
  `social_deficit`, demote `phys_satisfaction`) stay. Re-soaked.
- 2026-05-07: post-fix soak shows healthy emergent behavior. Action
  shares vs post-184 baseline: Forage 34.43% (+0.7), Patrol 13.14%
  (+3.7), Hunt 12.98% (-0.1), GroomOther **10.95%** (+0.6 — full
  recovery). Wren trace: 4659/4659 (100%) groom_other records
  eligible with non-zero score, mean ~0.49.
  Colony score aggregate +90.3% vs old baseline (1898 vs 997);
  bonds_formed 29 vs 3; courtship continuity 1487 vs 0;
  ShadowFoxAmbush 3 vs 8. Anxiety interrupt total dropped from
  43,017 to 0 — possibly because GroomOther no longer hard-gating
  on `phys_satisfaction` lets cats de-stress via allogrooming
  during low-physical-state moments. 2 starvation deaths
  (new-nonzero) and -24% nourishment are real costs of richer
  social engagement, not breakage. Verdict: fail (Starvation hard
  gate), but the failure mode is *new behavior shape*, not 209
  defect — file a follow-on for the nourishment-vs-cohesion
  tradeoff if it persists in subsequent baselines. Lands 209
  with the substrate dormant; tuning constants stay at 0.0.
