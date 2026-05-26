---
id: 276
title: Play-bout practice on JointIntention substrate (play continuity canary host)
status: ready
cluster: social-coordination
orchestration: substrate-sensitive
initiative: [mythic-texture]
added: 2026-05-11
parked: null
blocked-by: []
supersedes: [415]
related-systems: [ai-substrate-refactor.md]
related-balance: [healthy-colony.md]
landed-at: null
landed-on: null
---

## Why

The `play` continuity canary is gated by a four-condition AND × probabilistic
roll at `src/systems/personality_events.rs:80-90`:

```rust
if current.action == Action::Socialize
    && personality.playfulness > 0.6
    && mood.valence > 0.0
{
    let chance = personality.playfulness * 0.1;
    if rng.rng.random::<f32>() < chance {
        commands.trigger(PlayInitiated { cat: entity });
    }
}
```

Play fires iff (a) Socialize wins L3 softmax this tick, (b) the cat's
personality.playfulness exceeds 0.6, (c) mood.valence is positive, and (d)
an RNG roll capped at ~10% succeeds. Pre-066 the canary ran at ~348
events/soak; post-066 it collapsed to 8–14 with intermittent zeros. A
2026-05-25 audit of the last 10 seed-42 soaks confirms range 0–13 across
healthy-colony runs while grooming / mentoring / courtship remain at
800–1800+ events each. The 066 four-fix did not regress the emit path; it
surfaced a shape that was always knife-edge — any shift in any of the four
factors collapses the multiplicative product toward zero.

Per design pillar #2 (substrate over hacks), this ticket retires the
direct-emit hack in favor of a JointIntention-backed `PlayBout` practice.
Hosting the canary on substrate makes "playing together" mutually-public
practice-state (the JointIntention semantic category, per ticket 127),
co-extensive with the sister practices 274 (co-mentoring) and 275 (joint
cache-stocking). The canary becomes a side-effect of a practice's
Cooldown→drop transition rather than a parasitic emit on Socialize's
softmax win.

Supersedes ticket 415 (PlayFired gate orthogonal to Explore). 415's
layer-walk identified R5 (retire) as the structural option; this ticket
executes it.

## Scope

- Add `Practice::PlayBout` variant to the `Practice` enum in
  `src/components/joint_intention.rs` (sibling to `Courtship`).
- Add `JointStage::PlayBoutApproach / PlayBoutBouting / PlayBoutCooldown`
  variants. Stages carry observable-only fields (no internal heart-state),
  per the JointIntention field discipline (module rustdoc lines 1-50).
- Wire matchmaker eligibility for PlayBout in
  `src/ai/joint_intention.rs::author_joint_intentions`. Eligibility:
  both cats `playfulness > 0.6`, both `mood.valence > 0`, neither bound by
  a competing JointIntention, co-presence within scoring range, current
  action is `Socialize` or `Idle` (light-bandwidth coexistence).
- Author an HTN method `PlayBout` in the method registry, mirroring
  `src/ai/methods/courtship.rs`. The method drives stage progression and
  composes the step templates each stage requires. Per CLAUDE.md, naked
  aspiration Components without method-registry entries are forbidden;
  PlayBout lands active (not `PendingSubstrate`), so no `wires-method` glue
  ticket is needed.
- Add `EventKind::JointPlayBoutCompleted { actor, partner }` and increment
  `continuity_tallies["play"]` from its `record` arm in
  `src/resources/event_log.rs`. The existing `EventKind::PlayFired`
  increment at line 802 stays during migration so both paths feed the same
  tally key.
- Add `JointDropBranch::PlayBoutCompleted` as a carry-over branch for
  Cooldown→done; reuse the existing drop-cascade plumbing.
- After two consecutive seed-42 deep-soaks show play ≥ 10 via the
  JointIntention path, retire the direct-emit:
  `personality_events.rs:80-90`, `EventKind::PlayFired`, `PlayInitiated`,
  and the `on_play_initiated` observer. Migrate the observer's mood-lift
  + narrative cascade onto the `JointPlayBoutCompleted` arm or a
  Bouting-stage step resolver — whichever places the cascade closer to its
  causal trigger.
- Re-classify `Feature::*` enrolment in
  `src/resources/system_activation.rs::expected_to_fire_per_soak` if the
  retire pass changes which Feature variants exist.

## Out of scope

- Sister practices 274 (co-mentoring) and 275 (joint cache-stocking). Same
  JointIntention family, same substrate authoring pattern, but they don't
  share the play canary contract. Cross-referenced under Related work; do
  not bundle into this commit.
- Restoring play to the pre-066 magnitude (~348 events/soak). The canary
  contract is ≥1 named event per sim year; the realistic post-substrate
  target is the pre-066-stable range (50–150), not the four-AND × RNG
  amplification of the old shape. If the migrated canary lands below the
  pre-fragility floor, follow-on tuning is a separate ticket.
- Re-examining whether play *should* be a continuity canary at all (ticket
  445 demoted mythic-texture on similar reasoning). If the JointIntention
  path produces play counts that suggest the canary threshold is wrong,
  open the demotion conversation in a follow-on.

## Current state

Direct emit is live at `src/systems/personality_events.rs:88` (trigger:
`PlayInitiated`); observer `on_play_initiated` at line 266 emits
`EventKind::PlayFired` at line 320, which increments
`continuity_tallies["play"]` at `src/resources/event_log.rs:802`.
JointIntention substrate exists — `src/components/joint_intention.rs` is
1065 lines, `src/ai/joint_intention.rs` is 939 lines — but the only
authored practice is `Courtship`. Sister tickets 274 (co-mentoring) and
275 (cache-stocking) are template-stub frontmatter from 2026-05-11; this
ticket is the first to author body.

## Approach

Mirror the Courtship pattern end-to-end:

**Practice variant + stages.** Extend `Practice` and `JointStage` enums in
`src/components/joint_intention.rs`. Keep field discipline — observables
only. Approach = both cats within N tiles + co-orienting; Bouting =
co-located in a playful action sequence (witnessed by the bouting step
resolver); Cooldown = bout finished, mood-lift applied, drop pending.

**Matchmaker.** Extend `author_joint_intentions` in
`src/ai/joint_intention.rs` with a `PlayBout` eligibility predicate. Cap
concurrent play-bouts colony-wide if early soaks show oversaturation
(matchmaker bandwidth competition with Courtship is a known risk).

**HTN method.** Add `src/ai/methods/play_bout.rs` and register it in
`src/ai/methods/mod.rs`, mirroring `courtship.rs`. The method composes
step templates that drive the Approach→Bouting→Cooldown progression.

**Canary increment.** Add `EventKind::JointPlayBoutCompleted` and its
`record` arm in `event_log.rs` alongside the existing `MatingOccurred` /
`CourtshipInitiated` arms. Both `JointPlayBoutCompleted` and `PlayFired`
feed `continuity_tallies["play"]` during migration.

**Retire direct-emit.** After two clean soaks at play ≥ 10 via the
JointIntention path, delete `personality_events.rs:80-90` and remove
`PlayInitiated` / `EventKind::PlayFired` / the observer. Migrate the
mood-lift cascade onto the Cooldown completion path. Verify cascade
semantics preserved via narrative-log inspection.

## Layer-walk audit

Substrate to retire (direct-emit):

| Layer | File / line | Fact | Status |
|---|---|---|---|
| Trigger | `personality_events.rs:82-89` | 4-cond AND gate × RNG·0.1 chance per Socialize-tick | `[verified-fragile]` |
| Cascade observer | `personality_events.rs:266+` (on_play_initiated) | mood-lift to nearby cats; narrative entry; canary increment | `[verified-correct]` |
| Tally site | `event_log.rs:801-803` | `EventKind::PlayFired` → `continuity_tallies["play"] += 1` | `[verified-correct]` |
| Feature enrolment | `system_activation.rs:315` | `Feature::Socialized` is the per-soak canary; play is `continuity_tallies`-driven, not Feature-driven | `[verified-correct]` |

Substrate to author (JointIntention-backed):

| Layer | File / line | Required addition | Notes |
|---|---|---|---|
| Practice enum | `src/components/joint_intention.rs` | `Practice::PlayBout` variant | mirror Courtship |
| Stage enum | `src/components/joint_intention.rs` | `JointStage::PlayBoutApproach / Bouting / Cooldown` | observables only |
| Matchmaker | `src/ai/joint_intention.rs::author_joint_intentions` | eligibility predicate for PlayBout | co-presence + playful + good mood + light-bandwidth current action |
| HTN method | `src/ai/methods/play_bout.rs` (new) + `mod.rs` registration | drives stage progression | mirror `courtship.rs` |
| Drop branch | `JointDropBranch` in `src/components/joint_intention.rs` | `PlayBoutCompleted` carry-over branch | drop on Cooldown→done |
| Tally site | `src/resources/event_log.rs` | `EventKind::JointPlayBoutCompleted` arm → `continuity_tallies["play"]` | both paths feed same key during migration |

## Structural-option menu

Per CLAUDE.md "Bugfix discipline" — the structural options were considered
and `retire` chosen:

- **split** — give Play its own DSE+Action, score as a peer to Socialize.
  *Rejected.* Moves the four-AND gate into a DSE eligibility filter rather
  than retiring it; doesn't realize pillar #3 (richer perception, better
  strategy) because the gate is still parasitic on cat-state coincidence.
- **extend** — relax the AND gate (e.g., fire on Idle + co-presence too).
  *Rejected.* Tuning-shaped patch on a structural fragility; the
  multiplicative AND of independent factors remains.
- **rebind** — re-link the emit trigger to a different Action winning at
  L3. *Rejected.* Same shape problem; changes which softmax win the
  canary parasitizes.
- **retire** — delete the direct-emit; host the canary on a JointIntention
  PlayBout practice. **Chosen.** Substrate-side lever per pillar #2;
  composes with the existing JointIntention substrate; codifies "playing
  together" as publicly-performed practice state rather than an internal
  mood/personality coincidence; aligns with the sister practices 274/275
  that also need authoring.

## Verification

- `just check && just test` clean after each commit.
- After substrate lands but before retiring direct-emit: `just soak-trace
  42 Simba` followed by `just verdict <run-dir>` — pass; play canary ≥1
  (hard gate), ideally ≥10. Confirm `JointPlayBoutCompleted` events appear
  in `events.jsonl`.
- Two consecutive seed-42 deep-soaks at play ≥ 10 via the JointIntention
  path before retiring the direct-emit path.
- After retire: `just q events <run-dir> PlayFired` returns empty; `just q
  events <run-dir> JointPlayBoutCompleted` ≥ 10.
- Focal-cat trace shows the PlayBout stage progression
  (Approach→Bouting→Cooldown) under the JointIntention substrate; per
  CLAUDE.md design pillar #4, the L2 trace must show the held Intention's
  persistence-bonus offset for the practice's held action.
- `just frame-diff <baseline> <post>` — socialize / grooming / courtship /
  mentoring tallies within ±10% of baseline (no collateral via shared
  matchmaker resources). If matchmaker bandwidth becomes contested,
  address with the concurrent-practice cap in `author_joint_intentions`,
  not by reverting.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- · **274** (ready, social-coordination, score 0.88) — Co-mentoring practice on JointIntention substrate
- · **405** (blocked, social-coordination, score 0.88) — Family ritual substrate (RitualKind + RitualWitness + bond-multiplier transmiss…
- · **275** (ready, social-coordination, score 0.88) — Joint cache-stocking practice on JointIntention substrate

<!-- linkages:end -->

## Log

- 2026-05-11: opened as JointIntention practice group stub (play is continuity canary).
- 2026-05-19: accuracy audit — ticket body incomplete (template boilerplate with no substantive content). [needs-review] on scope/approach/verification sections before work commences.
- 2026-05-25: body authored. 10-soak audit (seed 42, range 0–13 with intermittent zeros) confirms `personality_events.rs:80-90` four-AND gate × RNG·0.1 as the structural fragility — pre-066 play was 348; post-066 stuck at 8–14. Per design pillar #2, retiring the direct-emit in favor of a JointIntention `PlayBout` practice is the substrate-correct fix. Structural-option menu chose `retire`; layer-walk audit promoted all rows from `[suspect]` to `[verified-*]`. Supersedes 415 (PlayFired gate orthogonal to Explore), which had named R5 (retire / cross-ref 276) as its candidate; this ticket executes it.
- 2026-05-26: Commit A landed (PlayBout substrate + matchmaker + drop-arm `EventKind::JointPlayBoutCompleted`). Seed-42 15-min soak: `JointPlayBoutCompleted = 12`, footer `continuity_tallies.play = 21` (12 substrate + 9 legacy), survival + continuity hard gates pass. Substrate verified. Matchmaker emits 8247 JIs / drops 8247 / ~12 complete (0.14% completion rate) — the churn surfaces a substrate gap: matchmaker fiat without mutual-perception grounding. Opened **469** (Ground JointIntention emission in mutual perception — confidence + candidacy) as the substrate-correct follow-on; this ticket continues with Commit B (retire direct-emit + Bouting-stage cascade) under the current substrate shape. 469 composes with **279** (body-cue source) and **280** (mental-model belief-holder) on the same JointIntention substrate.
- 2026-05-26 (later): 469 retired after substrate audit against `docs/systems/ai-substrate-refactor.md` §4.7 / §7.M.4 / §12.3 / §12.4 — 469's proposed `confidence: f32` field and `PracticeCandidate` marker duplicate `MentalModel<Cat>.perceived_intent_clarity` (landed 258, `src/components/beliefs.rs:148`). See landed/469's Log for the audit. The substrate-correct fix path for the matchmaker churn is now **279** (adds `PlayBow` / `ReciprocalAdvance` / `SustainedOrientation` `WitnessableEvent` variants + `belief_integrator` arms) → **280** (matchmaker rebind on mutual `MentalModel<other>.perceived_intent_clarity > floor`). 276 Commit B (retire direct-emit + Bouting-stage cascade) continues under the current matchmaker shape; the matchmaker rebind itself moves to 280.
