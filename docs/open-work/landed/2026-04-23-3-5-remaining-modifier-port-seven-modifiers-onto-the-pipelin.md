---
id: 2026-04-23
title: §3.5 remaining-modifier port — seven modifiers onto the pipeline + inline retirement
status: done
cluster: null
landed-at: null
landed-on: 2026-04-23
---

# §3.5 remaining-modifier port — seven modifiers onto the pipeline + inline retirement

Closes out the §3.5.1 catalog. The three Phase 4a
corruption-emergency modifiers (`WardCorruptionEmergency`,
`CleanseEmergency`, `SensedRotBoost`) were the first port; this
commit ports the remaining seven and retires the inline modifier
block in `src/ai/scoring.rs::score_actions` (~666–750). Translation-
scoped port — behavior-preserving by construction, gated by the
balance-methodology drift envelope.

**Seven new `ScoreModifier` impls in `src/ai/modifier.rs`:**

- `Pride` — additive × `personality.pride` on Hunt / Fight / Patrol
  / Build / Coordinate when `ctx.respect < pride_respect_threshold`.
- `IndependenceSolo` — additive × `personality.independence` on
  Explore / Wander / Hunt; no threshold.
- `IndependenceGroup` — subtractive × `personality.independence` on
  Socialize / Coordinate / Mentor, clamped ≥ 0.
- `Patience` — additive × `personality.patience` on the active
  disposition's constituent DSEs, dispatched via
  `constituent_dses_for_ordinal` keyed off
  `active_disposition_ordinal`. First modifier to route through the
  new disposition-scoped scalar surface.
- `Tradition` — flat additive bonus. **Preserves today's unfiltered
  "applies to every DSE" behavior** per the port discipline; the
  §3.5.3 item 1 filter bug is filed as **#13.7** in open-work's #13
  debt ledger (behavior change requires a hypothesis + A/B soak,
  not a translation port).
- `FoxTerritorySuppression` — multiplicative damp on Hunt / Explore
  / Forage / Patrol / Wander **plus** additive boost on Flee
  (§3.5.3 item 2). Single impl handles both the damp and the
  boost; Flee's transform is the `+ suppression × 0.5` branch.
- `CorruptionTerritorySuppression` — multiplicative damp on Explore
  / Wander / Idle.

All seven follow the established `WardCorruptionEmergency` pattern:
short-circuit on non-applicable `dse_id` per the §3.5.2
applicability matrix, read triggers via `fetch_scalar`, honor the
`score <= 0.0 ⇒ return score` contract for gated boosts (Tradition
is the spec-sanctioned exception — a flat bonus with no gate).

**New `ctx_scalars` keys** to feed modifier triggers: `respect`,
`pride`, `independence`, `patience`, `tradition_location_bonus`
(preserving today's caller-computed field), `fox_scent_level`,
`active_disposition_ordinal` (integer cast of the current
disposition for Patience's lookup).

**Registration in `default_modifier_pipeline`** — now hands out all
10 passes (3 corruption-emergency + 7 new) in retiring-inline
order, so future audits grep cleanly. Pinning order is cosmetic —
additive + multiplicative modifiers commute under the non-negative
score invariant — but makes intent readable.

**Retired:** inline post-scoring block at
`src/ai/scoring.rs::score_actions` ~666–750, plus the matching
constant-reads that were coupled to it. `ScoringContext` retains
its scalar fields (other consumers read them).

**Tests.** 23 new unit tests in `src/ai/modifier.rs` bringing the
total to 31 modifier tests. Per-impl coverage: applicability-filter
rejection (non-matching DSE ids return `score` unchanged), trigger
gate (e.g. Pride returns `score` when `respect >=
pride_respect_threshold`), transform math (additive, subtractive,
multiplicative, clamp), and the §3.5.3 item 2 Flee-boost branch on
FoxTerritorySuppression.

**Verification.** `just check` + `just test` green. Seed-42
`--duration 900` release soak hold: all four survival canaries
pass (Starvation = 0, ShadowFoxAmbush = 0, footer written,
`never_fired_expected_positives` unchanged from the pre-port
baseline). Characteristic metrics (MatingOccurred, KittenFed,
BondFormed, ScryCompleted, continuity tallies) within the ±10%
noise-band the port discipline demands — no drift exceeds the
envelope, consistent with the behavior-preserving hypothesis.

**Follow-on filed as #13.7.** Tradition's unfiltered-loop bug —
the inline loop (and this port) apply the bonus to every DSE,
not just those where the cat's history at the current tile
matches. Fixing it to the §3.5.3 item 1 (a) "structural" filter is
a behavior change. Caller sets `tradition_location_bonus` to 0.0
in production today, so the bug is muted in soak runs, but the
fix will surface when that caller starts setting non-zero values.

**Specification cross-ref:** `docs/systems/ai-substrate-refactor.md`
§3.5.1 (catalog), §3.5.2 (applicability matrix), §3.5.3
(discoveries).

---
