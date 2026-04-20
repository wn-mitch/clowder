# Sleep That Makes Sense — GOAP for Every Species

## Vision

Every animal in the sim chooses its next action through a utility-AI planner whose scoring is **species-specific**, **circadian-aware**, and **ecologically honest**. A cat forages at Dawn, pursues fulfillment during Day, hunts at Dusk, and sleeps through Night. A mouse emerges at midnight. A fox roves Dusk→Dawn. No animal's activity comes from a timer or a phase-agnostic constant. Sleep, foraging, breeding, and territorial behavior all fall out of the same mechanism the cat planner already uses.

Today only cats have a full planner. Foxes have a partial one (`src/ai/fox_planner/`). Prey run on timer-based emergence. Shadow-foxes spawn on corruption triggers. The gap is why the sim feels flat across time of day — and why cat breeding gates never pass.

This is a multi-pull initiative. This doc is the rubric.

## Why now

From the seed-42 15-min soak (`logs/tuned-42/events.jsonl`):

| Need | Pass rate | p50 |
| --- | --- | --- |
| hunger > 0.6 | 70% | 0.68 |
| **energy > 0.5** | **29%** | **0.44** |
| mood_eff > 0.2 | 45% | 0.19 |
| all three (breeding gate) | 15.8% | — |

Per-cat longest consecutive `energy > 0.5` streak: 14–29 snapshots (7–15% of a season). `physiological_satisfaction = smoothstep(0.15, 0.65, min(h,e,w))` never reaches `contentment_phys_threshold = 0.85`. The contentment modifier (+0.05, `src/systems/mood.rs:70`) never fires. Baseline `mood.valence = optimism * 0.4 - 0.05 ≈ 0.15` stays pinned. Mating gate demands `mood > 0.2`; it almost never passes.

## Design principle: cats are the player's protagonists

The cat rhythm has to serve both ecology and viewability. Domestic cats sleep 12–16 hours a day, mostly in polyphasic chunks across Day and Night. In the sim, Night is the off-screen time — players often fast-forward or look elsewhere. Day is the prime viewing window. So:

- **Dawn and Dusk** — feeding peak. Hunt and Forage win; Sleep near zero.
- **Day** — fulfillment focus. Socialize, PracticeMagic, Build, Herbcraft, Explore. A tired cat may nap, but the default is active. Sleep gets a tiny positive offset (tie-break, not a pull).
- **Night** — core sleep. Strong Sleep bias; only threat or critical hunger interrupts.

This is narrower than real feline biology (real cats nap heavily through Day), but ecological honesty serves gameplay second, watchability first. Call the design choice out explicitly: we're producing a crepuscular cat with a Night-heavy sleep schedule, not a perfectly-simulated Felis catus.

## Species rhythm profiles

Over the existing four-phase day (`DayPhase::{Dawn, Day, Dusk, Night}`, `src/resources/time.rs:50`, 250 ticks each):

| Species | Analogue | Active phases | Rest phases | Notes |
| --- | --- | --- | --- | --- |
| **Cat** | Felis catus (gameplay-biased) | Dawn (feed), Day (fulfillment), Dusk (feed) | Night (core sleep), Day nap when tired | Night-heavy by design; see above. |
| **Fox** | Vulpes vulpes | Dusk, Night, Dawn | Day | Crepuscular/nocturnal. |
| **Shadow-fox** | fantasy | Night, Dusk | Dawn, Day | Sun-averse. Day is cat refuge. |
| **Mouse** | Mus musculus | Night, Dusk | Dawn, Day | Nocturnal. |
| **Rat** | Rattus spp. | Dusk, Night | Dawn, Day | Nocturnal with dusk onset. |
| **Rabbit** | Oryctolagus cuniculus | Dawn, Dusk | Day (warren), Night | Crepuscular, warren-anchored. |
| **Fish** | general | Dawn, Dusk (feeding) | Day (deep), Night | Mild bias. |
| **Bird** | songbirds | Dawn, Day | Dusk transition, Night (roost) | Diurnal. |

Profiles are soft — a sleeping cat still flees an ambush, a resting mouse still runs when stepped on.

## Target architecture

All of this lives in `src/ai/`. The cat-specific modules become the template; the pattern generalizes.

```
src/ai/
├── planner/          <-- already generic
├── scoring/
│   ├── cat.rs, fox.rs, shadowfox.rs, mouse.rs, rat.rs,
│   ├── rabbit.rs, fish.rs, bird.rs
│   └── shared.rs     <-- day_phase bonus tables, level_suppression, jitter
├── actions/
│   ├── cat.rs (existing Action enum)
│   ├── prey.rs (Forage, Den, Flee, Breed, Explore)
│   ├── fox.rs (Hunt, Patrol, Den, Scent)
│   └── shadowfox.rs (Ambush, Shroud, Retreat)
└── species_ai.rs     <-- SpeciesKind -> planner dispatch
```

Shared scoring primitives live in `scoring/shared.rs`. Each species file exports `score_actions(ctx) -> Vec<(Action, f32)>` following the same interface.

## Phase 1 — Cat Sleep becomes phase-aware

Smallest pull. Lands first, validates the shape, and unblocks kittens on seed 42 as a downstream consequence.

### Change

`src/ai/scoring.rs:192-206`:

```rust
let day_phase_offset = match ctx.day_phase {
    DayPhase::Dawn => s.sleep_dawn_bonus,
    DayPhase::Day  => s.sleep_day_bonus,
    DayPhase::Dusk => s.sleep_dusk_bonus,
    DayPhase::Night => s.sleep_night_bonus,
};
let urgency = ((1.0 - ctx.needs.energy) * s.sleep_urgency_scale + day_phase_offset)
    * ctx.needs.level_suppression(1);
```

**Proposed defaults (protagonist-weighted):**

| Phase | Bonus | Rationale |
| --- | --- | --- |
| Dawn | 0.0 | Feeding peak. No Sleep pull. |
| Day | 0.1 | Fulfillment focus. Tiny tie-break so exhausted cats can nap; everyday cats don't. |
| Dusk | 0.0 | Feeding peak. No Sleep pull. |
| Night | 1.2 | Core sleep. Only threat/hunger interrupts. |

Urgency samples:

| energy | Dawn | Day | Dusk | Night |
| --- | --- | --- | --- | --- |
| 0.9 | 0.12 | 0.22 | 0.12 | 1.32 |
| 0.7 | 0.36 | 0.46 | 0.36 | 1.56 |
| 0.5 | 0.60 | 0.70 | 0.60 | 1.80 |
| 0.3 | 0.84 | 0.94 | 0.84 | 2.04 |
| 0.2 | 0.96 | 1.06 | 0.96 | 2.16 |

Hunt/Explore/Socialize/PracticeMagic routinely score 1.4–2.0. Under this curve:

- **Night** (any energy): Sleep dominates. Core rest period.
- **Dawn / Dusk**: Sleep barely registers. Hunt/Forage win — feeding drama for the player.
- **Day**: Sleep loses to fulfillment activities at normal energy (≥ 0.5). A severely tired cat (energy ≤ 0.3) may still nap, which is fine — it's a pressure-release valve, not the default.

"Barring serious threats" is already handled. `Flee` urgency `3.0 * (1 - safety)` beats any Sleep bonus at safety ≤ 0.6. Plan interrupts fire on Ambush events.

### Plumbing

`ScoringContext` at `src/ai/scoring.rs:110` gains `day_phase: DayPhase`. Callers in `src/systems/goap.rs:2732` and `src/systems/disposition.rs:2128` already compute `DayPhase::from_tick` — thread it through.

### Constants

Add to `ScoringConstants` in `src/resources/sim_constants.rs`:

```rust
pub sleep_dawn_bonus: f32,
pub sleep_day_bonus: f32,
pub sleep_dusk_bonus: f32,
pub sleep_night_bonus: f32,
```

Each with `#[serde(default = ...)]`.

### Verification

- `just soak 42`.
- Expect a visible Night-heavy Sleep pattern in `CatSnapshot.current_action` distribution.
- `energy_p50` rises from 0.44 → ~0.55 (smaller lift than a flat boost would give, because Day naps are suppressed).
- `mood_valence_p50` should still reach ~0.21 — contentment fires during extended Night sleep when hunger/warmth hold.
- `kittens_born > 0` on seed 42.
- `just check-canaries` — Starvation 0, ShadowFoxAmbush ≤ 5.

### Optional follow-on: fulfillment day bonus

If Phase 1 lands but cats spend Day bored/idle, add matching day-phase bonuses to fulfillment actions (PracticeMagic, Build, Herbcraft, Socialize, Explore) so Day's energy is deliberately channeled into the higher-Maslow stack. Treat as a v1.1 adjustment, not a gate on the Phase 1 soak.

## Phase 2 — Fox and shadow-fox circadian

Foxes have a planner (`src/ai/fox_planner/`). Add day-phase bias to Hunt / Patrol / Den scoring:

| Species | Dawn | Day | Dusk | Night |
| --- | --- | --- | --- | --- |
| Fox Hunt bonus | +0.3 | -0.2 | +0.5 | +0.7 |
| Fox Den bonus | 0.0 | +0.5 | 0.0 | 0.0 |
| ShadowFox Hunt bonus | -0.2 | -0.5 | +0.3 | +0.8 |

ShadowFox spawn (corruption-driven) is untouched; only per-tick action scoring shifts. Expect ambushes to cluster Dusk→Night; Day becomes a genuine refuge.

### Files

- `src/ai/fox_planner/actions.rs` — thread `day_phase` into scoring, add bonus tables.
- `src/ai/fox_scoring.rs` — same.

## Phase 3 — Prey migrate from timers to GOAP

Prey today emerge and forage on cadence set in `src/systems/prey.rs` and `src/world_gen/prey_ecosystem.rs`. Migration target is a minimal per-prey scoring function:

```rust
// src/ai/scoring/mouse.rs
pub fn score_actions(ctx: &PreyScoringContext) -> Vec<(PreyAction, f32)> {
    let phase = ctx.day_phase;
    let forage_phase_mult = match phase {
        DayPhase::Night => 1.0, DayPhase::Dusk => 0.7,
        DayPhase::Dawn => 0.3, DayPhase::Day => 0.0,
    };
    let mut scores = vec![];
    scores.push((PreyAction::Forage, ctx.needs.food * forage_phase_mult));
    scores.push((PreyAction::Den, 0.5 - forage_phase_mult * 0.4 + ctx.fear * 0.8));
    if ctx.predator_nearby { scores.push((PreyAction::Flee, 5.0)); }
    scores
}
```

Prey gain a lean `Needs` struct (`food: f32`, `fear: f32`) — narrower than cats' Maslow stack. Denning becomes a choice, not a scheduled event. Cat hunt success becomes "how many mice are active in this tile at this phase" rather than a constant.

Migrate one species at a time. Start with mouse; validate; extend to rat, rabbit, fish, bird. Rabbit has warren attachment; birds roost rather than den; fish stay in water — handle per-species subtleties in the per-species file, not in `shared.rs`.

### Files

- `src/ai/scoring/{mouse,rat,rabbit,fish,bird}.rs` — one per species.
- `src/ai/actions/prey.rs` — `PreyAction` enum.
- `src/systems/prey.rs` — tick each prey through its planner instead of the timer. Keep the timer as a fallback `run_if` until migration completes.
- `src/components/prey.rs` — add `PreyNeeds`.

## Phase 4 (deferred) — Per-species energy needs

Cats have `Needs::energy` with decay + Sleep recovery. Foxes and prey don't. For full ecological honesty, each species gains a narrow `energy` need with species-specific decay (nocturnal species decay slower at Night). Flagged but not in this initiative — Phases 1–3 already deliver observable behavior change.

## Hypothesis (per CLAUDE.md balance methodology)

**Ecological claim:** animals self-regulate activity by time of day through a per-species utility function. The resulting population dynamics reflect real ecology: crepuscular predators hunting crepuscular prey, nocturnal prey hiding from diurnal predators, shared refugia in phases nobody claims.

**Predictions on seed 42, cumulative by phase:**

| Metric | Baseline | After Ph 1 | After Ph 2 | After Ph 3 |
| --- | --- | --- | --- | --- |
| Cat energy p50 | 0.44 | ~0.55 | ~0.56 | ~0.57 |
| Cat mood p50 | 0.19 | ~0.21 | ~0.21 | ~0.23 |
| kittens_born | 0 | 1–3 | 1–3 | 2–5 |
| Ambushes at Dusk+Night share | ~25% | ~25% | ~65% | ~65% |
| Day-active cats in snapshots | low | high | high | high |
| Mouse hunts succeeding at Night | — | — | — | >> Day |

**Guardrails (all phases):**

- `Starvation = 0` on seed 42.
- `ShadowFoxAmbush deaths ≤ 5` on seed 42.
- Colony survives the 15-min soak.
- Multi-seed sweep (42, 99, 7, 2025, 314) after each phase — predictions generalize.

## Out of scope

- Weather × phase coupling.
- Sensing changes (night vision, scent falloff by phase) — the sensing pipeline already has phase awareness.
- Mood redesign. Contentment firing naturally once energy recovers is the lever.
- Predator pressure tuning (ShadowFox spawn rate, fox patrol radius). Separate axis.

## Migration summary

One pull per phase. Each lands with its own soak, hypothesis, and concordance check.

| Phase | Size | Landing signal |
| --- | --- | --- |
| 1. Cat Sleep phase-bias (protagonist-weighted) | S | Cat energy/mood p50 rise, kittens_born > 0, Day remains active |
| 2. Fox / ShadowFox phase-bias | S | Ambushes concentrate Dusk/Night |
| 3. Prey GOAP migration | M–L | Hunt success phases emerge; one species at a time |
| 4. Per-species energy needs | L | Deferred |
