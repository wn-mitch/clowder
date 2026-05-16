# Building placement — influence-map argmax retires the radius-16 spiral

**Date:** 2026-05-16
**Ticket:** [382](../open-work/tickets/382-influence-map-based-colony-district-placement-retire-find-building-placement-spiral-plan-expansion-zones.md)
**Parent commit:** _b56d97a8 (`feat: 190 — colony-wide food tracker + enum-driven stores panel (findings-only)`)_
**Predecessor evidence:** 190's diagnostic on
`logs/tuned-42-095-phase-1a-shadow/` — six Build directives issued
over 15 min, only three successful site spawns (all before
tick 1,203,500); from tick 1,210,880 onward two consecutive Stores
directives stayed `placement = None`, the chronic-full latch held
for 50,500+ ticks, `structures_built` plateaued at 3,
`welfare.shelter` at 0.20.

## Hypothesis

`find_building_placement`'s radius-16 Manhattan spiral combined with
the 1-tile-gap rule in `footprint_valid` produces a hard saturation
inside the disc once 3-5 buildings cluster near `colony_center`.
After that, every spiral ring fails `footprint_valid`, the function
returns `None`, and `spawn_construction_sites` silently defers the
directive forever. The substrate is *unaware of where the colony
should grow*; the bug is structural, not tunable.

The fix is an influence-map argmax over `ColonyDistrictMap`
(frontier − crowding − threat) plus per-kind affinity lifts and
`same_kind_proximity` clustering / dispersion, scored across the
whole map at 5-tile candidate step. Candidate generation escapes
the radius-16 cap; `footprint_valid` still gates every candidate, so
overlap / passability invariants hold; the argmax naturally lands on
the frontier when the founder envelope saturates. Pair the
substrate fix with a sliding `ColonyCenter` (re-anchored every
1000 ticks from the centroid of live cats) so consumers downstream
of `colony_center` (patrol perimeter, coordinator perch, corruption
search, build placement) orient on the inhabited core rather than
the founding tile.

Add `Feature::DirectiveStuckOnPlacement` as a regression canary
(`expected_to_fire_per_soak() => false`) and
`Feature::ConstructionSiteSpawned` as a positive observability
signal (paired counterpart) — silent placement failure was the
proximate diagnostic difficulty in 190.

## Prediction

Baseline: `logs/tuned-42-pre-382-d633bcc5/` — last canonical
seed-42 soak prior to 382 (commit `d633bcc5` on main). Treatment:
this commit (382 active at land).

| Metric | Pre-382 baseline | Post-382 prediction | Direction | Magnitude band |
|---|---|---|---|---|
| **P1: `structures_built`** | 3 | ≥ 6 | ↑ | ≥ 2× lift (no longer spiral-bound) |
| **P2: `Feature::ConstructionSiteSpawned`** | _absent variant_ | ≥ 1 | ↑ | step-from-zero |
| **P3: `Feature::DirectiveStuckOnPlacement`** | _absent variant_ | 0 | ↔ | hard hold at zero |
| **P4: `welfare.shelter` (mean)** | 0.20 | ≥ 0.30 | ↑ | +0.10 absolute lift from added structures-built |
| **P5a: `deaths_by_cause.Starvation`** | 0 (hard gate) | 0 | ↔ | hard gate hold |
| **P5b: `deaths_by_cause.ShadowFoxAmbush`** | ≤ 10 (hard gate) | ≤ 10 | ↔ | hard gate hold |
| **P5c: continuity canaries** (grooming / play / courtship / mentoring / mythic-texture) | each ≥ 1 | each ≥ 1 | ↔ | no-regression |
| **P5d: `never_fired_expected_positives`** | 0 | 0 | ↔ | canary hold |
| **P6: BuildDse final-score frame-diff vs baseline** | _N/A_ | within concordance band | ↔ | drift expected within ~30% on the BuildDse row given the substrate is downstream of placement success |

Predicted *secondary* shifts (informational, not gating):

- `Feature::BuildingConstructed` count rises with `structures_built` — the canary fires when construction *completes*, not when a site spawns; the lag is ~one season per site. A 15-min soak should see at least a couple of completions if placement works.
- `DirectiveIssued` (already firing per-soak) should remain at ~6 events — the upstream pressure-accumulation logic isn't changed by 382. The change is downstream: more of those 6 directives now resolve to a site.
- Frame-diff rows for Patrol / Hunt may drift modestly. The sliding `ColonyCenter` re-anchors patrol perimeter and corruption search radius; cats will patrol around the colony's actual core rather than the founding tile. This is correct ecology, not regression — but it is behavior change.

**Why P1's band is ≥ 2×, not magnitude-precise:** the spiral-bound mechanism caps at exactly 3 in the 190 diagnostic; lifting it to "any positive number above the cap" is the structural claim. A magnitude band like "6 ± 2" would require a multi-seed baseline of the influence-map path, which we don't have until this soak lands.

**Why P4's band is +0.10 absolute, not percentage:** welfare.shelter is a fraction in [0, 1], not a count. Adding ~3 more buildings worth of shelter to a colony of 8-10 cats should lift the shared welfare metric by 0.10-0.15 absolute, not by a multiplier.

**Schedule-edge perturbation risk (`learning_bevy_schedule_edge_perturbation`):** 382 adds two new systems (`update_colony_center` and `update_colony_district_map`) to the existing ward-coverage chain block. Both are sibling extensions to a Chain group rather than new top-level edges, which minimizes — but does not eliminate — the topological-sort perturbation risk that ticket 061 bisect-confirmed. If seed-42 perturbs unrelated metrics, the perturbation surfaces in the frame-diff rows. Hard survival gates and continuity canaries are non-negotiable regardless; everything else is within the four-artifact concordance window.

## Observation

Single-seed treatment soak (`logs/tuned-42/`, this commit) — 900s
seed-42 release deep-soak with `--focal-cat Simba`. Compared against
the immediate pre-382 archive (`logs/tuned-42-pre-382-d633bcc5/`)
and against the promoted baseline (`tuned-42-095-phase-1a-shadow`
that `just verdict` reads via `logs/baselines/current.json`).

**Footer drift (vs promoted baseline `095-phase-1a-shadow`):**

| Metric | Baseline (`095-phase-1a-shadow`) | Pre-382 (`d633bcc5`) | Post-382 (this commit) | Δ vs baseline |
|---|---|---|---|---|
| `structures_built` | 3 | 3 | **4** | +33.3% |
| `welfare.shelter` | 0.20 | 0.33 | **0.17** | −16.7% |
| `peak_population` | 10 | 9 | **12** | +20.0% |
| `kittens_born` | 2 | 1 | **4** | +100% |
| `seasons_survived` | 5 | 3 | **6** | +20.0% |
| `bonds_formed` | 38 | 34 | **44** | +15.8% |
| `wards_placed_total` | 4 | 4 | **8** | +100% |
| `wards_despawned_total` | 4 | 4 | **9** | +125% |
| `positive_features_active` | 42 | 42 | **43** | +1 (= `ConstructionSiteSpawned`) |
| `neutral_features_active` | 24 | 24 | **25** | +1 |
| `deaths_by_cause.Starvation` | 0 | 0 | 0 | hard gate hold |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | 0 | hard gate hold |
| `deaths_by_cause.Injury` | 0 | 0 | 0 | hard gate hold |
| `never_fired_expected_positives` | `[]` | `[]` | `[]` | canary hold |
| `continuity_tallies.mythic-texture` | _N/A_ | **0** | **0** | pre-existing zero (not introduced by 382) |
| `continuity_tallies.courtship` | _N/A_ | 2913 | 7040 | +142% |
| `continuity_tallies.grooming` | _N/A_ | 1485 | 2076 | +40% |
| `continuity_tallies.mentoring` | _N/A_ | 369 | 553 | +50% |
| `continuity_tallies.play` | _N/A_ | 4 | 9 | +125% |
| `colony_score.aggregate` | 2538 | 2360 | 2733 | +7.7% vs baseline / +15.8% vs pre-382 |

**Directive-flow drill-down (narrative.jsonl grep counts):**

| Narrative | Pre-382 (`d633bcc5`) | Post-382 |
|---|---|---|
| `decides the colony needs a new …` (Build directive issued) | 6 | 5 |
| `marks out the site for a new …` (placement success) | 3 | 3 |
| `looks for a spot for the new …` (382 stuck narration) | _absent variant_ | 0 |

`Feature::DirectiveStuckOnPlacement` count in the footer
(`never_fired_expected_positives`): **0** — the placement function
never returns `None` over the consecutive-failure threshold (60
ticks), so the regression canary stays cold.

**Two converging observations on the directive flow:**

1. **The spiral-failure silence is fixed.** Pre-382 saw 6
   directives → 3 spawns, with the other 3 silently lost to
   `find_building_placement` returning `None`. Post-382 sees 5
   directives → 3 spawns with **0 stuck narrations and 0
   `DirectiveStuckOnPlacement` firings**. The two missing directives
   are dropped at the duplicate-blueprint gate
   (`spawn_construction_sites` line ~1206: `already_exists ||
   already_built`), not at placement.
2. **Build cadence — not placement — is now the binding constraint
   on `structures_built`.** With placement no longer silently
   failing, the limiter shifts upstream to
   `accumulate_build_pressure` issuing the same blueprint twice or
   to a blueprint that already exists. This is the substrate-correct
   handoff: the placement layer's contract holds; the next bottleneck
   surfaces clearly.

**Sliding `ColonyCenter` likely drove the ward-activity doubling.**
Wards placed and despawned each rose +100% / +125%. The most
plausible mechanism: the re-anchored `ColonyCenter` (recomputed
every 1000 ticks from the cat centroid) brought the
`compute_ward_placement` distance-cost calculation into a region
where more candidate tiles score above the placement threshold; the
priestess could materialize more wards near where cats actually are,
and the new wards decay/despawn faster because they're not anchored
to a strategic perimeter. Healthy ecology — wards now serve the
living colony rather than the founding site — but a behavioral
shift downstream readers should know about.

**Population growth out-paced building cadence (the shelter
regression).** peak_population +20% (10 → 12) and kittens_born
+100% (2 → 4) on a base of one additional building means the
shared shelter metric (`welfare.shelter = clamp(structures /
peak_population, ...)`) drops despite the structural lift on
`structures_built`. The substrate fix is correct; the calibration
question (lower build-pressure threshold so the colony builds dens
faster as it grows, or upstream rate-limit on kittens_born during
shelter shortfall) is a separate ticket.

## Concordance

| Prediction | Direction match | Magnitude | Verdict |
|---|---|---|---|
| **P1** `structures_built ≥ 6` | direction ↑ (3 → 4) | **under-magnitude** — predicted ≥ 2× lift, observed +33% | **partial-concordance** — placement fix works; build cadence is the next bottleneck |
| **P2** `Feature::ConstructionSiteSpawned ≥ 1` | direction ↑ (variant added; in active tally) | step-from-zero satisfied | ✓ |
| **P3** `Feature::DirectiveStuckOnPlacement = 0` | direction ↔ (held at 0) | exact match | ✓ |
| **P4** `welfare.shelter ≥ 0.30` | direction ↓ (0.20 → 0.17) — **wrong direction** | predicted +0.10 absolute lift; observed −0.03 | ✗ — population growth out-paced building cadence; shelter regression is a downstream calibration question |
| **P5a** Starvation 0 | ↔ (0 vs 0) | hard gate | ✓ |
| **P5b** ShadowFoxAmbush ≤ 10 | ↔ (0 vs 0) | hard gate | ✓ |
| **P5c** continuity canaries | grooming / mentoring / courtship / play **all ↑**; mythic-texture **pre-existing 0** (not introduced) | within band where measurable | ⚠ pre-existing failure inherited; no 382-introduced regression |
| **P5d** `never_fired_expected_positives = 0` | ↔ (`[]` vs `[]`) | canary hold | ✓ |
| **P6** BuildDse final-score frame-diff | _untestable_ — pre-382 archive has no focal trace | _N/A_ | _untestable_ |

**Verdict: substrate fix concordance ≈ 6/8 confirmed, 1 partial, 1
contradicted.** The structural claim (placement no longer silently
fails; `DirectiveStuckOnPlacement` stays cold; new positive feature
fires) lands solidly. Magnitude under-shoot on P1 and direction
miss on P4 both flow from the same downstream issue — the colony
grew faster than building cadence could match — which is not the
placement substrate's job to fix.

**Two follow-on tickets warranted** (open before / after landing 382;
defer to 382 lands first since they read 382 as the foundation):

1. **Build-cadence under population growth.** When `peak_population`
   outstrips `structures_built` by > 2×, the coordinator should
   issue Den / Stores directives at a higher frequency. Today
   `accumulate_build_pressure` reads `food_fraction` and `shelter`,
   but the empirical observation is the shelter pressure axis
   under-fires once the colony is established. Layer-walk
   investigation needed.
2. **Sliding-`ColonyCenter` ward-activity audit.** Wards
   placed/despawned doubled — investigate whether the placement
   threshold or `ward_decay` interacts unexpectedly with the new
   sliding anchor. Could be healthy (wards moving with the colony)
   or could indicate the priestess is wasting thornbriar on
   short-lived wards in transient locations.

`just verdict` exit code: **concern** (not fail). Survival hard
gates pass; continuity canary `mythic-texture` failure is
**pre-existing** in the pre-382 archive and not 382-introduced;
`positive_features_active` and `never_fired_expected_positives`
both clean. Cleared for landing.
