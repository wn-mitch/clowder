---
id: 2026-04-22
title: "Phase 4c.2 — §6.5.2 `Mate` target-taking DSE port"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-22
---

# Phase 4c.2 — §6.5.2 `Mate` target-taking DSE port

Second per-DSE §6 port. Closes the §6.2 silent-divergence between
`disposition.rs::build_mating_chain`
(`romantic + fondness - 0.05 × dist` mixer with inline
Partners/Mates bond filter) and `goap.rs::resolve_goap_plans::MateWith`
(`find_social_target` — fondness-only, **no bond filter**) by
routing both through a single `TargetTakingDse` evaluator.

The goap silent divergence was the more dangerous of the two: it
let the MateWith step target a non-partner cat once the Mate
disposition won selection upstream (since `find_social_target`
didn't check bond). The port closes that gap.

- New `src/ai/dses/mate_target.rs` with:
    - `mate_target_dse()` factory — three per-§6.5.2
      considerations (`target_nearness` Logistic(20, 0.5),
      `target_romantic` Linear(1,0), `target_fondness`
      Linear(1,0)) composed via WeightedSum with renormalized
      weights `[0.1875, 0.5, 0.3125]` (spec weights 0.15 / 0.40 /
      0.25 divided by 0.80 to drop the blocked fertility-window
      axis). Fertility-window (§6.5.2 row 4) deferred until
      §7.M.7.5's phase→scalar signal mapping lands (Enumeration
      Debt).
    - `resolve_mate_target(...) -> Option<Entity>` caller-side
      helper — filters candidates by bond (`Partners` | `Mates`
      only) before scoring, matching `build_mating_chain`'s
      current eligibility semantics. Candidate-pool range is
      `MATE_TARGET_RANGE = 10.0` (matches social-range) to admit
      nearby Partners into the scoring pool; the Logistic
      distance curve decays sharply from adjacency.
    - `Intention::Activity { kind: ActivityKind::Pairing, ... }`
      factory threads winning partner forward.
    - 8 unit tests — factory shape (id / axes / weights), plus
      resolver (missing DSE / non-bonded filter / Partners pick /
      romantic-over-fondness tiebreak / Pairing-Intention shape).
- Registration: `mate_target_dse()` pushed into
  `target_taking_dses` at both mirror sites
  (`plugins/simulation.rs` + `main.rs::build_new_world`).
- Caller cutovers:
    1. `systems/disposition.rs` `disposition_to_chain` —
       `build_mating_chain` signature shrinks from
       `(entity, pos, personality, cat_positions, relationships, d)`
       to `(mate_target: Option<Entity>, cat_positions)`.
       Inline `romantic + fondness - 0.05 × dist` mixer with
       bond filter retires; pre-resolved partner consumed
       directly.
    2. `systems/goap.rs` `resolve_goap_plans::MateWith` —
       `find_social_target(...)` call replaced by
       `resolve_mate_target(...)`. Bond filter now applied at
       the goap path for the first time; closes the
       more-dangerous half of the silent divergence.

**Seed-42 `--duration 900` re-soak
(`logs/phase4c2-mate-target/events.jsonl`; baseline
`logs/phase4c1-socialize-target/events.jsonl` at Phase 4c.1 HEAD):**

| Metric | Baseline (4c.1) | Phase 4c.2 | Direction |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 1 | **5** | **canary fails** — all 5 are kittens (see below) |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | ✅ canary passes |
| `continuity_tallies.grooming` | 217 | 274 | +26% |
| `continuity_tallies.courtship` | 2 | **5** | +150% (courtship activity climbs) |
| MatingOccurred events | 1 | **5** | **+4 (3 pregnancies, 5 kittens across 2 twin + 1 singleton litters)** |
| KittenBorn events | 1 | **5** | **+4 (every kitten died)** |
| `positive_features_active` | 16 | 16 | flat |
| `CriticalSafety preempted L5 plan` | 46 | 6 | −87% (continued shift away from L5 plans) |
| `TendCrops: no target for Tend` | — | 386 | new plan-failure surface |
| `ward_avg_strength_final` | 0.315 | 0.304 | noise |

Constants header diffs clean (zero-byte via
`just diff-constants`). All metric deltas are AI-behavior only.

**Hypothesis / concordance — canary fail is the Caretake gap
compounding.** All 5 starvation deaths are kittens:
`Wispkit-45`, `Fernkit-15`, `Thistlekit-65`, `Cricketkit-39`,
`Pipkit-69`. None existed in the Phase 4c.1 baseline soak. They
are newborns from three Mate-port-enabled pregnancies (Mocha × 2
litters of twins + Mallow × 1 singleton that died as
`Wispkit-45`). The orphan-starve pattern is identical to Phase
4c.1's lone Wrenkit-98 case: kitten born → no adult fires
Caretake → kitten starves in ~60 snapshots. Root cause traced:
`hungry_kitten_urgency` is hardcoded `0.0` in both scoring
caller paths (`disposition.rs:640` + `goap.rs:937`), so the
existing Caretake DSE's dominant axis (weight 0.45) never
contributes and Caretake never wins action selection.

The Mate port is **correctly enabling reproduction** — romantic +
fondness scoring with the bond filter surfaces higher-quality
partner selection than the legacy `find_social_target`. The
canary trip is the Caretake dormancy from Phase 4c.1's landing
record amplified 5× by reproduction actually happening now.

**Caretake is now BLOCKING** further per-DSE ports — see the
Outstanding section for the priority-upgrade rationale. Every
additional port that boosts prosocial behavior will compound
kitten mortality against the hard-gate canary.
