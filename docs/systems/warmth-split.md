# Warmth Split — temperature need vs social warmth fulfillment axis

## Purpose

The `needs.warmth` axis in `src/components/physical.rs` and
`src/systems/needs.rs` currently conflates two distinct phenomena.
It is drained by weather and season (physiological body-heat) and
simultaneously restored by grooming another cat
(`src/steps/disposition/groom_other.rs:47`, affective closeness).
Both semantics land in the same scalar.

The consequence: a cat near a hearth is immune to loneliness at the
needs level, and a socially-isolated cat who happens to be well-
sheltered produces no warmth-deficit signal. Hearth and grooming
fill the same bar.

The split: `needs.temperature` (physiological, Maslow L1) plus
`social_warmth` (fulfillment-layer axis, post-refactor-doc §7.W).
This is a prerequisite for the warring-self dynamics of §7.W.2
landing legibly — a cat must be able to be physically warm and
socially starving at the same time. Otherwise the losing-axis signal
the narrative system depends on is drowned out by the first shelter
the cat finds.

This doc captures the design; implementation is staged across three
follow-on commits (see **Migration staging** below).

## Current state inventory

All existing `needs.warmth` call sites, grouped by semantic intent.
Numbers from a fresh `rg -n 'needs\.warmth|warmth_drain|warmth_bonus'`
over `src/` on commit `HEAD` at the time of writing.

### Drained by (temperature-semantic — clean)

| Site | File:line | Notes |
|---|---|---|
| Base drain | `src/systems/needs.rs:69,87` | Always-on, tick-scaled |
| Weather drain | `src/systems/needs.rs:52–61` | Snow, Storm, Wind, HeavyRain, LightRain |
| Season drain | `src/systems/needs.rs:62–68` | Winter, Autumn |

All of these are unambiguously temperature-semantic. Move to
`needs.temperature` unchanged.

### Restored by (mixed semantic — the conflation)

| Site | File:line | Current semantic | Post-split target |
|---|---|---|---|
| Hearth bonus | `src/systems/buildings.rs:63–64,81` | Temperature | `needs.temperature` |
| Hearth cold bonus | `src/systems/buildings.rs:64` (`hearth_warmth_bonus_cold`) | Temperature | `needs.temperature` |
| Den bonus | `src/systems/buildings.rs:53` (`den_warmth_bonus`) | Temperature | `needs.temperature` |
| Sleep gain | `src/steps/disposition/sleep.rs:12` (`sleep_warmth_per_tick`) | Temperature | `needs.temperature` |
| Self-groom gain | `src/steps/disposition/self_groom.rs:13` (`self_groom_warmth_gain`) | Temperature (fur maintenance keeps heat) | `needs.temperature` |
| **Groom-other gain** | `src/steps/disposition/groom_other.rs:47` (`groom_other_warmth_gain`) | **Mixed — currently feeds the groomer's own warmth need from a social act** | **`social_warmth` (both parties gain)** |

The groom-other entry is the **primary semantic fix**. In the
current code a cat can keep its warmth need topped up by grooming
other cats, which only makes physical sense if warmth *is* the social
axis. Under the split, grooming another cat raises both parties'
`social_warmth` fulfillment axis; neither party's body temperature
moves.

### Read by (consumer sites to update on migration)

| Site | File:line | Current semantic | Post-split reads |
|---|---|---|---|
| Self-groom scoring | `src/ai/scoring.rs:286` | Temperature deficit | `needs.temperature` |
| Self-groom scoring (disposition) | `src/systems/disposition.rs:698` | Temperature deficit | `needs.temperature` |
| Self-groom scoring (goap) | `src/systems/goap.rs:998` | Temperature deficit | `needs.temperature` |
| Rest-completion | `src/systems/disposition.rs:1069` | Temperature threshold | `needs.temperature` |
| Rest-completion (goap) | `src/systems/goap.rs:1413` | Temperature threshold | `needs.temperature` |
| Warmth deficit | `src/systems/disposition.rs:1112` | Temperature deficit | `needs.temperature` |
| Planner threshold | `src/systems/goap.rs:3957` | Temperature gate | `needs.temperature` |
| UI bar | `src/rendering/ui/cat_inspect.rs:337` | Single bar | Two bars: Temperature + Social Warmth |
| UI data | `src/ui_data.rs:106` | Single value | Two values |
| Snapshot construction | `src/systems/goap.rs:273,1168,2480,2552,2609` | Single field | Two fields |
| Set-full on bootstrap | `src/components/physical.rs:373`, `src/ai/scoring.rs:1400,1536,2381,2406,2430,2459` | Initializer | Two initializers |
| Set-zero on kill | `src/ai/scoring.rs:1615`, `src/systems/needs.rs:449` | Test fixture zero | Two fixture zeros |

Nothing in this list is subtle — each call site either reads or
writes one scalar. The mechanical rename of phase 2 (below) touches
all of them without behavior change.

## The split design

### `needs.temperature` — physiological, Maslow L1

- **Layer.** Stays in `Needs` component (`src/components/physical.rs`).
  Maslow pre-gate (`docs/systems/ai-substrate-refactor.md` §3.4)
  applies: a cold cat below survival threshold suppresses all higher
  needs and fulfillment pursuits. Unchanged from current
  `needs.warmth` semantics at this layer.
- **Drained by.** Weather, season, base drain. Unchanged.
- **Restored by.** Hearth, den, sleep, self-groom (fur maintenance
  keeps heat). All existing restoration sites except groom-other.
- **Read by.** Survival-critical gates, rest-completion predicates,
  planner thresholds, UI (left bar).
- **Scoring interaction.** Below a low threshold, temperature
  dominates every cat's decision — fleeing to shelter outranks
  anything else. That's `check_anxiety_interrupts` territory
  (`src/systems/disposition.rs:93`) and does not change.

### `social_warmth` — fulfillment-layer axis, post-§7.W

- **Layer.** New axis contributing to the Fulfillment register
  specified in `docs/systems/ai-substrate-refactor.md` §7.W.1. Does
  *not* live in `Needs`; lives alongside other fulfillment axes
  (the exact container is gated on §7.W implementation — see phase
  3 below).
- **Drained by.** Time alone (slow), isolation (faster), bond-partner
  death (step drop). Exact decay shape is balance-thread work.
- **Restored by.** Grooming others (both parties gain), being
  groomed, huddling with kin, mating-partner proximity,
  socializing. Any pair-positive social action feeds it.
- **Read by.** Fulfillment-bar aggregation, narrative templates that
  want a "lonely despite comfort" signal, mood cascade's
  losing-axis tension pathway (§7.W.2). Not read by Maslow gates —
  a socially-starved cat does *not* cross survival threshold,
  matching the vision-doc ecology where cats can survive without
  bonds even if they shouldn't want to.
- **Subject to §7.W dynamics.** Sensitization (off by default —
  ordinary social axis), tolerance (mild), diversity-modulated decay
  (narrow-source cats who get all their fulfillment from one bonded
  partner decay faster when that partner is absent). This is the
  mechanism by which a bereaved cat's grief becomes mechanically
  visible.

## Restoration reassignment table

Consolidated before/after for the six current restoration sites:

| Site | Current | Phase 3 target |
|---|---|---|
| Hearth | `needs.warmth += bonus` | `needs.temperature += bonus` |
| Hearth cold-bonus | `needs.warmth += bonus` | `needs.temperature += bonus` |
| Den | `needs.warmth += bonus` | `needs.temperature += bonus` |
| Sleep | `needs.warmth += per_tick` | `needs.temperature += per_tick` |
| Self-groom | `needs.warmth += gain` | `needs.temperature += gain` |
| **Groom-other** | `groomer.needs.warmth += gain` | `groomer.fulfillment.social_warmth += gain`; `groomed.fulfillment.social_warmth += gain` *(both parties)* |

The groom-other row is where the social-axis mechanics actually
change. Every other row is a pure rename.

## Migration staging

Four phases, each a separate commit. Phases 1–3 are substrate
work; phase 4 is a balance-thread.

### Phase 1 — design capture (current commit)

This doc + the §7.W.4(b) cross-reference in the refactor doc. No
code changes. Review gate: design approval by @will.

### Phase 2 — mechanical rename

Rename `needs.warmth` → `needs.temperature` across ~30 call sites.
Includes the `Needs` struct field, all constants named `*_warmth_*`
in `src/resources/sim_constants.rs` (rename to `*_temperature_*`),
UI labels, snapshot fields, test fixtures. No behavior change.

Verification:
- `just check` passes
- `just test` passes
- `just soak 42` header `sim_config` block byte-identical to pre-
  rename baseline (expected — constants renamed but numerically
  unchanged)
- Characteristic-metric drift ≤ ±10% (measurement noise; CLAUDE.md
  balance methodology)

Review gate: diff is a mechanical rename; no new logic.

### Phase 3 — `social_warmth` implementation

Gated on §7.W Fulfillment component/resource landing. Adds
`social_warmth` as an axis on whatever container §7.W lands.
Modifies `src/steps/disposition/groom_other.rs:47` to stop feeding
`needs.temperature` and start feeding both parties' `social_warmth`.
Adds drain (time alone / isolation) and the additional restoration
sources (huddling, mating-partner proximity, socializing) in sites
that currently only affect relationship values. UI gains a second
bar for `social_warmth` in the cat inspect panel.

Verification:
- `just check`, `just test` pass
- Seed-42 15-min soak: a cat isolated from grooming shows
  `social_warmth` deficit while `needs.temperature` stays high —
  the hearth-vs-loneliness semantic fix is observable.
- Mood cascade legibility: bereaved cats show sustained valence drop
  tied to `social_warmth` loss, separable from temperature.

Expected balance impact: small. The cats who were passively topping
up their warmth via grooming no longer do so at the temperature
layer, which may raise self-groom and sleep frequency modestly.
Starvation canary must remain 0.

### Phase 4 — balance-thread retune

Dedicated balance iteration documented in
`docs/balance/warmth-split.md` (new). Hypothesis-shape per CLAUDE.md
balance methodology:

> Removing the social-grooming path from temperature restoration
> reduces the total temperature-inflow rate for well-bonded cats by
> ~10–20%. Without compensating, temperature drain rates produced
> for the conflated axis will push some cats below the temperature
> threshold more often. Prediction: without retune, observable cold-
> stress events rise 1.5–3× on seed 42; with a ~15% reduction in
> base and weather warmth drain, cold-stress events return to
> pre-split baseline.

Acceptance: full four artifacts (hypothesis / prediction /
observation / concordance) per the methodology. Starvation and
temperature-death canaries remain 0 at acceptance.

## Non-goals

- **Elastic temperature** (wet-fur modifier, wind-direction heat
  loss, radiative cooling from cloud cover). Could land in phase 4
  but not required. Current weather drain shape is sufficient to
  drive the balance-thread retune.
- **Social warmth from strangers.** Only positive-affinity
  relationships contribute. Indifferent or low-affinity cats
  grooming each other is a follow-on design question (probably
  conditional on relationship valence) — noted here, not resolved.
- **Predator proximity as social-warmth drain.** Not in scope;
  predator effects land in sensing and mood, not social-warmth.
- **Collapsing temperature into ambient-environment readings.** The
  existing needs-scalar representation is sufficient for the
  current scope; heat-map-based temperature is out of scope.

## Cross-refs

- `docs/systems/ai-substrate-refactor.md` §7.W — axis-capture
  primitive that `social_warmth` participates in (§7.W.4(b)
  references this doc).
- `docs/systems/ai-substrate-refactor.md` §3.4 — Maslow pre-gate;
  `needs.temperature` stays under it.
- `docs/systems/project-vision.md` §5 — broaden-sideways register;
  grooming is one of the named behaviors, and the `social_warmth`
  axis is how it gets mechanical teeth beyond a narrative tick.
- `docs/open-work.md` — pointer for the phase-2/3/4 migration.
- `src/components/physical.rs` — `Needs` struct (rename target).
- `src/resources/sim_constants.rs` — `*_warmth_*` constants (rename
  targets).
- `src/steps/disposition/groom_other.rs:47` — the primary bleed
  site the split fixes.

## Tuning Notes

_Numeric values (drain rates, restoration magnitudes, decay shapes)
are balance-thread work, not design-doc content. Record iterations
in `docs/balance/warmth-split.md` once phase 4 begins._
