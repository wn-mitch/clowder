---
id: 167
title: Action::Groom split — examples + groom.ron asset follow-on
status: done
cluster: ai-substrate
added: 2026-05-04
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 3c522740
landed-on: 2026-05-05
---

## Why

The 158 landing (commit d1722a33 — "feat: 158 — split Action::Groom +
extract DispositionKind::Grooming") introduced `Action::GroomSelf` and
`Action::GroomOther` but did not update three call sites that still
reference `Action::Groom`:

- `examples/template_audit.rs:32` — `ALL_ACTIONS` array literal of
  size 18.
- `examples/template_prompt.rs:54` — `match action { Action::Groom => "groom.ron", … }`.
- `examples/template_prompt.rs:349` — `match rng.random_range(0..18) { … 7 => Action::Groom, … }`.

`cargo check --all-targets` (and therefore `just check`) fails with
three `E0599 no variant or associated item named Groom found` errors.
The lib alone compiles cleanly; this is examples-only.

The asset side has the same shape: `assets/narrative/groom.ron` is a
single template file. The Action split should propagate to a
`groom_self.ron` / `groom_other.ron` pair (or an explicit decision to
keep one shared file with both Action variants mapping to it).

Surfaced 2026-05-04 during ticket 166's verification pass when
`just check` failed on a tree with no 166-related changes to examples
or assets.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Action enum | `src/ai/mod.rs:45,53` | `Action::GroomSelf` and `Action::GroomOther` exist as separate variants. Old `Action::Groom` is gone. | `[verified-correct]` |
| Modifier registry | `src/ai/modifier.rs:2933-2934` | Both new variants are addressed via `GROOM_SELF` / `GROOM_OTHER` constants. | `[verified-correct]` |
| Audit example list | `examples/template_audit.rs:24-43` | `ALL_ACTIONS` enumerates the canonical action surface for template-coverage audits. Length-18 array literal stamped on the type. | `[verified-defect]` |
| Random-pick example | `examples/template_prompt.rs:340-360` | `pick_action` indexes 0..18 → Action variants for the prompt-generation debug tool. | `[verified-defect]` |
| Template asset | `assets/narrative/groom.ron` | Single file — pre-split. Decision needed: split into two files OR keep one file referenced by both Action variants. | `[suspect — needs spec]` |

## Fix candidates

**Parameter-level:** N/A — there's no parameter to tune.

**Structural:**

- R1 (**rebind** — keep one asset, two Action variants point at it) —
  the simplest fix: in `template_prompt.rs` map both `Action::GroomSelf`
  and `Action::GroomOther` to `"groom.ron"`; in `template_audit.rs`
  add `Action::GroomSelf` and `Action::GroomOther` to `ALL_ACTIONS`,
  bumping the array length from 18 → 19. The asset stays as-is.
  Pro: minimal churn, matches "Mate / Caretake → socialize.ron"
  precedent already in `template_prompt.rs:65-66`. Con: narrative
  templates can't differentiate self-grooming from allogrooming
  flavor-wise.
- R2 (**split** — two assets, one per variant) — author
  `groom_self.ron` and `groom_other.ron`. Map each Action to its own
  file. Pro: matches the structural intent of the 158 split (Mentoring
  / Eating precedent both have their own templates). Con: requires
  authoring template content; the existing `groom.ron` would need to
  be partitioned by hand.
- R3 (**retire** — drop the audit's `ALL_ACTIONS` enumeration entirely
  and derive from `Action::iter()` if available) — eliminates the
  drift class. Pro: future-proof. Con: depends on whether `Action`
  derives a sequence helper (likely doesn't today).

## Recommended direction

R1 first as a mechanical unblock so `just check` passes. R2 can land
later as a narrative-asset improvement when someone is touching the
narrative templates anyway. R3 is the right long-term answer but
requires `Action` to grow an iterator or `strum::EnumIter`.

## Out of scope

- Anything other than the three example references and the narrative
  asset for the Action::Groom split.
- The broader narrative-template refactor (own ticket if surfaced).

## Verification

- `cargo check --all-targets` succeeds.
- `just check` passes its `cargo check --all-targets` step.
- `cargo run --example template_audit` runs and reports both Groom
  variants in its coverage table.

## Log

- 2026-05-04: opened. Surfaced during ticket 166 (kittens_surviving
  wiring) verification when `just check` failed on a working tree
  with no 166-related changes to examples or assets.
- 2026-05-05: landed at 3c522740 — R1 (rebind) shipped: both Groom
  variants point at the shared `assets/narrative/groom.ron`. While
  unblocking, restructured `pick_action` from
  integer-match-with-wildcard to a slice-index lookup over a
  `PICKABLE_ACTIONS` const, and added compile-time exhaustiveness
  witnesses (`assert_pick_pool_covers_action`,
  `assert_all_actions_covers_action`) so a future Action variant must
  be classified at the author site instead of being silently absorbed
  by `_`. R2 (split groom.ron into per-variant assets) and R3 (retire
  `ALL_ACTIONS` via `strum::EnumIter`) remain follow-ons if narrative
  authors want flavor-divergence later.
