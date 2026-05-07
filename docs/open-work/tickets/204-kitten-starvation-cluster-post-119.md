---
id: 204
title: Kitten-starvation cluster surfaced by 119 substrate activation — bonding/mating cascade vs adult-feeding bandwidth
status: ready
cluster: ai-substrate
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The 119 substrate-activation soak (`logs/tuned-42-119-verify`) and the 108 follow-on soak (`logs/tuned-42`) both exhibit the same regression: 2 kitten starvations late in a 15-min seed-42 run (Wrenkit-85 @ tick 1309110 + Wispkit-78 @ tick 1312127). The 108 soak is bit-identical to 119-verify on macro outcomes — same kittens die at same ticks, same locations — confirming that 108 inherited but did not introduce the cluster.

Causal chain (preliminary): the 119/047 substrate cluster removed the `CriticalHealth` interrupt and lifted Flee/Sleep on `health_deficit > 0.4`, which retired the per-tick yank that was previously stripping cats out of bonding/socializing into anxiety states. Net effect on seed-42:

- `anxiety_interrupt_total: 43017 → 0` (full retirement)
- `bonds_formed: 3 → 34` (+1033%)
- `kittens_born: 0 → 3` (new behavior — no births in pre-119 baseline)
- `kittens_surviving: 0` (all 3 born die)
- `deaths_by_cause.Starvation: 0 → 2` (both kittens)
- `deaths_by_cause.ShadowFoxAmbush: 8 → 1` (cats survive wildlife better, but the 1 ambush death may have been a parent)
- `shadow_fox_spawn_total: 16 → 27` (+69%; coincident pressure)
- `continuity_tallies.burial: 0` (canary collapse — burials require an adult cat to be free to dig; competing demands dropped this to zero)

This is a *new* behavioral surface: the colony is now actively reproducing where it never did before, and the adult-feeding bandwidth doesn't yet keep up with kitten-survival demand under increased fox-spawn pressure.

## Scope

**Investigation phase first.** Don't pre-commit to a fix until the layer-walk is done. The structural-revision candidates worth drafting (per CLAUDE.md §Bugfix discipline):

- **split** — give kitten-feeding its own `DispositionKind` separate from generic Caretaking so the planner can prioritize hungry-own-kitten over peer Caretake.
- **extend** — keep `Caretaking` umbrella, branch the plan template on `is_parent_of_hungry_kitten` so a parent's kitten-feeding completion proxy is shorter than a peer-caretake's.
- **rebind** — change Action → Disposition mapping so `FeedKitten` routes through `Caretaking` directly without going through the ambient feed-from-stores chain that competes with mate-following.
- **retire** — drop a competing modifier or DSE that's pulling parents away from kittens (e.g., if the new bonding cascade keeps parents glued to their mate's tile, a `ParentingSatiation` damp on Mating could break the loop).

## Verification

- Layer-walk audit on the chain `parent_marker_active → caretake_resolution.urgency → CaretakeDse score → FeedKitten plan → step resolution`. Mark each row `[verified-correct]` or `[suspect]` per `_template_bugfix.md`.
- Focal-trace one of the dying kittens (Wrenkit-85 or Wispkit-78) and trace what its parent was doing at the kitten's hunger-decay window. CaretakeDse score? Was Mating winning? Was the parent on a long Hunt plan?
- `just q cat-timeline logs/tuned-42 Wrenkit-85` for the existing soak's narrative trace.
- After fix: paired-soak comparison vs `logs/tuned-42-119-verify` confirming starvations drop to 0 without regressing the bonding/mating cascade (don't fix the symptom by re-introducing the anxiety interrupts).

## Out of scope

- Re-introducing the legacy `CriticalHealth` / `CriticalSafety` interrupts. The substrate path is the doctrine; this regression doesn't justify reverting.
- Burial canary collapse — separate concern (burial requires an adult to be free for the rite; if all adults are kitten-feeding or mating, burial drops to 0). May resolve as a side effect of fixing the kitten-feeding bandwidth issue, or may need its own ticket.

## Log

- 2026-05-07: Opened in the same commit that lands 108 (Phase A activation). 108 itself is bit-identical to 119 on this seed; the cluster is 119's surfacing, observed in two independent soaks.
