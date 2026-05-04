---
id: 166
title: kittens_surviving footer field has zero increment-sites — substrate-bypass shape
status: ready
cluster: ai-substrate
added: 2026-05-04
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [mentoring-extraction.md, starvation-rebalance.md]
landed-at: null
landed-on: null
---

## Why

The `colony_score.kittens_surviving` footer field is **declared and
emitted but never authored**:

- Declaration: `src/resources/colony_score.rs:54-56` —
  `pub kittens_surviving: u64` with doc-comment "Living cats that
  were born in-sim (not founding members)".
- Emit: `src/plugins/headless_io.rs:577` — written into every
  `events.jsonl` footer.
- Default: `colony_score.rs:109` `default_is_zeroed` test confirms
  initial `0` and the field never moves from default — the test would
  fail otherwise.
- Increment-sites: **zero** in `src/` (verified by
  `grep -rn "\.kittens_surviving" src/`).

Three balance specs read the field as a primary or secondary metric:

- `docs/balance/032-2-stage-multipliers.yaml:18,34` — `metric:
  kittens_surviving` is the primary gate.
- `docs/balance/032-3-breeding-floor.yaml:16` — references the
  `kittens_surviving / kittens_born` ratio.
- `docs/balance/starvation-rebalance.md:84,127` — names the same ratio
  as the right post-balance metric for kitten-mortality stratification.

This is the same substrate-bypass shape as the
`IsParentOfHungryKitten` defect that ticket 164 (originally 158)
shipped its structural fix against: a load-bearing field consumed
elsewhere with no author in `src/`. The footer always emits 0
regardless of run dynamics, silently invalidating any sweep that uses
it as a comparison metric.

Surfaced during ticket 164's closeout investigation. The 164 ticket
itself listed `kittens_surviving ≥ 3` as part of its acceptance — that
was always going to be unmeetable both because of soak truncation
(maturation = 80,000 ticks; soak runs ~54k–148k ticks at the seed-42
window) AND because the metric never increments. `mentoring-extraction.md:91`
carries an annotation "(4 of 6 matured pre-run-end; 2 starved)" that
is observationally accurate (verified against KittenMatured-style
event logs) but mislabels the row's `0` as if the metric were
working — the row needs a `[unimplemented]` tag.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Field declaration | `colony_score.rs:54-56` | `pub kittens_surviving: u64`, with `#[serde(default)]`. | `[verified-correct]` |
| Footer emit | `headless_io.rs:577` | Field is included in every footer. | `[verified-correct]` |
| Increment-on-maturation | (none — N/A) | `tick_kitten_growth` at `growth.rs:33-40` removes `KittenDependency` and records `Feature::KittenMatured` at maturity ≥ 1.0, but does NOT increment `colony_score.kittens_surviving`. | `[verified-defect]` |
| Decrement-on-death | (none — N/A) | Death-handling systems do not decrement `kittens_surviving` even for in-sim-born adults. Whether the field is "living kittens-or-adults born in-sim" or "living matured-adults born in-sim" is unspecified. | `[suspect — needs spec]` |
| Documentation | `colony_score.rs:54` doc-comment | Says "Living cats that were born in-sim (not founding members)" — implies a *living count*, not a cumulative count. Suggests both increment AND decrement paths needed. | `[verified-correct]` |

## Fix candidates

**Parameter-level:** N/A — there's no parameter to tune; the field
needs an author.

**Structural:**

- R1 (**extend**) — author the field in two existing systems: extend
  `tick_kitten_growth` (`growth.rs:35-40`) to also `score.kittens_surviving
  += 1` at the same spot it records `Feature::KittenMatured`; extend
  the death-handling system that already decrements live-population
  counters to also decrement `kittens_surviving` when the dying cat
  was in-sim-born. Choice: spec the field as "living adults born
  in-sim" (post-maturation only). Fits the existing doc-comment and
  matches what the three balance specs assume.
- R2 (**extend** with a different spec) — author from birth onward:
  increment in the kitten-spawn path (`fertility.rs` or wherever
  `KittenDependency` is added on birth); decrement on any in-sim-born
  cat's death. Spec: "living kittens-or-adults born in-sim".
  Pro: matches the 15-min soak's typical truncation (kittens that
  haven't matured still count). Con: changes the semantics the three
  balance specs assume, which would invalidate their predictions.
- R3 (**retire**) — remove the field from the footer entirely; remove
  references from the three balance specs; replace with the existing
  `Feature::KittenMatured` activation count (which IS authored and
  shows up in the activation block). Pro: simpler; the activation
  block already gives us the data. Con: breaks footer-key
  cross-run continuity for any baseline that recorded
  `kittens_surviving` in its committed copy.

## Recommended direction

R1 is the lowest-risk path: it preserves footer-key stability, matches
the existing doc-comment, and matches what the three balance specs
already assume. Author it at the existing increment-point
(`tick_kitten_growth` at maturity transition) and at the existing
decrement-point (death-handling system, gated on "was in-sim-born").

The "was in-sim-born" gate needs a marker or a check on whether the
cat ever had `KittenDependency` — verify what exists today before
inventing a new marker.

Also fix the `mentoring-extraction.md:91` annotation to clarify "0"
is `[unimplemented metric — KittenMatured activation count is the
working proxy]`, OR — once R1 lands — re-derive the row's numbers.

## Out of scope

- **Fixing post-d1722a33 kitten attrition.** Owned by ticket 165.
- **Auditing other footer fields for the same shape.** Ticket 160
  (substrate-stub catalogue) already exists for the broader pattern;
  this ticket is just one entry surfaced into its own bucket because
  three balance specs depend on it.

## Verification

- After R1 lands: `just soak 42` (or any soak) emits
  `kittens_surviving > 0` whenever any in-sim-born cat is alive.
- `just sweep-stats` on a re-run of `032-2-stage-multipliers.yaml`
  produces non-zero per-rep `kittens_surviving` values (currently they
  all read 0).
- `default_is_zeroed` test at `colony_score.rs:109` still passes.
- `mentoring-extraction.md:91` row is updated with real (non-zero) numbers.

## Log

- 2026-05-04: opened. Surfaced during ticket 164's closeout
  investigation when the `kittens_surviving ≥ 3` acceptance criterion
  proved structurally unmeetable (soak truncation AND zero
  increment-sites). Same substrate-bypass shape as the
  `IsParentOfHungryKitten` defect 164 fixed.
