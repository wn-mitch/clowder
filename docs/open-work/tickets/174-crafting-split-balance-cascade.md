---
id: 174
title: Balance hypothesis for the wards-and-kittens unblock cascade (155 follow-on)
status: ready
cluster: balance
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 155 split `DispositionKind::Crafting` into Herbalism /
Witchcraft / Cooking. The post-155 seed-42 soak (`logs/tuned-42/`,
commit `e1646089`) shows large drift on output metrics:

| metric | baseline | post-155 | Δ% |
|---|---|---|---|
| `wards_placed_total` | 0 | 5 | new-nonzero |
| `kittens_born` | 0 | 6 | new-nonzero |
| `kittens_surviving` | 0 | 1 | new-nonzero |
| `bonds_formed` | 3 | 36 | +1100% |
| `health` | 0.245 | 0.688 | +181% |
| `peak_population` | 8 | 13 | +62.5% |
| `deaths_total` | 8 | 1 | -87.5% |
| `deaths_by_cause.ShadowFoxAmbush` | 8 | 0 | -100% |
| `wards_despawned_total` | 0 | 5 | new-nonzero |
| `shadow_foxes_avoided_ward_total` | 0 | 8 | new-nonzero |

Per CLAUDE.md, "a refactor that changes sim behavior is a balance
change," and drift > ±30% on a characteristic metric requires a
hypothesis with four artifacts (hypothesis · prediction · observation
· concordance — direction match + magnitude within ~2×). Multiple
metrics are well above ±30%, so this needs a structured `just
hypothesize` write-up.

## Predicted hypothesis (informal — the formal one belongs in
`docs/balance/`)

> Splitting Crafting unblocks two previously-dormant systems —
> Cooking (cooked-food happiness/hunger gains) and the Witchcraft
> ward / cleanse cascade (corruption defense, fewer ShadowFoxAmbush
> deaths) — and the colony-positive cascade follows downstream. The
> ward-system unblock prevents the mid-game ShadowFoxAmbush sink that
> kept population at 8 in the baseline; with that sink relieved, more
> cats survive long enough to bond and reproduce, hence the
> peak_population / kittens_born / bonds_formed surge.

## Plan

Run the four-artifact methodology end-to-end:

1. Frame the hypothesis in `docs/balance/155-crafting-split-cascade.md`
   with the prediction table above.
2. `just hypothesize docs/balance/155-crafting-split-cascade.yaml`
   — runs baseline + treatment sweeps and concordance check.
3. Confirm direction match on each metric and magnitude within ~2×
   per CLAUDE.md guidance.
4. If concordance fails on any metric, structure a follow-on
   investigation per the bugfix discipline.

## Out of scope

- Re-tuning constants to dampen the magnitude — substrate must
  stabilize first per CLAUDE.md substrate-refactor guidance. If the
  hypothesis confirms, the new equilibrium is the *intended* state
  of the colony; we accept it as the new baseline. If concordance
  fails, structure a balance fix before re-baselining.

## Log

- 2026-05-05: opened by ticket 155's closeout. The structural fix
  landed cleanly (`FoodCooked` off never-fired, 58% plan-failure
  reduction); the metric drift is the structural prediction firing,
  not a regression — but the magnitude needs the four-artifact
  hypothesis methodology before promoting to a new baseline.
