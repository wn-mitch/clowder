---
id: 027b
title: L2 PairingActivity — substrate-aware structural commitment layer (027 Bug 3 successor)
status: blocked
cluster: null
added: 2026-04-28
parked: null
blocked-by: [071]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 027 Bug 3's original spec shape (new `DispositionKind::Pairing`
variant + new `pairing_activity.rs` self-state DSE + target-taking
sibling + GOAP step resolver) is **obsolete** post-052/065. Per
`commitment.rs:175–183` doc-comment, "L1/L2 strategies are carried
inline on the emitting aspiration DSE and pairing-activity Intention,
not [in `DispositionKind`]". Target-taking DSEs are also no longer
independent score competitors; they run as second-pass selectors
inside step resolution. A new `pairing_activity_target.rs` would be
inert against the post-052/065 substrate.

The 2026-04-26 bias-only intervention on `socialize_target` (a fifth
`target_partner_bond` axis) is on HEAD but its single-seed deep-soak
landed in the unlucky tail of seed-42 noise, leaving acceptance
inconclusive. Empirically: the Mocha+Birch failure mode in the bug3
trace showed cats reaching `romantic = 1.0` but stalling at the
Partners-bond gate on the *fondness* axis. `partners_fondness_threshold`
was dropped 0.60 → 0.55 to compensate, but no structural mechanism
exists for a cat to *commit to a single partner across ticks* — the
passive courtship-drift loop accumulates fondness diffusely across all
peers.

This ticket implements L2 `PairingActivity` as a parallel persistent
commitment layer orthogonal to `GoapPlan`: a per-cat ECS component
holding `partner: Entity` that survives every disposition swap and is
read at evaluation time by existing target-pickers and self-state
scoring. No new `DispositionKind`, no new self-state DSE, no new GOAP
step resolver.

## Scope

Three commits, atomic and verifiable.

### Commit A — substrate landing, no behavior change

- New `src/components/pairing.rs` — `PairingActivity` component +
  `PairingProxies` snapshot + pure `should_drop_pairing` truth-table
  function with 14 unit tests.
- New `src/ai/pairing.rs` — `author_pairing_intentions` per-tick
  system reusing `MatingFitnessParams::snapshot()`. Idempotent
  insert/remove with synthetic-world tests covering the emission
  happy-path, no-emit-without-bond, no-emit-out-of-range,
  no-emit-while-pregnant, drop-on-partner-death, drop-on-bond-loss,
  no-double-emit transitions.
- `PairingConstants` block on `SimConstants` (range 25,
  emission_threshold 0.25, romantic/fondness floors 0.05/0.30,
  axis weights 0.40/0.40/0.20).
- Three `Feature` variants: `PairingIntentionEmitted` (Positive),
  `PairingDropped` (Neutral), `PairingBiasApplied` (Positive). All
  three start `expected_to_fire_per_soak() => false` in Commit A.
- Schedule edge in `simulation.rs` after
  `update_mate_eligibility_markers`.

### Commit B — wire L2 bias readers

- `socialize_target.rs::bond_score` extends to recognize the
  Intention partner as a hard 1.0 (overrides graduated bond-tier
  scalar). Thread `Option<&PairingActivity>` through
  `resolve_socialize_target`.
- `groom_other_target.rs` — fifth `target_pairing_bond` axis
  (Linear, weight 0.15, four existing weights renormalized ×0.85).
- `goap.rs::evaluate_and_plan` — fold `pairing_q: Query<&PairingActivity>`
  into existing SystemParam bundle; forward to both target resolvers.
- `scoring.rs` — `apply_pairing_bonus` (additive lift on
  `Action::Socialize` / `Action::Groom` / `Action::Wander` while
  Intention is held; mirrors `apply_priority_bonus` shape). Add
  `pairing: Option<&PairingActivity>` to `EvalInputs`.
- Promote `PairingIntentionEmitted` and `PairingBiasApplied` to
  `expected_to_fire_per_soak() => true`.
- Fire `Feature::PairingBiasApplied` only when picked candidate ==
  Intention partner AND pre-pin `bond_score < 1.0` (isolates "L2
  actually changed selection").

### Commit C — focal-trace observability + multi-seed verification

- `trace_log.rs` — `PairingCapture { partner_name, partner_bond_tier,
  romantic, fondness, familiarity, ticks_held, drop_branch_if_dropped }`.
- `FocalScoreCapture` — `pairings: Vec<PairingCapture>` field with
  `push_pairing` method. Author system writes per-tick rows for the
  focal cat.
- `tests/` integration test against deterministic Fern+Reed two-cat
  world: seed at Friends bond + orientation-compat, run for N
  seasons, assert Partners-bond formation rate.
- `docs/balance/027-l2-pairing-activity.md` — hypothesis +
  predictions P1–P4.
- Run `just baseline-dataset 2026-04-26-bug3-l2pairing` +
  `just sweep-stats … --vs logs/baseline-2026-04-25`. Acceptance:
  - **P1**: `MatingOccurred > 0` in ≥ 1 of 12 sweep runs.
  - **P2**: `BondFormed_Partners > 0` in ≥ 4 of 12 runs.
  - **P3**: `PairingBiasApplied / SocializeTargetResolves > 0.10`
    in ≥ 50% of runs.
  - **P4**: Survival canaries within ±10% noise band; Cohen's d <
    0.5 on `mean_lifespan` and `colony_size_end_of_window`.
- On full pass: `just promote logs/baseline-2026-04-26-bug3-l2pairing
  027bug3-l2pairing` (refreshes the stale `post-033-time-fix`
  pointer in `logs/baselines/current.json`); flip 027b to `done`;
  close 027.

## Out of scope

- Defense / provisioner / play character-expression bias channels on
  `fight_target` / `hunt_target` / Patrol (defer to ticket 027c).
- `mate_target.rs::mate_intention()` vestigial-tag fix (locked by
  test `intention_is_pairing_activity` at line 400; touching it
  expands blast radius for no behavioral gain).
- First-class L1 `ReproduceAspiration` aspiration-catalog entry —
  the `MatingFitness` gate is functionally equivalent for now.
  Confirmed 2026-04-28: no `Reproduce` chain in
  `assets/narrative/aspirations/*.ron`.
- Multi-partner / partner-switching cadence (defer to the §7.4
  fanaticism-vs-flexibility design knob).
- Coefficient tuning of `PairingConstants` defaults (post-landing
  balance work).

## Current state

Successor ticket to 027 Bug 3. Bug 3's bias-only intervention is on
HEAD (target_partner_bond axis on `socialize_target`,
`partners_fondness_threshold = 0.55`, `courtship_romantic_rate =
RatePerDay::new(3.5)`). 027b adds the structural commitment layer
that bias-only could not provide.

## Approach

L2 `PairingActivity` lives orthogonal to `GoapPlan`. The author
system runs every tick (idempotent — only insert/remove on
transitions) reusing the `MatingFitnessParams` snapshot from the
sibling mate-eligibility author. Bias readers in Commit B query
`Option<&PairingActivity>` directly; presence of the component is
the marker — no parallel ZST.

The five drop branches (`PartnerInvalid`, `BondLost`,
`AspirationCascade`, `SeasonOut`, `DesireDrift`) match §7.M's
OpenMinded gate. First-match precedence orders them so a degenerate
state where every branch fires reports the most-load-bearing branch
first (partner death over relationship-axis collapse).

## Verification

After each commit: `just check && just test` green; single-seed
`just soak 42 && just verdict logs/tuned-42` shows no metric drift
(Commit A) / events fire and no canary collapse (Commit B). After
Commit C: full `just baseline-dataset` + `just sweep-stats` clear
all four predictions P1–P4 above.

## Log

- 2026-04-28: Opened from 027 Bug 3 partial-Log recommendation
  ("open as ticket 027b rather than nesting a fourth bug here").
  Plan stored at `~/.claude/plans/let-s-work-027-golden-boot.md`.
  Commit A in flight.
- 2026-04-28: **Substrate landed; activation deferred** in commit
  `e95205bb` (combined Commit A + B) plus the activation-deferral
  follow-on. The 15-min seed-42 release deep-soak with the schedule
  edge active produced `Starvation = 3` (cluster death tick 1344K,
  last 11% of run) versus zero pre-027b at the same parent commit
  (cef9137; see `logs/tuned-42-cef9137-clean`). Diagnosed as the
  same Bevy 0.18 scheduler-shift hazard documented on ticket 061:
  registering `author_pairing_intentions` in chain 2a perturbs the
  topological sort enough to deflect seed-42's late-soak food/eat
  cadence. The author body itself is a true no-op when no Friends-
  bonded reproductive pair exists, so the regression is from the
  schedule reshuffle, not the system body.

  Activation deferred per the 061 precedent — schedule line in
  `plugins/simulation.rs` commented out, with a block comment
  explaining the hazard and the path to activation. The substrate
  is otherwise live: `PairingActivity` component, `should_drop_pairing`
  gate, bias wiring on `socialize_target.rs::bond_score`, three
  `Feature::Pairing*` variants. Both Pairing Positive features
  re-exempted from the `expected_to_fire_per_soak()` canary while
  dormant.

  Commit C scope changes: focal-trace observability still
  warranted (it'll inform the activation work), but multi-seed
  sweep is **deferred to the activation lift**. The active-bias
  failed soak is preserved at
  `logs/tuned-42-027b-active-failed/` for reference. Balance
  doc `docs/balance/027-l2-pairing-activity.md` updated with
  the active-bias observation + concordance + activation-status
  framing.

  Status remains `in-progress` until the activation lift lands
  via a follow-on. Recommended path: open ticket 027c with
  scope = "investigate scheduler-resilient activation strategy
  for L2 PairingActivity" and treat as a research-style ticket
  (the 061 precedent suggests this class of system needs
  attention to chain-placement, run-condition gating, or a
  fundamentally different schedule strategy).

- 2026-04-29: **Diagnosis corrected; reactivation blocked-by 071.**
  The "Bevy 0.18 topological-sort reshuffle" framing above is
  mechanically wrong. Chain 2a's marker batch is wrapped in
  `.chain()` at `src/plugins/simulation.rs:378`, which enforces
  source order — adding a system inside a `.chain()` block does
  not reorder its neighbors. The actual mechanism, established
  via direct grep + tick-1.2M divergence analysis on the two
  preserved soaks (`logs/tuned-42-027b-active-failed/` and
  `logs/tuned-42/`):

  1. The two runs are bit-identical for the first ~1.2M ticks
     (~80% of the soak) at the same commit + constants + seed.
     `PairingIntentionEmitted = 0` and zero `PairingActivity`
     insert events confirmed by direct grep — the author runs
     but never authors anything; bias readers collapse to
     identical math when `pairing_partner = None`.
  2. At tick ~1.2M a single mate-selection flip appears
     (Calcifer pairs with Simba in active, Ivy in deferred).
     The likely RNG-drift source is `MatingFitnessParams::
     snapshot()` HashMap iteration nondeterminism +
     `Res<SystemActivation>` write-contention rearranging
     Bevy's parallel-execution graph when a new writer is
     added.
  3. **The planning substrate amplifies that small divergence
     into a colony cascade.** Mocha's 109 `HarvestCarcass`
     failures, Nettle's 66 identical `TravelTo(SocialTarget)`
     failures, Lark's 91 `EngageThreat` failures — same bug
     class as tickets 038 (Founding-haul Flee-lock) and 041
     (Mallow Cook-lock). The substrate's three defenses
     (`replan_count` cap, per-plan `failed_actions` set,
     commitment drop gates) don't survive plan abandonment;
     the cat re-picks the same blocker.
  4. The L2 PairingActivity author isn't itself broken — it's
     a perturbation that exposes pre-existing fragility. Any
     sufficiently-perturbing schedule change would expose the
     same bug class.

  **Action.** Sub-epic ticket 071 opened with 10 child tickets
  (072–082) hardening the planning substrate inside the IAUS
  engine — every cross-tick "memory-of-failure / stay-where-
  you-are / force-fallback" defense lands as a Consideration,
  Modifier, or EligibilityFilter, not as a post-hoc pin. Adds
  `check_iaus_coherence.sh` to `just check` to enforce the
  discipline going forward. Ticket 027b reactivation is now
  ticket 082 (blocked by 072 + 073 + 074 minimum). 027c is
  not opened — the original "scheduler-resilient activation"
  framing was based on the wrong diagnosis.

  This ticket flips to `status: blocked`, `blocked-by: [071]`.
  Reactivation lands via ticket 082 once 071's children land.
  Plan stored at `~/.claude/plans/working-in-users-will-
  mitchell-clowder-keen-planet.md`.

- 2026-04-29: **Side observation — `bond_score` pin is the
  MacGyvered anti-pattern this hardening closes.** The
  `if pairing_partner == Some(target) { return 1.0 }` at
  `src/ai/dses/socialize_target.rs:193` (Commit B) is
  itself a post-IAUS override that subverts the partner-bond
  consideration's curve. Ticket 078 backports it to a
  first-class `target_pairing_intention` Consideration so
  the Intention partner's lift flows through the IAUS score
  economy — fully traceable, tunable via curve parameters,
  and inspectable in the modifier-pipeline trace. This is
  in scope of 071 (not deferred to 027c).
