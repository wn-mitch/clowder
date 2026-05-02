---
id: 046
title: FightTarget combat-advantage uses skill-points difference, not damage-per-tick exchange — cats engage threats they can't survive
status: done
cluster: ai-substrate
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [combat.md, ai-substrate-refactor.md, body-zones.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-02
---

## Why

Post-043+044 collapse probe: 5 cats died in a wildlife-combat cluster after the priestess's ward perimeter (ticket 045) failed to deflect a ShadowFox. Once the SF reached the cats, **the cats deliberately engaged** rather than retreating — and lost the per-tick damage exchange every time. Worth fixing independently of 045 because perimeter coverage will never be perfect; the inner ring of "cats engage threats they can win" must hold when the perimeter fails.

## Root cause

`FightTarget` DSE (`src/ai/dses/fight_target.rs`) scores engagement on four axes: distance, threat-level, **combat-advantage**, and ally-proximity. The combat-advantage formula:

```
advantage = self.skills.combat + self.health_fraction − target.threat_power
normalize to [0, 1] with midpoint=0
score via Logistic(10, 0.5)
```

The doc-comment intent: "cats engage at parity-or-better and disengage when the threat clearly outmatches them."

Reality, computed from the run:

| Cat | combat skill | health_frac | SF threat_power | "advantage" | Logistic score |
|---|---|---|---|---|---|
| Calcifer | 1.7 | 1.0 | 0.18 | **2.52** | ~1.0 |
| Mallow | 1.7 | 0.64 | 0.18 | **2.16** | ~1.0 |
| Ivy | 1.6 | 1.0 | 0.18 | **2.42** | ~1.0 |

Cats believe they have a 12× capability advantage. They engage with confidence.

What actually happens during combat (`src/systems/combat.rs:280` — `wildlife_attacks_cats` system):
- **ShadowFox does 0.18 damage per tick to the cat** (constant, plus jitter).
- **Cat's combat skill affects damage *output*, not damage *intake***.
- Cat at full health (1.0) dies in **~6 ticks** of adjacent combat.
- Even if cat skill 1.7 means cat does ~0.5 damage/tick to the SF (SF defense 0.08 from `default_defense`), the SF dies in ~2 ticks while the cat dies in 6. Cat *should* win 1v1 in pure exchange — but RNG jitter, terrain, and any second wildlife (Hawk = 0.10/tick) tip the balance fast.

The advantage formula compares apples to oranges: it sums skill *capability* and health *capacity* on the cat side against threat-power *per-tick output* on the wildlife side. The units don't match, so the score it produces doesn't predict combat outcome.

## Mechanism observed in the cluster

- Ivy at full health, hunger 0.54, last_scores top-3 = [Groom 0.90, Socialize 0.47, Eat 0.46]. **No Fight or FightTarget in her top 3 scores.**
- 21 ticks of `Resting` (waiting near colony).
- Tick 1,215,246: switches to `Guarding/EngageThreat`. Disposition selection appears triggered by a threat-near marker overriding action-score selection (separate code path from per-action softmax).
- 9 ticks later: dead at [36, 12]. Health went 1.0 → 0 in ≤9 ticks.

So Ivy didn't "score Fight high" — her `last_scores` show she was scoring Groom/Socialize/Eat. But once a threat marker triggered Guarding, the GOAP plan auto-chose `EngageThreat` as the first step. **The combat-advantage gate is bypassed by the disposition-trigger path.**

## Two layers to fix

**(1) Combat-advantage formula must model per-tick damage exchange, not capability sums.**
- Estimated cat dps = `combat_skill * dmg_coeff − target.defense`.
- Estimated wildlife dps = `target.threat_power − cat.armor` (or just threat_power for now).
- Advantage = `(cat.health / wildlife_dps) − (wildlife.health / cat.dps)` — positive means cat outlasts the threat, negative means death.
- This requires sticking some damage-coefficient constants somewhere and pulling `WildAnimal.health` into the consideration. Bigger surgery.

**(2) `ally_proximity` should be an eligibility gate for ShadowFoxes specifically.**
- Doc-comment for `FightTarget` cites `assets/narrative/banishment.ron`: "a posse of cats can meaningfully harm one." The design intent is collective banishment.
- `ally_proximity` is currently a 20% consideration weight, not an eligibility filter. A lone cat with high skill scores well enough to engage anyway.
- Cleanest fix: add a `.require()` on the `EligibilityFilter` that gates engagement of ShadowFoxes on having ≥1 ally within 4 tiles. Or generalize: any wildlife with `threat_power > 0.15` requires an ally.
- Single-line surgery; aligns with stated design intent; immediately addresses the cluster.

## Predicted effects

For (2) alone:
- `WildlifeCombat` deaths against ShadowFoxes drop sharply.
- `ShadowFoxBanished` mythic-texture events still fire (when posses gather, the engagement is intentional and effective).
- Lone cats facing a ShadowFox switch to Flee (since FightTarget is no longer eligible, the next-best disposition wins).

For (1) + (2):
- Cats correctly assess engagement viability for *all* wildlife species.
- Engagement against weak species (Snake, Hawk) stays normal.
- Engagement against Foxes becomes risk-aware (cat skill matters more, allies matter more).

## Fix candidate

Land (2) first — it's a one-line change with clear design provenance and immediate effect on the cluster pattern.

```rust
// In FightTarget's EligibilityFilter (src/ai/dses/fight_target.rs):
.require_when(|target_kind| target_kind == WildSpecies::ShadowFox, "HasAllyNearby")
```

If `require_when` doesn't exist as an API yet, two-DSE split: `FightTargetSolo` (excludes ShadowFox) and `FightTargetPosse` (requires ally marker, includes ShadowFox).

Defer (1) to a balance-iteration ticket. The damage-per-tick rebuild is real but bigger and benefits from sweep validation.

## Verification

- **Pre-fix anchor:** `logs/collapse-probe-42-fix-043-044/` — 5 WildlifeCombat deaths in year-15 cluster, all with cats engaging ShadowFox alone or near-alone.
- **Acceptance for (2):** Re-run collapse probe. Lone cats faced with a ShadowFox should switch to Flee rather than Engage. ShadowFoxBanished events should still fire when 2+ cats are nearby a SF (posse formation). `WildlifeCombat` deaths drop, ShadowFoxAmbush deaths possibly rise (cats fleeing don't always escape).
- **No regression:** `Action::Fight` against weaker wildlife (Snake, Hawk, Fox) remains unchanged. Cats still hunt these for food (`HuntDse` is independent).

## Out of scope

- Ward perimeter coverage (ticket 045).
- CriticalHealth interrupt treadmill (ticket 047).
- The damage-per-tick rebuild (deferred — start with the eligibility gate, validate, then revisit).
- Cat armor / damage-mitigation systems (don't exist yet; would be a new mechanic).

## Log

- 2026-04-27: Ticket opened during post-043+044 collapse-probe drill-down. The cluster's proximate cause was inadequate ward perimeter (045), but the inner-ring failure was cats willingly engaging an apex predator they could not survive.
- 2026-05-02: Closed as superseded by the substrate-over-override approach. Both layers of the proposed fix are override-shaped (formula rebuild, eligibility gate); the substrate replacements ship under their own tickets:
  - **Layer 1** (combat-advantage formula units mismatch — `combat + health − threat` mixes capability-and-capacity with per-tick output, producing the "12× advantage" misperception). Superseded by **095** §IAUS Integration §2 (`combat_advantage_normalized` reads `health_derived` instead of `Health.current` — Phase 1 explicitly carries this as a "Breaking change to track") plus **095** §IAUS Integration §1 (dynamic `threat_power` per key-part condition — wounded predators read as less urgent through the Quadratic curve's convex amplification). The remaining piece — a real "is this engagement winnable?" perception scalar capturing dps balance + time-to-kill + ally factor — opens as ticket **133** in this commit.
  - **Layer 2** (lone cats engage ShadowFox; intent: gate engagement on having ≥1 ally nearby). Superseded by the adrenaline-valence substrate framework opened off ticket 047: **103** (`escape_viability`, landed) + **102** (`AcuteHealthAdrenalineFight`, gated on low `escape_viability`) + **105** (`AcuteHealthAdrenalineFreeze`, gated on low `escape_viability` AND low `combat_winnability`) + **108** (`ThreatProximityAdrenalineFlee`, lurches Flee on rising threat density). The eligibility-gate intent ("a lone cat shouldn't engage a SF") becomes substrate score-shaping — `combat_winnability` (133) reads ally count as one of its sub-axes, and a future `EngagementUrgency` modifier consumes it to suppress Fight when winnability is low — rather than a `.require_when(...)` override on FightTarget DSE.

  No code change in this retirement. The substrate work proceeds under 095 / 102 / 105 / 108 / 133 with their own verification playbooks. This closure follows the 112 precedent (close-as-superseded with downstream substrate not yet all landed; the 047 cluster's first-class ticket-level handling).
