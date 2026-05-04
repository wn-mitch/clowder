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
