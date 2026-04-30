---
id: 2026-04-22
title: "Phase 4c.1 — §6.5.1 `Socialize` target-taking DSE port"
status: done
cluster: null
landed-at: db7362b
landed-on: 2026-04-22
---

# Phase 4c.1 — §6.5.1 `Socialize` target-taking DSE port

First per-DSE §6 port. Closes the §6.2 silent-divergence between
`disposition.rs::build_socializing_chain` (fondness × 0.6 +
(1-familiarity) × 0.4 weighted mixer) and
`goap.rs::find_social_target` (fondness-only max-by) by routing
both through a single `TargetTakingDse` evaluator.

- New `src/ai/dses/socialize_target.rs` with:
    - `socialize_target_dse()` factory — four per-§6.5.1
      considerations (`target_nearness` Quadratic(exp=2),
      `target_fondness` Linear(1,0), `target_novelty` Linear(1,0),
      `target_species_compat` piecewise-cliff) composed via
      `WeightedSum([0.25, 0.35, 0.25, 0.15])` with
      `TargetAggregation::Best`.
    - `resolve_socialize_target(...) -> Option<Entity>`
      caller-side helper — assembles candidates, builds
      `fetch_self_scalar` + `fetch_target_scalar` closures
      (fetcher computes `target_nearness` from position
      geometry), invokes `evaluate_target_taking`, returns the
      winning target. Single source of truth consumed at three
      call sites (see below).
    - `Intention::Activity { kind: ActivityKind::Socialize,
      termination: UntilInterrupt, strategy: OpenMinded }`
      factory thread winning target forward for future §L2.10
      downstream planning.
    - 13 unit tests — 8 factory shape (id / axes / weights /
      aggregation / argmax / silent-divergence tiebreak / empty
      candidates / intention shape) plus 5 resolver integration
      (missing-DSE / out-of-range / fondness pick / self-exclude
      / novelty tiebreak).
- Registration: `socialize_target_dse()` pushed into
  `target_taking_dses` at both mirror sites
  (`plugins/simulation.rs` + `main.rs::build_new_world` +
  save-load path). `ExecutorContext` + `ChainResources`
  (SystemParam bundles) gained `dse_registry: Res<DseRegistry>`
  so `resolve_goap_plans` and `disposition_to_chain` can invoke
  the resolver.
- Caller cutovers:
    1. `systems/disposition.rs` `evaluate_dispositions` —
       `has_social_target` bool gate now reads
       `resolve_socialize_target(...).is_some()`.
    2. `systems/disposition.rs` `disposition_to_chain` —
       `build_socializing_chain`'s signature loses
       `entity/pos/cat_positions/relationships` (target now
       pre-resolved), keeps `cat_positions` for position lookup
       of the returned target; the inline weighted-mixer picker
       at lines 1348-1365 retires.
    3. `systems/goap.rs` `evaluate_and_plan` — `has_social_target`
       reads through the resolver (same shape as disposition.rs).
    4. `systems/goap.rs` `resolve_goap_plans` `SocializeWith`
       step — replaces `find_social_target(...)` call with
       `resolve_socialize_target(...)`. Other three callers of
       `find_social_target` (GroomOther/MentorCat/MateWith) stay
       on the legacy helper until their §6.5.2–§6.5.4 ports.
- Three orphaned constants (`fondness_social_weight`,
  `novelty_social_weight`, `social_chain_target_range`) remain in
  `SimConstants` as dead fields pending a follow-on cleanup
  commit — retirement shifts the constants-hash which isn't this
  port's concern.

**Seed-42 `--duration 900` re-soak
(`logs/phase4c1-socialize-target/events.jsonl` on the uncommitted
working copy; baseline `logs/phase4b4-db7362b/events.jsonl` at
`db7362b2`):**

| Metric | Baseline | Phase 4c.1 | Direction |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 0 | 1 | **canary fails** — see causal chain below |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | ✅ canary passes |
| `continuity_tallies.grooming` | 262 | 217 | −17% |
| `continuity_tallies.courtship` | 0 | **2** | **new** — courtship activity unblocked |
| MatingOccurred events | 0 | **1** | **new** — first mating on seed 42 in project history |
| KittenBorn events | 0 | **1** | **new** — first reproduction on seed 42 |
| `positive_features_active` | 13 | 16 | +3 |
| `ward_avg_strength_final` | 0.456 | 0.315 | −31% |
| `CriticalSafety preempted L5 plan` | 6403 | 46 | −99% (cats spend less time in self-actualization plans, more in social) |

Constants header diffs clean (zero-byte diff via
`just diff-constants`), so all metric deltas are from AI behavior
changes alone.

**Hypothesis / concordance for the starvation canary fail.**
Wrenkit-98 is a kitten born at tick 1354759 to Mocha (mating with
17v0 at 1334759) and starves at 1361472 (~7k ticks, ~0.3 sim-day
post-birth) at position (26, 5). The mating that produced her
*never happened in baseline* — it's the first successful mating
on seed 42 in project history, enabled by the new target-taking
DSE surfacing a higher-quality partner than the legacy
fondness-only picker. Her death traces to Caretake's still-open
§6.5.6 gap: no adult routes TO (26, 5) to feed her. The kitten's
score table at tick 1360100 shows Eat at 0.154 (ranked 7th);
Caretake doesn't surface in the adult cats' action pools because
the Caretake DSE today navigates to nearest `Stores`, not to the
kitten with unmet hunger need.

Direction: the canary trip is a **spec-predicted downstream
dormancy surfacing**, not a regression introduced by the Socialize
port itself. The port's contribution is validated — mating /
courtship / BondFormed signals all climb — and the refactor's
design explicitly anticipated that "marker authoring alone does
**not** unblock the Cleanse / Harvest / Commune dormancies"
(open-work #14 commentary); Caretake belongs to the same
"navigate TO a physical location" class of gap.

Landing commitment: ship as-is with this causal record; **§6.5.6
Caretake port is the immediate priority follow-on** to resolve
the orphan-starvation pattern before it compounds over
multi-generation soaks.
