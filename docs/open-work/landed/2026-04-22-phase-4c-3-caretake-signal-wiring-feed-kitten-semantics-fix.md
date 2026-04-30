---
id: 2026-04-22
title: Phase 4c.3 — Caretake signal wiring + feed-kitten semantics fix
status: done
cluster: null
landed-at: null
landed-on: 2026-04-22
---

# Phase 4c.3 — Caretake signal wiring + feed-kitten semantics fix

Simplest-scope Caretake fix targeting the orphan-kitten starvation
that Phase 4c.1 / 4c.2's reproduction-enabling ports surfaced.
**Not** a §6.5.6 target-taking DSE port — this is the
pre-requisite signal wire-up + step-handler fix. A future §6.5.6
port can layer a declarative bundle over this same signal once
the Maslow-tier balance lands.

- New `src/ai/caretake_targeting.rs`:
    - `KittenState` snapshot type + `CaretakeResolution` output.
    - `resolve_caretake(adult, adult_pos, kittens)` — scans
      kittens-in-range for hunger < 0.6, returns the argmax of
      `hunger_deficit × distance_decay × kinship_boost` plus
      `is_parent` flag and winning target (Entity + Position).
      `CARETAKE_RANGE = 12` (matches §6.5.6 template), kinship
      boost 1.25× for biological parents.
    - 7 unit tests covering empty-kitten / well-fed / out-of-range
      / argmax / parent-kinship / is_parent-only-when-hungry /
      closer-kitten-ties.
- Wire the urgency signal at both scoring-caller sites
  (`disposition.rs:evaluate_dispositions` + `goap.rs:evaluate_and_plan`):
  build a kitten snapshot at the top of each tick, call
  `resolve_caretake` per adult, populate
  `hungry_kitten_urgency` + `is_parent_of_hungry_kitten` in the
  `ScoringContext` (was hardcoded `0.0` / `false`, which nulled
  the existing `CaretakeDse`'s dominant axis at weight 0.45).
  Kitten query lives in `CookingQueries` (existing bundle) for
  `evaluate_dispositions`, in `WorldStateQueries` for
  `evaluate_and_plan`.
- Rewrote `build_caretaking_chain` for physical causality: old
  chain `[MoveTo(stores), FeedKitten(stores)]` retires; new
  chain `[MoveTo(stores), RetrieveAnyFoodFromStores(stores),
  MoveTo(kitten), FeedKitten(kitten)]`. Takes the winning
  kitten from a fresh `resolve_caretake` call in
  `disposition_to_chain`'s per-cat loop.
- New `StepKind::RetrieveAnyFoodFromStores` variant + handler
  in `resolve_disposition_chains`. Wraps the existing
  `resolve_retrieve_raw_food_from_stores` helper (any food kind,
  raw/uncooked) so the Caretake chain doesn't commit to a
  specific `ItemKind` variant that might be absent from Stores.
- Fixed `resolve_feed_kitten` to actually feed the kitten:
    - Old behavior: took `target = Stores`, removed food from
      stores, credited the **adult's** `needs.social` by 0.05.
      The kitten was never fed.
    - New behavior: takes `target = kitten`, pulls food from
      adult's inventory via `Inventory::take_food()`, returns
      `(StepResult, Option<Entity>)` where the second value is
      the kitten to credit. Hunger credit (`+0.5`, capped 1.0)
      is applied in a post-loop pass to avoid a double-&mut on
      `Needs` (the cats query already owns &mut Needs over all
      non-dead cats). Adult's social bonus preserved.
- Callers at both paths (`resolve_disposition_chains` +
  `resolve_goap_plans`) updated to the new return shape. Goap
  path's FeedKitten step-state target-resolution swapped from
  nearest-Stores to `resolve_caretake`'s winning kitten.
- New `Feature::KittenFed` positive-activation signal recorded
  on each successful feeding (classified Positive in
  `system_activation.rs`; `positive_features_total` bumped 33→34
  with paired test updates).

**Seed-42 `--duration 900` re-soak
(`logs/phase4c3-caretake-wired/events.jsonl`):**

| Metric | Phase 4c.2 | Phase 4c.3 | Direction |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 5 | 1 | apparent improvement, see caveats below |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | ✅ canary passes |
| `continuity_tallies.grooming` | 274 | 268 | noise |
| `continuity_tallies.courtship` | 5 | 4 | noise |
| MatingOccurred events | 5 | 4 | noise |
| KittenBorn events | 5 | 2 | half — reproduction still noisy |
| **KittenFed events** | — | **0** | **the primary metric the fix targets — still zero** |
| BondFormed | — | 47 | climbed further |

**Unvarnished concordance — the fix is partial.** `KittenFed=0`
means no adult successfully completed a FeedKitten step in this
soak, despite the signal firing and the chain being built
correctly. Tracing the lone starvation (`Pebblekit-68`) shows
the signal propagates (kittens appear in the scoring pool with
Caretake scored at 0.24), but the `CaretakeDse`'s Maslow tier-3
classification suppresses its score heavily when the adult's own
tier-1 needs (hunger ~0.4) are unsatisfied. Maslow-gated
Caretake at 0.26 loses softmax draws against Explore (0.70+) and
Forage (0.54+) consistently. Mothers never pick Caretake over
their own Eat / Explore while hungry themselves.

The apparent Starvation improvement (5→1) is mostly
reproduction-variance: this run had 2 kittens born vs 4c.2's 5.
Run-to-run non-determinism is a pre-existing effect of Bevy's
parallel system scheduler (surfaced while debugging Phase 4c.3 —
one earlier run of the same seed produced an 8-adult-starvation
wipeout; the re-run produced the 1-kitten baseline above).
Non-determinism predates Phase 4c.3's changes — listed as an
open-work follow-on below.

**Ecological review — how feral queens actually react.**
Before proposing a balance fix I asked "how do kitten mothers
normally react when they and their kittens are both hungry?"
The literature (Nutter et al. 2004; Crowell-Davis et al. 2004;
Liberg et al. 2000; Macdonald et al. 2000; Veronesi & Fusi 2022;
Bradshaw 2012; Vitale et al. 2022 review) says **the current
Clowder wiring is ecologically correct for the feeding
decision**. A feral queen's maternal strategy is "stay alive
and lactating" — lactation roughly doubles her energy
requirement, wild felids don't regurgitate, and kittens can't
be provisioned with solid food until week 4. Her investment
channel *is* her own body condition. A hungry queen who finds
food at a patch eats first and returns to the den; milk yield
drops with her cortisol / undernutrition. Behavioral rule:
keep the queen viable; she is the bottleneck. The
Caretake-tier-3 / Eat-tier-1 priority ordering matches this.

**Where the realism gap actually is — four findings to track
as separate follow-ons:**

1. **Milk-yield / nursing-quality model, not priority
   inversion.** What breaks down under scarcity is
   *nursing quality* — milk yield scales with queen body
   condition; kittens starve from thin milk and secondary
   infection on depressed immunity, not from the queen
   choosing her stomach over theirs at the food patch.
   Model kitten hunger restoration as a function of the
   queen's recent nutrient surplus rather than a constant
   +0.5 per FeedKitten tick. Direct starvation is a
   minority cause in the literature even when kittens die
   in droves — infectious disease is ~66% of necropsied
   neonatal deaths.

2. **Alloparental care.** Feral colonies are matrilineal.
   Sisters, mother-daughter pairs, aunt-niece dyads co-den
   and **allonurse** each other's kittens; non-nursing
   queens bring prey to nursing queens. All 12 breeding
   dyads at Church Farm allonursed (Macdonald 2000). Co-
   reared kittens are left alone less, wean ~10 days
   earlier, and survive better. Prerequisite: a
   concentrated food source sufficient to support grouping.
   This is **the single most-cited feature of feral colony
   life missing from Clowder today**, and maps cleanly onto
   `docs/systems/project-vision.md` §5's sideways-broadening
   list (kin-weighted grooming + provisioning). Worth a
   dedicated system stub once Caretake stabilizes.

3. **Graded abandonment, not hard threshold.** Maternal
   collapse is a continuous drop-off: longer absences →
   reduced nursing → differential neglect of the runt →
   abandonment → (rarely) cannibalism of the non-viable
   kitten (scent removal, adaptive). Hard-threshold "queen
   abandons litter at X% body condition" would be less
   realistic than "nursing frequency + grooming of kittens
   decay smoothly with body condition; weakest kitten loses
   attention first."

4. **Male infanticide.** Unfamiliar toms entering a colony
   kill ~6.6% of litters to reset queens to estrus
   (Macdonald 2000). Distinct from maternal-care collapse;
   a separate predator-style ecological pressure if
   Clowder ever wants that mechanic.

**Baseline mortality calibration point.** Feral colonies lose
~75% of kittens before 6 months (Nutter, Levine & Stoskopf
2004; JAVMA 225:1399). Peak windows are first 2 weeks and
weaning (4–5 weeks). Leading identifiable causes: trauma
(vehicles, predators), infectious disease (URI, panleukopenia,
FeLV/FIV, parasites), congenital defects. If Clowder's
eventual kitten survival rate sits near 20–30% it's in a
realistic band; hitting 100% would be implausibly generous
and 0% is the current broken state.

**Retraction of earlier "let Caretake beat Eat" options.**
The previous three options in this entry (lower Maslow tier,
is_parent override, bump composition weights) would all push
hungry queens toward feeding kittens instead of themselves.
The literature says that's anti-realistic — it would model
cats as altruists, when they are actually metabolically
obligate and the realistic channel of investment is their own
body condition feeding lactation. Retiring those options.

**Next concrete follow-on (not yet blocking).** The highest-
realism / highest-return follow-on is **alloparental care** —
non-nursing queens bringing food to nursing mothers. That
would let a well-fed aunt feed Mocha's kittens when Mocha
herself can't. New Caretake sub-targets, a `nursing_queen`
marker, and routing food-delivery to the nursing queen rather
than directly to the kitten. Design stub belongs in
`docs/systems/` paired with the §6.5.6 target-taking DSE port
when it lands.

Sources: Nutter FB et al. *JAVMA* 2004 (n=169 kittens, feral
mortality); Crowell-Davis SL et al. *J Feline Med Surg* 2004;
Liberg O, Sandell M, Pontier D, Natoli E (in Turner & Bateson
eds. 2000); Macdonald DW, Yamaguchi N, Kerby G 2000 (farm-cat
allonursing + infanticide); Veronesi MC, Fusi J *J Feline Med
Surg* 2022; Bradshaw JWS 2012 (*The Behaviour of the Domestic
Cat* 2nd ed., ch. 8); Vitale KR et al. 2022 review of
free-ranging cat social lives. Alley Cat Allies field guides.
