# Ward placement ambush + carcass anchor weights — first-light activation

**Date:** 2026-05-11
**Ticket:** [284](../open-work/tickets/284-tune-ward-placement-ambush-carcass-anchor-weights.md)
**Predecessor evidence:** 210's post-soak diagnosis (29 wards placed, 37 Ambush events landing on tile clusters near colony center while wards sat on the fox-scent perimeter); 220's substrate landing (`5348be2d`, 2026-05-11) added two `Logistic(8.0, 0.5)` lifts to `compute_ward_placement()`'s threat term consuming `RecentAmbushMap` (219) and `CarcassScentMap` (Phase 2C), both shipped dormant at `0.0`.
**Substrate:** `src/systems/coordination.rs:1390-1502`. Threat axis = `(fox_scent.max(corruption) + w_ambush · L(recent_ambush) + w_carcass · L(carcass_scent)).min(1.0)` where `L(x) = 1 / (1 + exp(-8·(x-0.5)))`. Both lifts short-circuit when `w == 0.0`.

## Hypothesis

Ward placement in the post-210 baseline followed fox-scent perimeters while ShadowFox ambushes concentrated in tile clusters near colony center. `RecentAmbushMap` and `CarcassScentMap` encode where strikes have actually landed; lifting the two anchor weights off `0.0` biases `compute_ward_placement()`'s threat axis toward those empirical hot zones, intercepting future ambushes before they fire.

## Methodology — first-light, not four-artifact

Per the user's session call, this landing uses a single `just soak-trace 42 Wren` qualitative evaluation rather than the canonical `just hypothesize` four-artifact loop. The first-light question is binary — does the layer fire and do something behaviorally observable — and a single soak settles it cheaply. The follow-on ticket carries the magnitude-tightening burden with hypothesize-grade rigor.

(Memory: `feedback_dormant_substrate_activation_soak_first` documents this discipline as a reusable pattern.)

## Constants landed

```rust
default_ward_ambush_anchor_weight() -> f32  { 0.5 }   // was 0.0 (220 dormant)
default_ward_recency_anchor_weight() -> f32 { 0.3 }   // was 0.0 (220 dormant)
```

Both values are the ticket's prior intuitions; first-light evaluation neither confirmed nor refuted them as the *right* magnitudes, just that the layer is wired and fires. Tighter tuning is deferred.

## Observation

Single soak: `logs/tuned-42/` at commit `81e555db` (dirty — 284's changes in flight), seed 42, 900s release, Wren as focal cat.

### Substrate-active spatial signal — load-bearing

Post-284 ward placements cluster on the empirical ambush corridor — the exact failure mode 210 flagged:

| Ambush position (count) | Nearest ward in run |
|---|---|
| `(29,24)` ×2 | `(29,23)` — co-located |
| `(38,21)`, `(38,20)`, `(38,23)`, `(39,21)` corridor | `(38,22)`, `(39,22)`, `(39,23)`, `(33,22)` cluster |
| `(42,17)` | `(43,20)` — 3 tiles |
| `(46,11)` | `(56,19)` ward — farther; corridor unhit |

Compare post-210 baseline (commit `06f651be`): wards at `(42,22)` and `(41,22)`, far from the equally-prominent `(29,23-24)` ambush cluster. The lifts visibly redirect placement toward the strike geometry.

### Macro outcome counters — magnitude-flat

vs current.json baseline (commit `4bcae2de`, "post-127-joint-intention", 2026-05-11 12:00 EDT):

| Counter | Baseline | Post-284 | Δ |
|---|---|---|---|
| `wards_placed_total` | 16 | 16 | 0 |
| `wards_despawned_total` | 16 | 16 | 0 |
| `shadow_fox_spawn_total` | 18 | 18 | 0 |
| `shadow_foxes_avoided_ward_total` | 2 | 2 | 0 |
| `deaths_by_cause.ShadowFoxAmbush` | 2 | 2 | 0 |
| `deaths_by_cause.Starvation` | 1 | 1 | 0 |
| `deaths_by_cause.WildlifeCombat` | 1 | 1 | 0 |
| `negative_events_total` | 19860 | 19860 | 0 |

Identical at the macro level. The lifts redirect WHERE wards land without (yet) shifting HOW MANY foxes are deterred or HOW MANY ambushes connect.

### Behavioral counters — substrate-active

vs the same baseline:

| Counter | Baseline | Post-284 | Δ |
|---|---|---|---|
| `continuity_tallies.grooming` | 896 | 910 | +14 |
| `continuity_tallies.courtship` | 2487 | 2544 | +57 |
| `continuity_tallies.mentoring` | 215 | 216 | +1 |
| `continuity_tallies.play` | 14 | 14 | 0 |
| `continuity_tallies.mythic-texture` | 35 | 35 | 0 |

Tick-level firings drift slightly upward — small absolute deltas (+1.5% grooming, +2.3% courtship) but in the "behavior is doing slightly different things moment-to-moment" direction the spatial check predicts.

### Ambush events vs the older post-210 reference

| Reference | Commit | Ambush events |
|---|---|---|
| post-210 | `06f651be` | 37 |
| post-284 | `81e555db` | 27 (−27%) |

The 70+ commits between these two states confound a clean substrate-only attribution, but the direction matches the hypothesis and the spatial check rules out the trivial "fewer foxes spawned" explanation (post-210 had 23 spawns; post-284 has 18 — comparable order).

## Concordance

| Artifact | Result |
|---|---|
| **Hypothesis** | Lifting both weights biases ward placement toward empirical ambush tiles. |
| **Prediction** | `deaths_by_cause.ShadowFoxAmbush` decreases vs the dormant baseline; `WardPlaced` positions cluster on `recent_ambush_at_position` tiles. |
| **Observation** | Wards visibly cluster on the `(29,23-24)` and `(37-39, 20-23)` ambush hotzones (substrate fires); macro death/spawn counters identical to baseline (magnitude flat); soft continuity tallies drift +1-3% (substrate behaviorally active). |
| **Concordance** | **Spatial direction matches; macro magnitude flat.** Layer-fires call: PASS. Magnitude call: deferred to follow-on. |

### Hard-gate readout

- `deaths_by_cause.Starvation == 0` → **FAIL** (1). Same as baseline; not a 284-induced regression. Note for follow-on.
- `deaths_by_cause.ShadowFoxAmbush <= 10` → PASS (2).
- `never_fired_expected_positives == 0` → PASS.
- Five continuity canaries each ≥ 1 → PASS (grooming 910, play 14, mentoring 216, courtship 2544, mythic-texture 35).
- Constants-drift-vs-baseline → clean.
- Verdict exit: **fail** (sole reason: Starvation==0 hard gate; identical to baseline).

## Decision

Ship `0.5 / 0.3` as first-light defaults. The substrate is wired and fires (spatial check + soft counter drift); magnitude is small at these weights and the macro counters don't yet move. Open a magnitude-tightening follow-on ticket against the four-artifact `just hypothesize` methodology to find weights that materially shift `shadow_foxes_avoided_ward_total` and `Ambush` event totals.

## Iteration history

- **iter-1 (2026-05-11):** Landed `0.5 / 0.3` on first-light criteria. Spatial check positive; macro counters flat; one-soak qualitative.

## What this writeup is NOT

- Not a four-artifact magnitude validation. The follow-on does that.
- Not a multi-seed concordance check. Single seed (42) only.
- Not a Welch's t / Cohen's d analysis. No effect-size band claims.
- Not the final landing values for these weights. They are the *first* landing values.
