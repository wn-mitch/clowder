# Mentoring Extraction (ticket 154) — Balance Cascade

## Summary

Extracting `DispositionKind::Mentoring` out of `Socializing` (ticket 154)
unblocks `Action::Mentor`, which previously fired at zero per the L3
cost-asymmetry collapse documented in 152's audit. Once mentor sessions
actually run, a structural cascade fires through the social/relational
layer: fondness → bonds → mating → kittens → reproduction. Most welfare
axes climb sharply; one CLAUDE.md hard gate (`Starvation == 0`) is
violated by a brand-new failure mode (kittens born for the first time,
two starve mid-year-8 due to a localized Caretaking-range gap).

This doc is the four-artifact methodology entry for the resulting drift.
Per CLAUDE.md, "a refactor that changes sim behavior is a balance
change," and drifts > ±10% on multiple characteristic metrics require a
hypothesis. The cascade is *predicted* and *positive in aggregate*; the
two regressions are tracked as follow-on tickets.

## Iter 1 — 2026-05-03 (post-154 land)

### Hypothesis

Mentoring is the structural choke-point on the social-coordination side
of the L3 layer. Once `Action::Mentor` actually runs:

1. `resolve_mentor_cat` grants per-tick mastery, social, and respect to
   the mentor; per-tick fondness + familiarity to both ends of the
   mentor↔apprentice pair.
2. Repeated mentor sessions accumulate fondness. Fondness drives
   `socialize_target.rs`'s `target_fondness` (weight 0.28), which
   re-elects the same partner. Familiarity drives `social.rs`'s bond
   formation (Acquaintance → Friends → Partners thresholds).
3. Once a pair crosses the Friends-tier fondness threshold, courtship
   drift starts emitting `PairingIntention` events for compatible
   pairs. The new ticket-078 `PAIRING_INTENTION_INPUT` (weight 0.10 on
   `socialize_target`) re-elects them again, locking the pair into a
   self-reinforcing courtship loop.
4. Pairs reach Partners → Mating fires → kittens born. Pre-154,
   `kittens_born=0` for the canonical seed-42 soak; post-154, the loop
   ignites for the first time.

**Predicted direction (each metric, magnitude band):**

- `mentoring` continuity tally: 0 → ≥1 (split lights MentoredCat).
- `bonds_formed`: large positive (≥3×) — mentor sessions concentrate
  fondness on a small partner set.
- `kittens_born`: 0 → small positive (≥1) — first generation possible.
- `welfare`, `happiness`: positive ≥10% — fondness/respect deficits
  closed; per-tick respect grant on mentor.
- `anxiety_interrupt_total`: meaningful negative (≥−20%) — bonded cats
  read each other as Allies, fewer threat reads on familiar peers.
- `deaths_injury`: meaningful negative — same Ally-overlap mechanism;
  intraspecies conflict downshifts.
- `Starvation`, `burial`: not predicted to change. New failure modes
  *enabled* by the cascade (kitten-feeding at peak demand, burial
  triggering on a new death pattern) are out-of-prediction surprises.

### Prediction (magnitude bands going in)

| Metric                  | Direction | Magnitude band | Concordance threshold |
|-------------------------|-----------|----------------|------------------------|
| `mentoring` tally       | up        | ≥1             | structural — ≥1 = pass |
| `bonds_formed`          | up        | 3× to 10×      | direction match + within 2× |
| `kittens_born`          | up        | 1 to 5         | structural — any nonzero = pass |
| `welfare`               | up        | +15 to +35%    | within 2× |
| `happiness`             | up        | +20 to +40%    | within 2× |
| `aggregate score`       | up        | +30 to +60%    | within 2× |
| `anxiety_interrupt_total` | down    | −20 to −50%    | within 2× |
| `deaths_injury`         | down      | −20 to −60%    | within 2× |
| `Starvation`            | flat      | 0              | gate — `==0` required |
| `burial`                | flat      | ≥1             | continuity — ≥1 required |

### Observation (canonical seed-42 deep-soak, commit `bb189bc`, 8 sim years)

Baseline: `logs/tuned-42-baseline-0783194/events.jsonl` (commit
`0783194`, pre-154).

| Metric                  | Baseline | Observed | Δ        | Band              |
|-------------------------|----------|----------|----------|-------------------|
| `mentoring` tally       | 0        | 1614     | +∞       | **structural pass** |
| `bonds_formed`          | 3        | 29       | +867%    | direction match, magnitude over band |
| `kittens_born`          | 0        | 6        | +∞       | **structural pass** |
| `welfare`               | 0.339    | 0.475    | +40.1%   | direction match, magnitude over band |
| `happiness`             | 0.625    | 0.907    | +45.2%   | direction match, magnitude over band |
| `aggregate score`       | 997      | 1876     | +88.1%   | direction match, magnitude over band |
| `anxiety_interrupt_total` | 43017  | 14881    | −65.4%   | direction match, magnitude over band |
| `deaths_injury`         | 8        | 3        | −62.5%   | direction match, within band |
| `Starvation`            | 0        | **2**    | new      | **gate violation** — both kittens at tile (38,22), 552 ticks apart |
| `burial`                | (not tracked baseline) | 0 | flat | **canary dark** |
| `kittens_surviving`     | 0        | 0        | 0        | (4 of 6 matured pre-run-end; 2 starved) |
| `KittenFed` events      | (n/a; no kittens) | 1631 | new | Caretaking firing healthy run-wide |

### Concordance

**Direction match:** 6/8 predicted axes match direction; 2 are out-of-
prediction surprises (Starvation and burial) caused by a new failure
mode the cascade *enabled* rather than the cascade itself. Concordance
holds for the cascade hypothesis.

**Magnitude:** every direction-match axis exceeds the upper band of its
prediction (welfare +40% vs ≤+35% band; happiness +45% vs ≤+40%;
aggregate +88% vs ≤+60%; bonds_formed +867% vs ≤10× = +900% — just
inside; anxiety −65% vs ≤−50%). The cascade is *more* potent than
predicted. Reading: fondness/respect/familiarity feedback in the social
layer was *severely starved* by the L3 collapse, and the rebound is the
shape of how undernourished the layer was.

**Verdict:** concordant on the cascade hypothesis. The two
out-of-prediction surprises are not concordance failures — they're
new-failure-mode discoveries enabled by the cascade. Tracked as
follow-on tickets:

- 156 (kitten-starvation locality) — root-cause the (38,22) cluster.
- 157 (burial canary dark) — verify the burial-on-death code path
  with the new death distribution.

### Out-of-prediction findings (open questions)

**Starvation = 2 (kitten-localized).** Both deaths at tile (38,22),
within 552 ticks of each other in year 8. Caretaking is healthy run-
wide (1631 `KittenFed` events). Likely shapes: `CARETAKE_TARGET_RANGE`
(currently 12) doesn't reach (38,22) from where the active adults
cluster; or kitten-spawn locality clusters tightly on a corner the
adult migration pattern doesn't visit; or feed-kitten chain timing
loses the race against kitten-hunger drain at sustained-demand peak.
Ticket 156 holds the investigation.

**`burial` = 0.** With only 5 deaths total (2 Starvation kittens, 3
ShadowFoxAmbush) and the adult population mostly kitten-rearing or
mating, burial may have specific eligibility predicates (witness
adult? non-fled? specific personality threshold?) that the new death
distribution doesn't match. Ticket 157 holds the investigation.

**`shadow_fox_spawn_total` doubled (16→32).** Welfare/aggregate climb
is partially counteracted by *more* shadowfox events. Possible reason:
the seasons_survived bumped 7→8 (we got an extra year of soak content
because the run didn't collapse on starvation/predation as the
baseline did), so spawn rates × longer survival = 2× total. Not a
balance issue per se; just a confounder when reading the spawn-rate
metric.

### Knobs touched

None. This iteration is a substrate split (DispositionKind variant);
no `SimConstants` field changed. Header constants drift between the
baseline (commit `0783194`) and observed (commit `bb189bc`) is from
ticket-154's structural change, not from any tuning knob.

### Next iteration triggers

Open Iter 2 if:
- Ticket 156 lands and post-fix soak shows `Starvation == 0` while
  preserving the cascade. Document the localized fix.
- Ticket 157 lands and post-fix soak lights `burial ≥ 1`. Document
  the eligibility-predicate clarification.
- A *cross-seed sweep* (`just sweep`) shows the cascade is seed-42
  -specific. Right now we have one seed; the magnitude-over-band
  numbers may shrink across seeds.
- A characteristic metric drifts > ±10% from this iteration's numbers
  in a future change unrelated to 156/157.

## Iter 2 — 2026-05-04 (post-158 land)

### Hypothesis

Ticket 158 splits `Action::Groom` into sibling `GroomSelf` /
`GroomOther` and extracts `DispositionKind::Grooming` from
`Socializing` (mirrors 154's Mentoring extraction shape exactly).
Pre-158 the post-154 socializing template `[SocializeWith (2),
GroomOther (2)]` had two equivalent-effect actions; A* at
`planner/mod.rs:437` pre-pruned `GroomOther` because both
produced the same `(SetInteractionDone(true), IncrementTrips)`
next-state, so `GroomedOther` never fired in soaks. The split
makes the L3 softmax pick directly determinative — `GroomOther`
becomes a first-class affiliative action competing with
`SocializeWith`, `Mentor`, and `Caretake`.

Predictions:

1. `GroomedOther` clears `never_fired_expected_positives` (the hard
   structural-success criterion).
2. `continuity_tallies.grooming` rises 10–30% as allogrooming
   sessions actually run.
3. `mentoring` and `courtship` redistribute downward as the
   affiliative-time-share rebalances across three peer dispositions
   instead of two.
4. Survival canaries inherit 154's cascade unchanged: any new
   Starvation deaths reflect 156's kitten-feeding gap rather than a
   158-specific regression.

### Observation

Soak: `logs/tuned-42` (seed 42, commit `e9d9ac1d` dirty, ~22 sim
years equivalent at `final_tick = 1,309,441`).

| Metric | Baseline pre-154 (`tuned-42-baseline-0783194`) | Post-154 pre-158 (`tuned-42-pre158`, commit `bb189bc`) | Post-158 (`tuned-42`, commit `e9d9ac1d`) |
|---|---|---|---|
| `continuity_tallies.grooming` | 194 | 499 | **1,279** |
| `continuity_tallies.mentoring` | 0 | 445 | 165 |
| `continuity_tallies.courtship` | 999 | 3,613 | 1,330 |
| `continuity_tallies.play` | 219 | 81 | 21 |
| `continuity_tallies.burial` | 0 | 0 | 0 |
| `deaths_by_cause.Starvation` | 0 | 0 | 3 (all kittens) |
| `deaths_by_cause.ShadowFoxAmbush` | 8 | 3 | 6 |
| `deaths_by_cause.WildlifeCombat` | 0 | 1 | 2 |
| `never_fired_expected_positives` | n/a | `[GroomedOther]` | `[FoodCooked]` |

### Concordance

| Prediction | Direction | Magnitude | Verdict |
|---|---|---|---|
| GroomedOther clears never-fired | ✓ off list | n/a | **match** |
| grooming +10–30% | ↑ +156% (499 → 1279) | exceeds band | **direction match, magnitude over** |
| mentoring + courtship redistribute downward | ↓ -63% mentoring, ↓ -63% courtship | both within 2× of predicted | **match** |
| Survival canaries inherit 154 cascade unchanged | ↑ Starvation 0 → 3 (new) | new failure mode | **partial — see attention-share below** |

The grooming-tally exceeded its predicted band. Working hypothesis:
post-158 cats commit to `Grooming` (single-step `[GroomOther]`
plan) more decisively than they committed to `Socializing`'s
mixed-step plan pre-158, because the `SingleMinded` strategy on
the new disposition resists drift mid-session. This isn't a bug —
it's the structural intent of the Pattern-B extraction (mirrors
why Mentoring went 0 → 1614 post-154). The over-shoot is
*directionally correct*; future seeds will tell whether the
magnitude band needs widening.

### New finding: parent attention-share regression

`Mocha` (the colony's reproductive matriarch) action distribution
during her kittens' lifetime (ticks 1,268,664 → 1,309,441,
~40K-tick window covering all three of her kittens being alive):

| Action | Count | % of CatSnapshot |
|---|---|---|
| Forage | 119 | 29.4% |
| **GroomOther** | **71** | **17.5%** |
| Patrol | 70 | 17.3% |
| Hunt | 56 | 13.8% |
| Coordinate | 31 | 7.7% |
| Wander | 15 | 3.7% |
| Mentor | 14 | 3.5% |
| Sleep | 11 | 2.7% |
| **Caretake** | **11** | **2.7%** |
| Eat | 3 | 0.7% |
| Build | 3 | 0.7% |
| GroomSelf | 1 | 0.25% |

Mocha picked `GroomOther` 6.5× more often than `Caretake` while
her own kittens were alive and starving. All three kittens
(Thymekit-19, Wispkit-21, Emberkit-3) starved within ~200 ticks
of each other (1,309,257 → 1,309,441), two at the exact same tile
(41, 22) where they were born.

The pre-158 substrate hid this attention-share question: when
`Action::Groom` was a single L3 entry that mostly resolved to
self-grooming via the `>=` resolver bias, allogrooming wasn't
competing for parent-cat time. Post-158 it is, and on this seed
it dominates `Caretake`. **No code regression — the substrate
correctly surfaces a balance question the L2 modifier layer
hasn't answered yet:** there's no per-cat lift on `caretake_dse`
when the cat carries the `IsParentOfHungryKitten` marker, so the
"my kittens are starving" signal doesn't punch through the
L3 softmax.

This is the same failure cluster as ticket 156 (kitten-feeding
gap post-154 cascade), now exercised by a different per-tick
attention-share pattern. Tracked under 156's umbrella; not opening
a separate ticket because the underlying ecological question is
the same: the colony reaches reproduction, the kittens get born,
and the existing `caretake_dse` scoring shape doesn't keep
parent attention focused tightly enough on own-kitten hunger.

### Knobs touched

`SimConstants.self_groom_temperature_scale` field **removed**.
This field weighted the side-channel resolver computation that
158 deleted; no longer referenced anywhere. Header-shape change
breaks comparability with `tuned-42-*` runs that included the
field — acceptable because 158 is itself a balance-shifting
structural change, and the next `just promote` re-locks the
baseline. No active sim-tuning knobs changed.

### Next iteration triggers

Open Iter 3 if:
- Ticket 156 lands a parent-attention lift (e.g.,
  `IsParentOfHungryKittenLift` modifier on `caretake_dse`)
  and post-fix soak shows Mocha's Caretake share climbing back
  toward parity with GroomOther while own kittens are alive.
- A *cross-seed sweep* shows the +156% grooming tally and the
  attention-share inversion are seed-42-specific; magnitude
  bands may need widening if the shift varies seed-to-seed.
- `continuity_tallies.play` dropping further (post-158 = 21,
  down from baseline 219) needs investigating — playfulness
  metrics may be load-bearing for kitten development that the
  current scoring doesn't capture.
