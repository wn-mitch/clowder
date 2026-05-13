---
id: 267
title: Conflict-low DSEs — Threaten / Posture / Hiss escalation rungs (cheap pre-Fight signaling)
status: blocked
cluster: combat-threat
initiative: []
added: 2026-05-10
parked: null
blocked-by: [261]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Cats have a binary fight-or-flight today: when threat scoring crosses threshold, it's Flee or (post-256-R6 split) EngageThreat. Real cats have cheap escalation rungs *before* commitment to combat — `Threaten` (low body posture, raised hackles), `Posture` (size display, side-on stance), `Hiss` (vocal escalation). These signal "I see you, I might fight, but I haven't decided" and let conflicts resolve without combat (which is expensive in injury/death) when the other side reads the signal and backs off. They also feed C3's belief substrate: a cat who hisses at me elevates my `MentalModel<that_cat>.perceived_violence_capability` *without* requiring them to actually attack.

Pre-existing related work: ticket 109 (IntraspeciesConflictResponse — full four-valence framework, landed) is the parent design naming the four-valence shape (fight/flee/freeze/fawn). This ticket adds *signaling* rungs (sender side) that don't fit cleanly into the four valences but compose with them — Threaten/Posture/Hiss are *pre-commitment* signals that can resolve a conflict before any of the four valences fire. Sister tickets to compose with: 145 (Submit gesture DSE — receiving side; cat submits when threatened), 143 (IntraspeciesConflictResponseFight — combat valence Modifier), 270 (EngageThreat split — cross-species combat DSE). Together: 109 names the framework, 142/143/144/145 implement intraspecies sub-valences, this ticket adds escalation rungs, 270 implements cross-species combat.

## Scope

- **`Threaten` DSE** (new): scoring elevates when `perceived_hostility(target)` is rising AND `perceived_violence_capability(target)` is roughly matched (no point threatening someone vastly stronger or weaker — different DSE choice). Resolver: emit a body-cue marker (`BodyCueThreatening`) read by 243.
- **`Posture` DSE** (new): scoring elevates with audience presence (other cats nearby) — posturing is partly social signaling, not just dyadic. Resolver: emit `BodyCuePosturing` marker.
- **`Hiss` DSE** (new): scoring elevates with my distress level + proximity to target. Resolver: emit `WitnessableEvent::Hiss` (audible cue per ticket 244, distinct from body cue).
- **Per-DSE Affordance reads**: `Affordance(Threaten|Posture|Hiss, self, target)` from ticket 261's substrate.
- **Per-DSE Belief reads**: `MentalModel<Cat>(target).perceived_violence_capability`, `.perceived_hostility`.
- **Resolver-side cue emission**: each conflict-low resolver emits the appropriate cue via 242/243/244 channels so other cats' belief integrators (258) can consume.

## Out of scope

- The receiving side (`Submit` / `Yield` / `BackOff` DSE) — ticket 145 owns.
- Combat itself — ticket 143 (IntraspeciesConflictResponseFight) and the 256 R6 EngageThreat split (sibling ticket) own.
- Cross-species conflict signaling. v1 is cat-cat only. Cross-species (cat hissing at fox, fox snarling at cat) is a cleaner extension that may or may not warrant a follow-on ticket.
- The body-cue substrate itself (242 owns).
- Audible-cue substrate (244 owns).

## Current state

- Blocked-by 258 (Belief substrate) + 261 (ActionAffordances substrate).
- Pre-existing related work: ticket 145 (Submit gesture DSE — receiving side); ticket 143 (IntraspeciesConflictResponseFight — combat valence). This ticket completes the conflict-vocabulary triangle: send (this), receive (145), commit (143 / 256-R6).
- Cue substrate dependencies: emit-side wiring needs 242 (body cues) and 244 (audible cues) to ship in some form. v1 of this ticket can land DSE scoring + resolver structure without the cue emission landing first; cue emission becomes a follow-up commit once 242/244 ship.

## Approach

1. Audit ticket 145's design to align Submit-side reading semantics with this ticket's emit-side (cues need to be on the same channel).
2. Land Threaten DSE first (least audience-dependent; simplest to test).
3. Land Hiss DSE second (audible cue).
4. Land Posture DSE third (audience-dependent scoring is novel — needs a "neighbors nearby" consideration).
5. Wire each DSE's cue emission via the appropriate substrate.

## Verification

### Per-DSE scenario microexperiments

- `threaten_picks_matched_capability_target` — two candidate targets, one matched, one vastly stronger; verify Threaten picks the matched.
- `hiss_at_distress_threshold` — cat in rising distress with target nearby; verify Hiss fires at the threshold.
- `posture_audience_dependent` — same dyad, with vs without audience cats nearby; verify Posture fires only when audience present.
- `threat_signal_lifts_target_belief` — cat A threatens cat B; verify B's `MentalModel<A>.perceived_violence_capability` lifts (via 258's belief integrator consuming the emitted cue).
- `threat_signal_yield_chain` — cat A threatens cat B; B has Submit DSE wired (145); verify the conflict resolves *without* triggering Fight (the substrate works).

### Soak gates

- Combat mortality canary stable per healthy-colony.md (this ticket should not increase combat — it provides off-ramps).
- bonds_formed not regressed.

## Log

- 2026-05-10: opened sibling-to-258. Sender side of conflict signaling; pairs with 145 (receiver side) and 143 / 256-R6 (commit side). Session plan: `~/.claude/plans/after-working-256-i-dreamy-fiddle.md`.
