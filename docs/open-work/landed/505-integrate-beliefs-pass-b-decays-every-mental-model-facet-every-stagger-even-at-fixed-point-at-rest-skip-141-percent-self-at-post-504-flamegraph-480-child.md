---
id: 505
title: integrate_beliefs Pass B decays every mental-model facet every stagger even at fixed point — at-rest skip (14.1 percent self at post-504 flamegraph, 480 child)
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-07-05
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 837b1aaf1f6f
landed-on: 2026-07-05
---

## Why
`integrate_beliefs` is #1 self at the post-504 flamegraph
(`logs/flamegraphs/42-419569a938af`): **14.10% self**, top child
`HashMap::retain` — the Pass B decay walk. The title's original
"at-rest skip" theory was WRONG (the retain already evicts
zero-strength models); a sizing probe found the real defect:
**`CatBeliefs.models` holds 300–700 entries per cat in an 8-cat
colony** (probe: `BELIEF_SIZES tick=1204004 cats=552/890 …`,
fluctuating 273–719 across the run). The map is supposed to hold
beliefs about *cats* (~7 targets); it is stuffed with churned
wildlife entities, each decayed 9 facets per stagger pass — thousands
of wasted lerps per tick.

## Current state (verified by sub-agent sweep, 2026-07-05)

Writers into `CatBeliefs.models` (all `belief_integrator.rs` Pass A):
13 arms; 12 key only cat entities (Groom/Mate/Care/Hunt-hunter/
PlayBow/ReciprocalAdvance/SustainedCoPresence/FesteringWound/
FleeFrom-fleer; Attack×2 are test-only). **Exactly one keys a
non-cat: the `FleeFrom → threat` arm (~line 452)** — `threat` is
sourced from `ec.wildlife.iter().min_by_key(...)` at `goap.rs:8313`
(nearest wildlife), flows through `FleeWitness.threat`
(`flee_travel.rs:22,99`), and Pass A does a blind
`cats.models.entry(*threat).or_default()` +
`perceived_violence_capability` write.

Readers of `CatBeliefs.models`: exactly one —
`affordance_writer.rs:158`, which iterates cat-snapshot targets only.
Threat violence-capability that consumers actually read comes from
**`PredatorBeliefs`** (`goap.rs:2619,2641`, `affordance_writer.rs:174`),
whose Implant path already maintains wildlife models. The
`prey_yield` facet is authored/read on `LocationBeliefs` only.

**Conclusion: wildlife entries in `CatBeliefs.models` are pure decay
ballast — read by nothing.** They persist until strength bleeds to 0
(no liveness sweep exists, despite the `beliefs.rs:227-237`
doc-comment claiming one — stale doc, fix in the same commit) and
wildlife churn keeps re-inserting them.

## Scope
- Delete the misrouted `FleeFrom → threat` insertion into
  `CatBeliefs.models` (behavior-preserving — no reader can observe
  those entries; `CatBeliefs` never serializes into the event
  stream).
- Correct the stale liveness-sweep doc-comment on `CatBeliefs`.

## Out of scope
- **Redirecting the FleeFrom threat-violence signal to
  `PredatorBeliefs`** — a real substrate improvement (witness learns
  the threat is violent) but behavior-CHANGING (PredatorBeliefs
  readers exist at goap.rs:2619) — fold into the 265 wildlife-belief
  wiring (0.4.0 Phase IV, plan.md step 18) where it lands
  dormant-then-activated with priors.
- True at-rest/closed-form decay for the remaining legitimate
  entries — revisit only if the frame is still hot after the ballast
  is gone.

## Verification
- Byte-identical event stream vs `logs/tuned-42-419569a9` common
  range (modulo 503-signature Patrol ULP + tail).
- `just flamegraph 42 60` post; integrate_beliefs self-time target
  well under 14.1%.
- `just check && just test`; `just verdict` pass.

## Log
- 2026-07-05: opened at the post-504 flamegraph; sizing probe +
  sub-agent writer/reader sweep replaced the at-rest theory with the
  FleeFrom-threat misrouting evidence.
- 2026-07-05: FleeFrom->threat ballast write removed: byte-identical soak (1 Patrol-ULP/503 + tail), verdict pass. Flamegraph UNCHANGED (14.1% self) — sizing probes traced the remaining ballast to NearPairCache composition (99.9% non-cat pairs); structural fix spun out to 506
