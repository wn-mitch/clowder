---
id: 280
title: Mental model of partner JointIntention (compose 127 with 258 C3 mental models)
status: ready
cluster: social-coordination
orchestration: substrate-sensitive
initiative: [full-sensory-perception]
added: 2026-05-11
parked: null
blocked-by: [279]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Today's `JointIntention` matchmaker in `src/ai/joint_intention.rs::author_joint_intentions` emits practice substrate on local-eligibility predicates (Courtship: orientation + bond + reproductive; PlayBout: playfulness + mood + co-presence) without grounding emission in either cat's perception of the other. The JI module rustdoc names this gap explicitly: *"Real cats perceive each other's persistent practice-engagement through scent, posture, mounting tolerance, repeated proximity. We don't have those perception channels yet, so the substrate stands in."* Ticket 258 (landed 2026-05-11) closed the substrate gap by authoring `MentalModel<Cat>` at `src/components/beliefs.rs:146` with `perceived_intent_clarity` and `perceived_receptivity` facets, updated from `WitnessableEvent` observations via `belief_integrator`. 280 is the consumer that wires those facets into the matchmaker emission gate.

The PlayBout matchmaker churn surfaces this most clearly: 276 Commit A's seed-42 60k-tick soak emitted 8247 JIs and dropped 8247 with only ~12 completions (0.14%). Cat A picks B, B picks C, neither holds the other; the drop gate fires correctly while the matchmaker keeps re-pairing the wrong way because emission ignores whether either cat has actually perceived the other as a partner. The substrate-correct fix is to gate emission on mutual belief: both cats must hold a sufficiently-clear model of the other as engagement-eligible before the JI crystallizes.

This composes with — does not duplicate — the related JointIntention follow-ons. **279** wires the upstream `WitnessableEvent` variants (play-bow, sustained-orientation, reciprocal-advance) that drive `perceived_intent_clarity` accrual for PlayBout-relevant cues; without 279, the PlayBout matchmaker rebind has no evidence source to gate on. **127** is the foundational substrate (mutually-public practice state). **258** is the belief substrate this ticket reads. Subsumes the retired ticket 469's "mutual-candidacy gate" deliverable — see 469's Log for the substrate-shape audit that retired it.

## Scope

- **Matchmaker rebind: read `MentalModel<other>` to gate emission.** In `pick_courtship_partner` (`src/ai/joint_intention.rs:501`) and `pick_playbout_partner` (`src/ai/joint_intention.rs:637`), in addition to the existing local-eligibility predicates, require:
  - `self.CatBeliefs.models[other].perceived_intent_clarity > emission_intent_clarity_floor`
  - `other.CatBeliefs.models[self].perceived_intent_clarity > emission_intent_clarity_floor`
  - For affiliative practices (both Courtship and PlayBout qualify): additionally `self.CatBeliefs.models[other].perceived_receptivity > emission_receptivity_floor` (symmetric).
- **Per-practice tunables in `PracticeConstants`** (`src/resources/sim_constants.rs`) — add `emission_intent_clarity_floor` and `emission_receptivity_floor` to both `CourtshipPracticeConstants` and `PlayBoutPracticeConstants`. Courtship's floors can be lenient (Courtship today works around the perception gap via narrow eligibility — Friends-bonded + orientation + reproductive reduces the candidate set to typically 0–1; the rebind is structurally correct but unlikely to drop completion rate). PlayBout's floors are the load-bearing tuning surface — the 8247→≤1600 emission target depends on them.
- **No new field on `JointIntention`.** §12.3's `still_goal` proxy plus the existing belief-integrator decay-toward-prior already cover the "low-belief drop" semantics that retired 469's `ConfidenceCollapsed` branch was reaching for. If a per-practice low-belief drop floor is wanted, it composes as a read of `MentalModel<partner>.perceived_intent_clarity` in `should_drop_joint`, not as a new mechanism.
- **No new marker.** `PracticeCandidate` (proposed in 469) is duplicate to `MentalModel<Cat>.perceived_intent_clarity` per the 469 retirement audit.
- **Bias-reader does not change.** `joint_bias_multiplier` (`src/components/joint_intention.rs:327-363`) keeps its current shape; bias scales by the existing per-practice `bias_multiplier` constant. The perception-grounding fix is at emission, not at scoring.

## Out of scope

- Adding new `WitnessableEvent` variants for body cues. **279**'s scope.
- Refactoring `JointIntention`'s field shape. The Component stays unchanged.
- Tuning per-practice `bias_multiplier` magnitudes. Balance-thread work; orthogonal to this rebind.
- Reworking per-practice local-eligibility predicates (`is_reproductive_for_courtship`, `is_playbout_eligible`). The rebind composes mutual-belief AND local-eligibility; both predicates remain.
- Body-zone integrity gates on Courtship (deferred per §7.M.1 to the Body Zones epic).

## Current state

- **258 landed 2026-05-11.** `MentalModel<Cat>` with seven facets, `belief_integrator` system, `WitnessableEvent` consumer path. `perceived_intent_clarity` and `perceived_receptivity` accrue from `Groom` / `Mate` / `Care` / `FleeFrom` / `Attack` / `Hunt` variants today.
- **279 stub.** Body needs to land before 280's PlayBout rebind has the evidence-source it requires (play-bow / sustained-orientation / reciprocal-advance variants). Courtship's matchmaker rebind may be testable before 279 lands because Courtship interactions already emit `Groom` and `Mate` witnessable events; verify in 280's investigation phase.
- **276 Commit A landed 2026-05-26.** PlayBout substrate exists; matchmaker emits 8247/60k ticks; Commit B (retire direct-emit + Bouting-stage cascade) is in flight under the current matchmaker shape. The matchmaker rebind itself moves to this ticket.
- **469 retired 2026-05-26** as substrate-duplicating. See landed/469's Log for the audit. The "mutual-candidacy gate" deliverable migrated here.

## Approach

**Pre-flight (layer-walk audit).** Before any code change, promote the matchmaker rows in the substrate audit:
- `pick_courtship_partner` local-eligibility predicate at `src/ai/joint_intention.rs:501` — `[verified-correct]` (matches §7.M.1 Layer 2 firing conditions).
- `pick_playbout_partner` local-eligibility predicate at `src/ai/joint_intention.rs:637` — `[verified-correct]` (playfulness + mood + Socialize/Idle/Wander matches 276's PlayBout-ethology spec).
- Absence of mutual-belief read in either matchmaker — `[verified-defect]` (the rebind target).
- `MentalModel<Cat>.perceived_intent_clarity` accrual on Courtship-adjacent cues today (`Groom`, `Mate`) — `[verified-correct]` per `belief_integrator` (`src/systems/belief_integrator.rs`).
- `MentalModel<Cat>.perceived_intent_clarity` accrual on PlayBout-adjacent cues today — `[verified-defect]` (no play-bow / orientation / reciprocal-advance witnessable variants emitted; blocked on 279).

**Implementation steps:**

1. Add `emission_intent_clarity_floor: f32` and `emission_receptivity_floor: f32` to `CourtshipPracticeConstants` and `PlayBoutPracticeConstants` in `src/resources/sim_constants.rs`. Default values: Courtship `intent=0.15` / `receptivity=0.10` (lenient — Courtship's narrow eligibility already filters); PlayBout `intent=0.30` / `receptivity=0.20` (load-bearing — drives the 80% emission-count reduction).
2. Extend `pick_courtship_partner` and `pick_playbout_partner` signatures to take a `&CatBeliefs` map (or query for it). Add the mutual-belief check after the existing local-eligibility filter but before the score-by-quality comparator.
3. Update unit tests in `src/ai/joint_intention.rs` to seed `MentalModel` entries for matchmaker-eligible pairs.
4. Run `just check && just test`.

**Verification before soak.** Use `just scenario` against a two-cat preset (post-279 wiring): seed both cats' `MentalModel` of the other to `perceived_intent_clarity > 0.30`, confirm matchmaker emits; zero out one side's facet, confirm matchmaker silences.

## Verification

- `just check && just test` clean.
- `just soak-trace 42 Simba && just verdict` — hard gates hold (Starvation 0, ShadowFox ≤10, all continuity canaries ≥1).
- PlayBout matchmaker emission count drops by ≥80% (target ≤1600 emits per 60k-tick soak vs 8247 pre-280 baseline).
- PlayBout completion rate (completions / emissions) rises by ≥10× (target ≥1.4% vs 0.14% pre-280 baseline).
- Courtship behavior preserved bit-for-bit: frame-diff socialize / grooming / courtship / mentoring within ±5% of pre-280 baseline. Courtship's narrow eligibility (Friends+ bond + reproductive + orientation) and existing `Groom`/`Mate` witnessable accrual mean established pairs reach the lenient floor quickly.
- Focal-cat trace shows `MentalModel<partner>.perceived_intent_clarity` is read at matchmaker emission decisions (instrument via a one-line scenario-only debug print or trace-row in `joint_intention.rs` if helpful; remove before merge).
- New unit tests: matchmaker silences when self-belief below floor; matchmaker silences when partner-belief below floor; matchmaker emits when both above floor.

## Related work

<!-- linkages:start -->
- · **279** (ready, social-coordination) — Body-cue-driven joint adoption (compose 127 with 242 + 243) — upstream evidence source; **blocks this ticket** for the PlayBout rebind.
- · **276** (in-progress, social-coordination) — Play-bout practice on JointIntention substrate — the consumer whose Commit-A churn surfaced the substrate gap that 280 closes.
- · **127** (done) — JointIntention substrate (mutually-public practice state).
- · **258** (done) — C3 worked design — subjective belief substrate (MentalModel + facets + evidence typology). The substrate this ticket reads.
- · **469** (retired 2026-05-26) — Ground JointIntention emission in mutual perception — retired as substrate-duplicating; its deliverable folded into this ticket.
<!-- linkages:end -->

## Log

- 2026-05-11: opened as joint-adoption mental-model composition stub.
- 2026-05-19: accuracy audit — ticket body incomplete (template boilerplate with no substantive content). [needs-review] on scope/approach/verification sections before work commences.
- 2026-05-26: body reshape after 469 retirement audit. Scope folded in 469's "mutual-candidacy gate" deliverable (substrate-correct shape: read `MentalModel<partner>.perceived_intent_clarity` rather than author a new marker or JI field). Added `blocked-by: [279]` because PlayBout-rebind needs 279's play-bow / orientation / reciprocal-advance `WitnessableEvent` variants to drive belief accrual. Courtship-side rebind may be testable independently; confirm in investigation.
