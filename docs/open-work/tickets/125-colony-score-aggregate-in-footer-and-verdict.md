---
id: 125
title: Surface `ColonyScore.aggregate` in the footer and add numerical-delta gating to `just verdict`
status: ready
cluster: tooling
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

`ColonyScore` already computes a single scalar — `aggregate = welfare × max(1, seasons_survived) + achievement_points + positive_activation_score` — and emits it on every `ColonyScore` event in `events.jsonl`. The aggregate is a deterministic function of (seed, constants, commit), so two runs with matching `events.jsonl` headers produce identical aggregates; the variance shape is well-understood (welfare × integer-season clamping makes the metric jumpy near season boundaries, but flat between).

Two gaps prevent `just verdict` from using it today:

1. **Aggregate is not in the `_footer` line.** `verdict` reads `_footer.*` as the comparison surface. To use aggregate, verdict would either tail the events.jsonl for the last `ColonyScore` event (extra IO, schema-fragile) or the footer needs the field (substrate-correct).
2. **Verdict has no numerical-delta channel.** Today's gate is binary (canaries pass/fail) + categorical (footer drift > ±10% surfaces a concern). A continuous metric like aggregate wants its own bucket: small drift → pass, mid drift → concern, large drift → fail-or-hypothesis-required.

Mining the trajectory across the last 25 seed-42 soaks (logs/tuned-42-*) shows aggregate is **a useful lens that's currently invisible to gating tooling**: best-ever 1330 (post-085), 091 cliff to ~330, 092 partial recovery to ~740, plateau 800–1050 across recent landed work. None of this is reflected in any verdict output today.

## Substrate-over-override pattern

Verdict's existing canaries are **policy gates** (zero starvation, ≤10 ambush, ≥1 each of seven continuity canaries). Aggregate is **a continuous health signal** — a different lens, complementary not replacement. The doctrinal mistake to avoid: treating aggregate-up as proof of correctness. A bug that spams a positive feature, an inflated bond count from a relationship-init regression, or a shelter computation that double-counts dens would all raise aggregate while breaking the sim. Hard canaries stay hard.

## Scope

Two commits.

### Commit 1 — Footer field

Extend the `_footer` JSONL line written at sim end to include a `colony_score` block populated from the last `emit_colony_score` snapshot:

```json
"colony_score": {
  "aggregate": 1034.10,
  "welfare": 0.525,
  "shelter": 0.625,
  "nourishment": 0.900,
  "health": 1.000,
  "happiness": 0.600,
  "fulfillment": 0.376,
  "seasons_survived": 5,
  "peak_population": 8,
  "kittens_born": 0,
  "kittens_surviving": 0,
  "structures_built": 8,
  "bonds_formed": 3,
  "deaths_starvation": 0,
  "deaths_old_age": 0,
  "deaths_injury": 0
}
```

The five welfare axes plus the cumulative ledger are what give the aggregate diagnostic value — when aggregate moves, the axis-level breakdown names which dimension shifted. Without it, "score went down" is opaque.

**Implementation surface.** The footer write site lives in the events.jsonl writer's drop / flush path; track it via `grep -n '"_footer"' src/`. The colony score's last snapshot is reachable via `Res<ColonyScore>` + a fresh recomputation of welfare axes (or by remembering the last emitted snapshot — pick whichever matches the existing `emit_colony_score` shape).

### Commit 2 — Verdict numerical-delta channel

Extend `just verdict` so when a baseline is set (`logs/baselines/current.json`), the report includes:

```json
"colony_score_drift": {
  "aggregate": { "this": 1034.10, "baseline": 997.48, "delta_pct": +3.7 },
  "welfare":   { "this": 0.525, "baseline": 0.339, "delta_pct": +54.9 },
  "axes": { "shelter": +12.3, "nourishment": +2.1, ... }
}
```

Bucketing:

- `|delta_pct| ≤ 5`: pass channel, no surface in `next_steps`.
- `5 < |delta_pct| ≤ 15`: concern. `next_steps` names the moved axis and asks for a hypothesis if intentional.
- `|delta_pct| > 15`: fail (or concern + hypothesis-cited). Same shape as the existing footer-drift bucket.

Aggregate-only gating is *not* enough to fail a run on its own (canaries still gate hard). But aggregate moving 30% with all canaries green is exactly the case the existing tooling silently misses today — that's the gap this closes.

### Bisect-canary integration (optional, this ticket or follow-on)

`just bisect-canary aggregate` becomes a meaningful invocation: same shape as the existing footer-field bisect, just reading `_footer.colony_score.aggregate` (or any axis path) instead of the top-level footer fields. Decide at unblock-time whether to include in this ticket or open as 125b.

## Sequencing

Blocked on 097 because 097 lands per-species substrate audits that may surface their own footer-shape questions; doing 125 first risks landing a footer schema that 097's audits would want to extend. Land 097 first, then 125 with a clean schema.

## Reproduction / verification

```
just check
cargo test --lib                    # all green; new tests for the footer field + verdict delta logic
just soak 42
just verdict logs/tuned-42 --baseline logs/baselines/current.json
```

The verdict output for the seed-42 deep-soak should now include `colony_score_drift` with the cross-baseline numerical comparison. Hand-eyeball against the trajectory recorded in this ticket's §Why (best-ever 1330; recent plateau 800–1050) — the metric is well-calibrated when delta vs baseline names a known regression.

**Backfill check:** running verdict against `logs/tuned-42-085-v1-strict` (best ever) vs current `logs/baselines/current.json` should report a meaningfully positive aggregate — proves the metric direction is right (higher = more colony).

## Out of scope

- **Replacing the canaries with aggregate.** Hard survival canaries stay where they are. Aggregate is additive.
- **Re-defining `aggregate`'s formula.** `welfare × max(1, seasons) + achievement_points + positive_activation_score` is the existing computation. Tuning weights or adding/removing terms is a separate balance question.
- **Adding aggregate to per-tick instrumentation overlays.** The existing `ColonyScore` events already carry it; UI exposure is a separate concern.

## Log

- 2026-05-01: Opened as a follow-on to 096's landing soak. The user proposed using `aggregate` as the verdict gate ("we can ensure that they will guaranteed be controls against each other"). Counter-proposal recorded above: keep canaries as hard gates, add aggregate as a continuous-delta channel. Blocked on 097 to avoid double-landing the footer schema.
