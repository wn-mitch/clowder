---
id: 469
title: Ground JointIntention emission in mutual perception (confidence + candidacy)
status: done
cluster: social-coordination
orchestration: substrate-sensitive
initiative: [mythic-texture]
added: 2026-05-26
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-26
---

## Why

Today the `JointIntention` matchmaker in `src/ai/joint_intention.rs::author_joint_intentions` is a 3rd-party fiat — it scans pairs and inserts JIs on local eligibility (orientation + bond + reproductive for Courtship; playfulness + mood + co-presence for PlayBout per ticket 276). Neither cat "perceives" the other before the JI exists. This is substrate-shaped wrong: real shared practice is the *shadow of mutual perception*, not a verdict delivered to two cats from outside. People don't idly think "someone else is doing this thing with me" — the belief crystallizes from observed cues (gaze meeting, play-bow + reciprocal play-bow, approach + non-flee, sustained orientation).

The narrative consequence is visible in the 276 Commit-A soak: 8247 PlayBout emissions in 60k ticks, 8247 drops, ~12 completions (0.14%). Most JIs die at Approach via `PartnerLeftPractice` cascade — cat A picks B, B picks C, neither holds the other. The drop gate fires correctly; the matchmaker keeps re-pairing the wrong way because emission has no grounding in either cat's perception. Tuning the matchmaker (mutual best-match bidirectionality, bond weighting, score thresholds) is a stopgap. The substrate-correct fix is a confidence dimension on the practice plus a mutual-candidacy gate that crystallizes the JI only when both cats have independently registered the other as a practice partner.

This composes with — does not duplicate — the related JointIntention follow-ons:
- **279** scopes the body-cue *source* (compose 127 with 242/243 — what observable signals feed candidacy).
- **280** scopes the mental-model *belief-holder* (compose 127 with 258 C3 — where the partner-belief lives in the cat's head).
- **469** (this) scopes the *strength dimension and emission gate* — JI carries a confidence scalar, matchmaker emits when both cats hold mutual candidacy markers (not before).

## Scope

- Add `confidence: f32` to `src/components/joint_intention.rs::JointIntention`. Per the module rustdoc's "field discipline" section: confidence IS observable practice-state (visible in posture, sustained orientation, mounting tolerance, persistent proximity). Starts low at emission; strengthens via observed mutual interactions; drops on missing signal.
- Add `PracticeCandidate { practice, target, since_tick }` marker Component. Authored by a candidacy-author pass that runs *before* the matchmaker emission. Each cat independently registers candidates it has perceived as engagement-eligible — without the cross-cat coupling the current matchmaker imposes.
- Rebind matchmaker emission: a JI emits only when both cats hold reciprocal `PracticeCandidate { practice, target=other }` markers (mutual perception confirmed). The current local-eligibility predicate becomes the predicate for *candidacy* marker insertion, not JI insertion.
- Add `JointDropBranch::ConfidenceCollapsed` and wire it into `should_drop_joint`. Fires when `confidence < confidence_floor` for `confidence_decay_window_ticks` consecutive ticks.
- Bias-reader integration: `joint_bias_multiplier` scales bias by `confidence` (low-confidence JIs get minimal lift; established JIs get the full multiplier). The current `bias_multiplier` becomes the asymptote, not the immediate value.
- Per-practice tuning constants for the confidence curve (initial value, strengthen rate per observed interaction, decay rate per silent tick, floor) — one block in `PracticeConstants` per practice that already exists (Courtship, PlayBout).

## Out of scope

- The actual source of candidacy signal beyond what's wired today. The first cut uses current eligibility predicates (Courtship: bond + fertility + orientation; PlayBout: playfulness + mood + co-presence) as the candidacy-marker trigger. Body-cue grounding (compose with 242/243) is **279**'s scope. Confidence strengthening *via observed body cues* is also 279's scope — until 279 lands, confidence strengthens on existing `JointInteractionObserved` ticks and tick-elapsed-in-practice.
- Mental-model representation of the partner's JI (where the belief about partner-state lives in the cat's head). **280**'s scope.
- Refactoring Courtship's matchmaker to fix its theoretical asymmetric-pairing risk. Courtship today works around the gap via narrow eligibility (orientation + reproductive + Friends-bonded reduces the candidate set to typically 0-1 cats per actor); this ticket adds the substrate but doesn't change Courtship's behavior in healthy colonies. PlayBout (276) is the consumer where the substrate gap manifests in soak data.
- Reworking the per-practice eligibility predicates themselves. This ticket lifts the gating mechanism; tuning the predicates is per-practice work.

## Current state

276 Commit A landed the PlayBout matchmaker that surfaced the substrate gap most clearly (8247 emit / 8247 drop / ~12 complete in seed-42 15-min soak). 127 (the foundational JointIntention substrate) is landed and stable for Courtship. 279 / 280 are template-stub tickets — body-cue and mental-model angles are scoped but not authored. 469 is the third angle on the same substrate shape and lands independently (no blocked-by; lands without requiring 279/280, and 279/280 compose with it cleanly when they land).

## Approach

**Two-pass author system.** Split `author_joint_intentions` into:
1. **Candidacy pass** — for each cat, scan local-eligibility against nearby cats and insert/refresh `PracticeCandidate { practice, target, since_tick }` markers. Per-practice predicate (Courtship: orientation + bond + reproductive; PlayBout: playfulness + mood + co-presence). Markers age out after `candidacy_expiry_ticks` of no refresh — captures "the cat noticed the other but lost interest / line-of-sight."
2. **Emission pass** — for each cat lacking a JI, emit a JI only when (a) self holds `PracticeCandidate { target=B }`, (b) B holds `PracticeCandidate { target=self, practice=same }`. Mutual confirmation. JI starts at low confidence (`confidence_initial`).

**Confidence dynamics.** Mirror the existing per-tick `last_interaction_tick` substrate:
- Each tick a bias-amplified interaction fires (resolver target == JI partner), confidence accrues `confidence_per_interaction`.
- Each tick without interaction, confidence decays by `confidence_decay_per_tick`.
- Confidence is clamped to `[0, 1]`. At `confidence ≥ 1.0`, the practice is at full belief; bias multiplier reaches its asymptote.
- `should_drop_joint` adds a `ConfidenceCollapsed` branch with precedence between `CompatibilityLost` and `StageStalled`.

**Bias reader integration.** `joint_bias_multiplier` returns `(1.0 + (bias_multiplier - 1.0) * confidence, …)` so low-confidence JIs get near-zero lift, full-belief JIs get the full Courtship/PlayBout multiplier. This preserves today's behavior for established Courtship pairs (which reach full confidence quickly via repeated grooming/socialize interactions) while preventing the fresh PlayBout matchmaker from emitting full-bias JIs on cats that haven't actually engaged.

**Stage-progression integration.** PlayBout's tick-elapsed stage gates (276 Commit A) become *confidence-gated* — Approach → Bouting fires when `confidence > approach_confidence_floor` AND `ticks_in_stage > approach_min_ticks` (whichever is later). The matchmaker emits at near-zero confidence; the practice doesn't actually progress until both cats have observed enough mutual interaction to build belief. This is the substrate-honest replacement for the current "30 ticks elapsed and we declare you're playing" gate.

## Layer-walk audit

To be filled in when the ticket is picked up. Initial framing:

| Layer | File / line | Fact | Status |
|---|---|---|---|
| Matchmaker emission | `src/ai/joint_intention.rs:302-450` (Pass 3) | Emits JIs on local eligibility without mutual confirmation | `[suspect]` |
| Drop gate | `src/components/joint_intention.rs:458-516` | `PartnerLeftPractice` fires when partner JI shape mismatches; ConfidenceCollapsed branch missing | `[suspect]` |
| Bias reader | `src/components/joint_intention.rs:327-363` | `joint_bias_multiplier` returns the full multiplier on any matching JI; no confidence scaling | `[suspect]` |
| Stage progression | `src/components/joint_intention.rs:551+` | PlayBout uses tick-elapsed gates; no belief-strength gate | `[suspect]` |
| Substrate field discipline | `src/components/joint_intention.rs:22-28` | Module rustdoc says "only observables" — confidence IS observable (posture, mounting tolerance, sustained orientation) so it qualifies | `[suspect]` |

## Structural-option menu

- **split** — separate `PracticeCandidate` and `JointIntention` Components (candidacy markers vs in-practice substrate). **Chosen for the candidacy substrate** — keeps the JointIntention Component shape simple while adding the new perception layer.
- **extend** — add `confidence: f32` to existing `JointIntention`. **Chosen for the strength dimension** — the rustdoc says only observables belong on JI, and confidence IS observable practice-state (sustained orientation, posture, etc.). Extending the existing Component keeps the field-discipline invariant.
- **rebind** — change matchmaker emission to gate on mutual candidacy. **Chosen** — the emission predicate moves from "local eligibility" to "mutual candidacy confirmed." Local eligibility predicates remain, but they trigger candidacy markers, not JI emission.
- **retire** — N/A.

## Verification

- `just check && just test` clean.
- `just soak-trace 42 Simba` followed by `just verdict`: hard gates hold (Starvation 0, ShadowFox ≤10, all continuity canaries ≥1).
- PlayBout matchmaker emission count drops by ≥80% (target: from 8247 → ≤1600 per 60k-tick soak) — the mutual-candidacy gate filters asymmetric pairings before JI emission.
- PlayBout completion rate (completions / emissions) rises by ≥10x (target: from 0.14% → ≥1.4%).
- Courtship behavior is preserved bit-for-bit (frame-diff socialize / grooming / courtship / mentoring within ±5% of pre-469 baseline). Courtship pairs reach full confidence quickly because their per-tick interaction rate is already high.
- Focal-cat trace shows confidence progression: emission near zero → strengthens over Bouting-stage interactions → reaches `confidence_initial + strengthen_rate * bouting_duration_ticks` at Cooldown entry → decays through Cooldown → drops on Completed.
- New unit tests in `src/components/joint_intention.rs`: candidacy mutual-confirmation predicate; confidence accrual / decay arithmetic; ConfidenceCollapsed drop branch fires below floor; bias-reader confidence scaling.

## Related work

<!-- linkages:start -->
- · **279** (ready, social-coordination) — Body-cue-driven joint adoption (compose 127 with 242 + 243) — sibling angle: source of candidacy signal.
- · **280** (ready, social-coordination) — Mental model of partner JointIntention (compose 127 with 258 C3 mental models) — sibling angle: where the partner-belief lives.
- · **276** (in-progress, social-coordination) — Play-bout practice on JointIntention substrate — the consumer that surfaced the substrate gap most clearly.
- · **127** (done) — JointIntention substrate (codified body language; mutually-public practice state).
- · **258** (done) — C3 worked design — subjective belief substrate (mental models + facets).
- · **242** / **243** (blocked, belief-perception) — Behavior-observation L1 channel (target-side body-cue + physical marker substrate).
<!-- linkages:end -->

## Log

- 2026-05-26: opened from 276 Commit-A soak observation. The PlayBout matchmaker's 8247-emit / 12-complete churn is a symptom; the substrate gap is the absence of mutual-perception grounding on JointIntention emission. Will Mitchell flagged the narrative correctness issue ("people don't idly think 'someone else is doing this thing with me'") — substrate must encode mutual perception before practice membership can be authored.
- 2026-05-26: retired after substrate audit against docs/systems/ai-substrate-refactor.md §4.7 / §7.M.4 / §12.3 / §12.4. The proposed `confidence: f32` on `JointIntention` fails the JI rustdoc field-discipline test (src/components/joint_intention.rs:22-28) — its dynamics are per-cat asymmetric belief (each cat tallies its own bias-amplified interactions), not mutually-public practice-state, so the spec-honest home is `MentalModel<Cat>.perceived_intent_clarity` (landed in 258 at src/components/beliefs.rs:148, with sibling facet `perceived_receptivity`). The proposed `PracticeCandidate { practice, target, since_tick }` marker duplicates the same facet. The `ConfidenceCollapsed` drop branch duplicates §12.3's `still_goal` proxy, which §7.M.4 already names for L2 `PairingActivity` ("Partner still present; partner in sensory range; bond ≥ Partners"); §12.4 is explicit that Activity-shaped Intentions don't need CI1 — achievement is termination, not a world-predicate. Auto-memory anchor: feedback_model_perception_as_beliefs.md. The matchmaker-mutual-perception-gate deliverable is now 280's scope (read MentalModel<other>.perceived_intent_clarity to gate JI emission); the upstream witnessable-event wiring for play-bow, sustained-orientation, and reciprocal-advance is 279's scope. Both ticket bodies reshape from template stubs to substantive scope in this same retirement pass. PlayBout matchmaker churn (8247 emit / 12 complete / 0.14% per 60k-tick seed-42 soak) is now tracked through 279 → 280 — not 469.
