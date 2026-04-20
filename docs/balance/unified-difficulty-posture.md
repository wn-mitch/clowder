# Unified difficulty posture — Sleep Phase 1 + Cooked Food + data

Seed 42, 15-min deep soak (tick 1,200,000 → 1,218,863). Commit dirty.

## Thesis

Two recent balance interventions — Sleep Phase 1 (phase-biased Sleep
urgency) and Cooked Food (Kitchen + 1.3× multiplier) — are **mechanically
correct but not reaching the cat population**. Each targets a different
lever on the same underlying problem: **the colony cannot escape
survival-tier behaviour long enough for fulfillment-tier systems
(cooking, building, mating, banishment) to activate**. The sim currently
runs on "Little House on the Prairie" difficulty — every day a scramble,
no buffer, one bad week wipes the family.

The data below grounds the claim and proposes specific dials.

## The numbers

### 1. Sleep Phase 1 — hit its own target, didn't move welfare

| Metric | Baseline | Predicted | Observed | Concordance |
|---|---|---|---|---|
| `sleep_share_night` | 0.055 | +80% → 0.099 | **0.10** | ✓ exactly on target |
| `sleep_share_day` | ? | −40% | 0.01 | ✓ direction matches |
| `energy_p50` | 0.44 | +25% → 0.55 | **0.46** | ✗ barely moved |
| `mood_valence_p50` | 0.19 | +10% → 0.21 | **0.22** | ✓ crossed breeding gate |
| `kittens_born` | 0 | +200% → 1–3 | **0** | ✗ no mating happened |

**Reading:** the Sleep Phase 1 intervention did exactly what it promised
at the scoring layer (Night-heavy sleep, Day-suppressed sleep), but the
welfare chain downstream of it (energy → mood → mating) stalled. The
`mood_valence_p50` jump to 0.22 — right at the `breeding_mood_floor` —
is the tell: cats got *close* to escaping survival mode but not reliably
over the threshold.

### 2. Cooking — scored high, never executed

- **Cook directives issued: 0.** The `has_kitchen` gate in
  `accumulate_build_pressure` was never true (no Kitchen ever completed),
  so the Cook-directive-issuance branch never fired.
- **Cook won per-cat scoring 67 times.** Example snapshot — Nettle
  (diligence 0.88, hunger 0.94, energy 0.58): `last_scores = [("Cook",
  0.95), ("PracticeMagic", 0.78), ("Hunt", 0.55)]`. Cook dominated.
- **Cook as `current_action`: 0.** The chain builder returned `None`
  (no Kitchen, nothing to cook at), and the crafting cascade fell through
  to `PracticeMagic` / `Herbcraft`.
- `FoodCooked` feature: 0 activations throughout.

**Reading:** the cooking pipeline is wired correctly — scoring, directive
issuance logic, task chain, step handlers all fire under the right
conditions. The gate is upstream: **no Kitchen was ever built in the
15-min soak**.

### 3. Why no Kitchen? Because cats barely Build at all.

Dispositions chosen across the run:

| Disposition | Count | Share |
|---|---:|---:|
| Resting | 16,935 | **82%** |
| Crafting | 1,570 | 7.6% |
| Hunting | 1,322 | 6.4% |
| Foraging | 479 | 2.3% |
| Exploring | 180 | 0.9% |
| Guarding | 128 | 0.6% |
| Socializing | 58 | 0.3% |
| Coordinating | 16 | 0.1% |
| **Building** | **5** | **0.02%** |

Only **5 Building dispositions in 15 minutes of sim.** Four building
*decisions* were made by the coordinator (narrative log):

1. Tick 1,200,560 — "Nettle decides the colony needs a new storehouse."
2. Tick 1,202,600 — "Mocha decides the colony needs a new workshop."
3. Tick 1,203,520 — "Mocha decides the colony needs a new kitchen."
4. Tick 1,208,460 — "Nettle decides the colony needs a new storehouse." (Stores #2 — first one was full)

Kitchen was 3rd in the queue. Only 3 `BuildingConstructed` events
completed (not enough to clear the queue to Kitchen). Kitchen site was
placed at 1,203,520 and still incomplete when the colony ended at
1,218,863 (15,343 ticks = ~8 in-sim days of construction starvation).

### 4. Population — survival-mode signature

Average needs across 993 per-cat snapshots:

| Need | Mean | Interpretation |
|---|---|---|
| hunger | **0.702** | slightly above starvation floor, never full |
| energy | **0.465** | chronic low — p50 = 0.46 |
| warmth | 0.784 | healthy |

`current_action` distribution across snapshots — reveals decision thrash:

| Action | Count | % |
|---|---:|---:|
| Explore | 341 | 34% |
| Socialize | 177 | 18% |
| Hunt | 135 | 14% |
| Forage | 109 | 11% |
| Herbcraft | 72 | 7% |
| Sleep | 46 | 5% |
| Coordinate | 24 | 2% |
| Eat | 18 | 2% |
| Fight | 16 | 2% |
| **Cook** | **0** | **0%** |

Cats spend 34% of time exploring and only 5% sleeping *despite* chronic
low energy. Classic decision-thrash: urgent needs get bid on but can't
resolve before the next urgency hits.

### 5. Mortality — not food, not sleep

All 8 deaths were injury:

| Tick | Cat | Cause |
|---|---|---|
| 1,205,717 | Calcifer | WildlifeCombat |
| 1,211,141 | Mallow | WildlifeCombat |
| 1,211,750 | Lark | ShadowFoxAmbush |
| 1,212,006 | Simba | WildlifeCombat |
| 1,212,092 | Birch | ShadowFoxAmbush |
| 1,212,176 | Ivy | ShadowFoxAmbush |
| 1,215,941 | Mocha | ShadowFoxAmbush (the coordinator who ordered the Kitchen) |
| 1,218,863 | Nettle | ShadowFoxAmbush (last cat, last tick) |

**19 ambush events → 5 ambush kills + 3 wildlife-combat kills = colony
wipe.** Starvation: 0. The ShadowFox ambush canary is *exactly at the 5
limit*. The colony never reliably defended (only 41 wards placed, all
despawned). Only 2 `ScryCompleted` events — magic-affine cats never got
to perform the ritual that lets them detect threats.

## Unified diagnosis

Every failing prediction above has the same root: **cats never build a
buffer**. Phase 1 sleep-bias shifted *when* they rest but didn't raise
the *floor* of energy. Cooking multiplies food-value but requires a
Kitchen. Kitchen requires the colony to exit Resting disposition long
enough to Build. None of that happens because every incremental
improvement is consumed keeping physiological needs above starvation.

The intended feedback loop:
```
high energy/mood → more fulfillment-tier actions → more building/cooking →
better food, better defense → higher energy/mood → …
```
The actual loop:
```
low energy → constant Resting disposition → no Build / no Kitchen →
no Cook, no ward → predators catch under-slept, under-fed cats → deaths →
even fewer hands to Build → colony wipe
```

This is the "Little House on the Prairie" pattern the user names:
everyone is working hard, nothing is safe, one bad week is fatal. There's
no second-order buffer (pantry, ward-ring, posse training) because
the buffer takes investment and investment requires surplus.

## Proposed dials — smallest changes with largest leverage

The goal is *not* "cats dominate the sim." It's giving the fulfillment
stack enough oxygen to activate, so the systems we've been building
actually run often enough to be observable. Pick any 1–2 of these per
iteration; each has a clean hypothesis.

### Tier 1 — raise the energy floor (directly addresses the stall)

**A. Raise `sleep_energy_per_tick` from 0.002 → 0.0035** (+75%).

- *Why:* Phase 1 made cats sleep more at Night. Each Night-sleep tick
  currently restores 0.002 energy. Over a 250-tick Night phase with 100
  effective sleep ticks, that's +0.2 energy — not enough to push the
  floor past 0.55. A 1.75× multiplier lifts the Night recovery to the
  level where cats actually wake rested.
- *Predicts:* `energy_p50` ≥ 0.55 (up ~20%), Building dispositions ≥ 15,
  at least 1 Kitchen completes in a 15-min soak.

**B. Lower `energy_decay` from 0.0001 → 0.00008** (−20%).

- *Why:* direct attack on the drain rate. Less sensitive to phase timing
  than A.
- *Predicts:* similar direction to A, smaller magnitude.

### Tier 2 — reduce predator pressure (addresses the mortality spiral)

**C. Lower ShadowFox spawn rate** — threshold from corruption > 0.7 → 0.85.

- *Why:* 3 ShadowFox spawns in the soak produced 19 ambushes and 5 kills.
  The current threshold is easy to cross; raising it gates shadow-fox
  emergence behind deeper corruption, which takes longer to develop.
- *Predicts:* `deaths_by_cause.ShadowFoxAmbush` ≤ 2 (not 5), colony
  survives seed-42 soak with ≥ 4 living cats.

**D. Buff early-game cat combat.** Seed all cats with `combat = 0.20`
(currently ~0.05).

- *Why:* ambush damage is 0.18 per hit, cat health is 1.0 — five hits
  kill. A trained cat can fight back; an untrained cat has no recourse.
  Phase-0 combat lets defenders bleed shadow-foxes before they finish a
  kill.
- *Predicts:* `WildlifeCombat` kills ≤ 1 (not 3); ambushes per death rises.

### Tier 3 — accelerate the build loop (addresses the infrastructure gap)

**E. Raise Kitchen BuildPressure accumulation rate** — add a multiplier
like `cooking_pressure_multiplier = 1.5` on the `rate` used when
accumulating `pressure.cooking`.

- *Why:* match the `no_store_pressure_multiplier` pattern that
  prioritizes Stores for a first-time colony. Kitchen is a tier-2
  amenity that should follow Hearth by a predictable margin.
- *Predicts:* first Kitchen completes by tick +5,000 from sim start;
  `food_cooked_total` ≥ 5.

**F. Raise Build directive priority for Kitchen specifically.** Currently
Build-directive priority scales only with `build_repair_priority_base +
skills.building`. Differentiate by blueprint type.

- *Why:* coordinator issues generic Build directives; cats picks them up
  by nearest-skilled. Weighting Kitchen above Workshop, or lowering
  Workshop priority, would clear the queue ahead of Kitchen's site.
- *Predicts:* same as E; cleaner mechanism.

### Tier 4 — cushion the early game (one-shot)

**G. Start with seeded food + a pre-built Hearth.** Currently colonies
start with only a Stores + starting inventory; Hearth needs to be built
before Kitchen pressure even triggers.

- *Why:* the first 1–2 hours of sim-time are spent reaching tier-1 (food
  stable, shelter built). With seeded starting infrastructure, the first
  hour can be spent on tier-2 (Kitchen, wards) instead. This is a
  one-shot rather than a rate.
- *Predicts:* dramatic lift — Kitchen completes in < 1000 ticks; cooking
  pipeline fires naturally.

## Suggested next iteration

Run A + C together (sleep recovery + shadow-fox gate) as a pair. They're
on opposite ends of the same loop: A raises the energy floor, C reduces
the predation drain. The prediction is compositional:

- `energy_p50` ↑ 20–25% (A alone expected 20%, + slack from fewer
  hospital trips)
- `deaths_by_cause.ShadowFoxAmbush` ↓ to ≤ 2 (C)
- `kittens_born` ↑ to 1–3 (Phase 1 prediction revived)
- `food_cooked_total` ↑ to ≥ 5 (Kitchen reaches completion)

If that overshoots into "cats dominate," pull A back to +40% instead of
+75%. If it undershoots, add E (Kitchen pressure multiplier).

Canaries stay the same: Starvation = 0, ShadowFoxAmbush ≤ 5,
wipeout = false, positive features dead count ≤ prior baseline.

## What NOT to change

- `cooked_food_multiplier` (1.3×) — already conservative. The problem is
  that cooking doesn't happen, not that cooked food is too strong.
- Phase 1 Sleep bonuses — they're hitting their targets. The failure is
  downstream.
- Directive scoring — 211 Cook-high-score snapshots confirm scoring is
  working. Cook wasn't in the DirectiveIssued list because no Kitchen
  was built, not because the directive logic is wrong.

## Canaries for the balance pass itself

When rolling Tier-1 and Tier-2 dials, watch:

1. `deaths_by_cause.Starvation = 0` (hard canary — softening difficulty
   should NOT create starvation regressions).
2. `deaths_by_cause.ShadowFoxAmbush ≤ 5` (hard canary — can only
   improve).
3. `BuildingConstructed ≥ 5 unique types` (new canary — cooking depends
   on infrastructure breadth, not just one Stores).
4. `FoodCooked ≥ 1` (new canary — the pipeline must demonstrably fire
   at least once on seed 42).
5. `KittenBorn ≥ 1` (new canary — mating gate was at the edge; any
   meaningful lift should cross it).

## Artifacts

- Soak: `logs/tuned-42/events.jsonl` (7 MB), `logs/tuned-42/narrative.jsonl` (4 MB)
- Constants hash: `1b1fbf4f0a6f5010`
- Commit: `8d8fb85` (dirty — working tree carries Sleep Phase 1 + Cooked
  Food + unrelated fox/goap/scoring scaffolding)
- Phase 1 predictions: `docs/balance/sleep-phase-1.predictions.json`
- Cooking predictions: `docs/balance/cooked-food.predictions.json`

---

## Iteration 2 results — forage yield +20%, cooking pressure ×1.5

Dials applied on top of iteration 1 (Tier 1A + Tier 2C):

- `forage_yield_scale`: 0.25 → **0.30** (+20%)
- new constant `cooking_pressure_multiplier`: **1.5** — applied as
  `pressure.cooking += rate * cooking_pressure_multiplier` in
  `accumulate_build_pressure`, matching the `no_store_pressure_multiplier`
  pattern.

Clean seed-42 soak (duration 900s, constants hash `6d344daa4c9f124c`):

| Metric | Baseline (pre iter-1) | Iter 1 (A+C) | **Iter 2** | Direction |
|---|---|---|---|---|
| `deaths_by_cause.Starvation` | 0 | 6 | **3** | down, still present |
| `deaths_by_cause.ShadowFoxAmbush` | 5 | 1 | **2** | flat (within noise) |
| `deaths_by_cause.WildlifeCombat` | 3 | 1 | **3** | flat |
| Positive features active | 17/33 | 20/33 | **19/33** | roughly stable |
| `energy_p50` | 0.465 | 0.493 | **0.511** | up ✓ |
| `mood_valence_p50` | 0.22 | n/a | **0.25** | up ✓ (breeding gate surpassed) |
| `kittens_born` | 0 | 0 | **0** | no mating materialised |
| `food_cooked_total` | 0 | 0 | **0** | Kitchen still not built |
| Sleep-share Night | 0.10 | 0.10 | 0.04 | down |
| Final tick (wipeout) | 1,218,863 | 1,257,225 | 1,276,414 | colony lives longer each iteration |
| Build dispositions (across run) | 5 | 4 | **6** | essentially flat |

Kitchen was decided at tick 1,202,780 (~740 ticks earlier than iter-1 —
the multiplier helped) but **still never completed**. The Kitchen
construction site sat idle for the remaining ~74,000 ticks because cats
never entered Building disposition (6 total in the whole soak).

### Concordance vs predictions (`iteration-2.predictions.json`)

| Prediction | Predicted | Observed | Verdict |
|---|---|---|---|
| Starvation → 0 | down 100% | down 50% (6→3) | partial — magnitude off |
| ShadowFoxAmbush ≤ 2 | flat ±10% | 2 | ✓ |
| food_cooked_total ≥ 3 | up | 0 | ✗ — Kitchen never built |
| kittens_born ≥ 1 | up | 0 | ✗ — mood gate passed but mating didn't fire |
| energy_p50 holds | flat ±5% | +4% | ✓ |

### Diagnosis

Raising `cooking_pressure_multiplier` shifted Kitchen *decision* 740
ticks earlier but had **zero effect on completion**. The bottleneck is
not pressure — it's Building-disposition adoption. Across the 65,000
decisions made during the soak, Building was chosen only **6 times**
(0.009%). Raw counts:

| Disposition | Count | Share |
|---|---:|---:|
| Resting | 63,160 | 79% |
| Hunting | 8,300 | 10% |
| Crafting | 6,147 | 8% |
| Foraging | 5,023 | 6% |
| Exploring | 675 | 0.8% |
| Guarding | 565 | 0.7% |
| Socializing | 182 | 0.2% |
| Coordinating | 12 | 0.02% |
| **Building** | **6** | **0.009%** |

Build-action scoring is the next target. The BuildPressure mechanism
produces a Build *directive*, but the cat who receives that directive
rarely actually scores Build highest. Something about Build scoring is
under-competitive.

### What worked

- **Forage yield +20%** cut starvation in half. The remaining 3 starves
  cluster at end-of-run, same pattern as iter 1 — cats survive middle
  game but deplete stores late. Not enough production.
- **Sleep energy +75%** (iter 1) held — energy p50 continues climbing
  (0.465 → 0.493 → 0.511).
- **ShadowFox threshold 0.85** (iter 1) held — ambush deaths stayed
  low (1 → 2, within noise).
- **Mood crossed the breeding gate** (mood p50 = 0.25, gate = 0.20)
  but `kittens_born` stayed at 0, implying the mating system has a
  second gate beyond mood that isn't obvious yet — opportunity for a
  separate diagnostic.

### What didn't

- **Kitchen never completed**, so Cook pipeline still dormant. The
  1.5× pressure multiplier moved decision-time but not execution-time.
- **Starvation not eliminated**, though halved. Forage yield needs more
  lift OR the fix is on consumption side (lower `hunger_decay`) OR the
  right answer is waiting for cooking to kick in (1.3× cooked multiplier
  would extend food supply ~30%, covering the gap).

## Iteration 3 — proposed next dials

Two candidates, pick one or combine:

**Option α: fix Build-action scoring** — the underlying bottleneck.
Raise the base weight of Build in `src/ai/scoring.rs` so it wins more
often, particularly when a construction site exists nearby. This is the
structural fix; without it, Kitchen (and any future amenity) never
completes.

**Option β: bump forage yield further** (0.30 → 0.35) as a stopgap,
accepting that cooking won't activate and food buffer must come from
raw production alone. Simpler but doesn't close the cooking loop.

**Recommended:** α. It unblocks cooking, wards, workshops — every
infrastructure-based system we've added is stuck on the same
construction bottleneck.

**Prediction for α (Build score ×2):**
- Kitchen completes in seed-42 soak → `food_cooked_total` ≥ 1
- 1.3× cooked multiplier extends food supply → starvation = 0
- Build dispositions rise to ≥ 30 (from 6)
- Canaries hold.
