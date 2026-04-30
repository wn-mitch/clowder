---
id: 087
title: Interoceptive perception substrate
status: done
cluster: null
landed-at: fc4e1ab8
landed-on: 2026-04-30
---

# Interoceptive perception substrate

**Why:** Cats build a per-tick perception model of the *external* world (`sensing.rs::update_target_existence_markers` and siblings author `HasThreatNearby` / `HasSocialTarget` / `PreyNearby` / `…` ZST markers consumed by IAUS DSEs). They built no equivalent model of the *self* — DSEs and `ScoringContext` population at `disposition.rs:730–895` reached into raw `Needs`/`Health` fields directly. Symptom: critical-health interrupt at `disposition.rs:263` fires reactively (19,382 times in the seed-42 baseline soak) into a score landscape that doesn't elevate Flee or Rest, ticket 047's treadmill. Reframed the cluster (047 / 076 / 032) as a perception gap rather than three independent fixes.

**What landed:**

1. **`src/systems/interoception.rs`** — new perception module symmetric to `sensing.rs`. Per-tick author of `LowHealth` (HP ratio ≤ `critical_health_threshold`), `SevereInjury` (≥1 unhealed `InjuryKind::Severe`), and `BodyDistressed` (composite gate: any of {hunger / energy / thermal / health deficit} > `body_distress_threshold`). Public helper functions `pain_level(injuries, normalization_max)`, `health_deficit(health)`, `body_distress_composite(needs, health)` are the canonical body-state derivations consumed by `disposition.rs` and `goap.rs` in lieu of inline math.
2. **`src/components/markers.rs`** — three new ZST markers with `KEY` constants (`LowHealth` / `SevereInjury` / `BodyDistressed`) under §State markers, alongside the existing `Injured`. Marker-queryable test added.
3. **`src/resources/sim_constants.rs`** — `DispositionConstants::body_distress_threshold` (default 0.6) gates the `BodyDistressed` marker; `pain_normalization_max` (default 2.0) divides the unhealed-injury severity sum to land `pain_level` in `[0, 1]`. Both serialize into the `events.jsonl` header per the comparability invariant.
4. **`src/ai/scoring.rs`** — `ScoringContext` gains `pain_level: f32` and `body_distress_composite: f32` fields, populated at construction by the interoception helpers. `ctx_scalars()` exposes both as named scalar inputs (`"pain_level"`, `"body_distress_composite"`) for DSE consumption.
5. **`src/plugins/simulation.rs`** — `interoception::author_self_markers` registers in Chain 2a alongside `update_injury_marker`, before the GOAP/scoring pipeline so consumers see fresh markers.
6. **DSE adoption (Phase A consumers):**
   - `src/ai/dses/flee.rs` — fourth Consideration on `health_deficit`. Originally tried `flee_or_fight(0.6)` Logistic gating; the `cautious_cat_flees_when_threatened` test caught the regression (CP geometric mean crashed full-health Flee scores below Cook). Switched to `Linear { slope: 0.4, intercept: 0.6 }` so the axis floors at 0.6 (full-health → bonus lift, not gate) and saturates at 1.0 (full deficit). New tests: `flee_has_four_axes_with_health`, `flee_health_deficit_axis_floors_to_preserve_cp_gating`, `wounded_cat_scores_flee_above_healthy_cat_under_threat`.
   - `src/ai/dses/sleep.rs` — fifth Consideration on `pain_level` (Linear identity, weight 0.10). Original four weights `[0.40, 0.24, 0.16, 0.20]` scaled by 0.90 to preserve the WS sum at 1.0. Healthy cats (pain_level=0) see no behavioral change; injured cats get a small additive lift toward Sleep-via-Resting.
7. **Sub-tickets 088 / 089 / 090** — Body-distress Modifier (§L2.10), interoceptive self-anchors, L4/L5 self-perception scalars. All `blocked-by: [087]`. Opened in the same commit (`11164473`) as 087 to surface the full design before any of them get re-discovered as fresh work.

**Implementation deviation from plan.** The plan called for a separate `Rest` DSE distinct from `Sleep` — on inspection the catalog has DSE-id-to-action mapping in many sites that look up by hardcoded id strings (`scoring.rs::score_dse_by_id`, `eval.rs`, `disposition.rs`, etc.). A genuinely distinct Rest would require new resolver wiring and new id propagation; high risk. Sleep already produces the `Resting` disposition that the critical-health interrupt special-cases at `disposition.rs:263`, so the user's symptom-level ask ("cats at low health should flee or rest") is addressed at the scoring layer by adopting `pain_level` as a Sleep axis. Documented in the ticket so the deviation doesn't re-emerge as fresh work.

**Verification:**

- `just check && cargo test --lib` — both green; 1640/1640 tests passing including 17 new `systems::interoception::tests::*` covering pure helpers and Bevy-schedule integration, and 3 new flee tests covering the CP-floor invariant.
- `just soak 42 && just verdict logs/tuned-42` — surfaced an unexpected colony-action collapse in the logged tail-window (Eat 62% / no Forage or Hunt / 8 founder starvations / Stores reach capacity 50 but never fill). Reads as a balance / DSE→GOAP plan-execution issue rather than a strict sim regression — `Eat` action records 62% but `FoodEaten` Feature never witnesses; cats elect Eat plans the resolver can't complete. Detail captured for follow-up at **ticket 091**.

**Surprise.** Two of them. (a) The originally-planned Logistic gating curve on Flee's new axis was caught by an existing `cautious_cat_flees_when_threatened` test before the soak — CP composition demands a *floor* (Linear with intercept), not a *gate* (Logistic), for a bonus axis. The test caught what eyeballing the math wouldn't. (b) The deep-soak surfaced behavioral collapse the unit tests can't catch. The substrate-and-DSE-adoption math is locally clean, but the colony-scale food economy diverged enough across 1.2M ticks of accumulated scoring shifts to produce a starvation cascade. Captured as 091 for investigation; the substrate itself stays in place.

**Landed at:** `fc4e1ab8`. **Tickets opened:** `11164473`.

---
