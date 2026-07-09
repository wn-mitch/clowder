# 310 S1 — shadow-fox satiation drive: four-artifact record

Ticket: `docs/open-work/tickets/310-*.md` (S1 of the staged plan; release-plan
step 23). Commits: 12c5eea9 (drive + gates + scenario) → 55c75f5c (hunger
eligibility gate) → ef08d805 (escalation satiation gate) → 1effd660
(KnowledgePromoted gate demotion, user-approved). Baseline for all verdicts:
`tuned-42-e6a0d627` (step-22 accepted stream). Accepted gate artifact:
`tuned-42-1effd660` (900s) — verdict **concern** (survival PASS, continuity
PASS, never-fired clean, throughput −1.1%).

## Hypothesis

Shadow-fox predation timing was governed only by `ambush_cooldown` (100
ticks) + the 5%/tick stalk roll — ticket 310's "pinball with a kill rule."
S1 adds a satiation ledger: kills feed it (+0.8 cat ambush / +0.4 prey
kill), the motivation cadence decays it (0.001 per 16-tick cadence),
satiation ≥ 0.7 suppresses the legacy stalk roll, and hunger
`(1 − satiation)² × 0.10` joins the 023 motivation softmax as a fifth
drive electing Stalking deliberately (beyond legacy sight 8, within
scan 12). Escape hatches: weight 0.0 restores the four-drive softmax
byte-exactly (fifth score + jitter draw skipped); threshold > 1.0
disables all satiation gating.

## Iteration 1 (`tuned-42-12c5eea9`, 900s) — discordant: election defect

20 Ambush events vs baseline's 1; 8 on one cat in 243 ticks (12–80-tick
spacing, below the 100-tick cooldown). The event payloads carried the
diagnosis: `ShadowFoxHungerHuntEntered.satiation` recorded elections at
**0.987 / 0.868 / 0.787** — the hunger candidate joined the softmax
whenever its weight was nonzero, so once ANY drive held the pressure
floor open, temperature spread elected the ~zero-pressure hunger
candidate ≈ 1/5 of cadences. The four 023 drives are benign at zero
pressure (their states pace and wander); hunger elects Stalking, and a
Stalking fox adjacent to its target ambushes the same tick.

**Fix (55c75f5c):** eligibility-before-scoring —
`hunger_eligible = weight > 0 && hunger_pressure ≥ shadow_fox_motivation_min_pressure`.
Regression test `hunger_below_floor_never_elected_under_softmax_spread`
(temp 10.0, dread holding the floor, satiation 0.98, 100 elections →
zero Stalking). Latent pre-existing sibling noted, out of scope: the
softmax can elect any near-zero 023 drive the same way — harmless there,
but the same shape.

## Iteration 2 (`tuned-42-55c75f5c`, 900s) — discordant: second blind path

Eligibility gate proven in-run (zero hunger elections), but ambushes ROSE
to 71 (~30 on one cat at 42–65-tick spacing; 169 Haunting entries, 5
banishments). The wave driver is the satiation-blind **023
Haunting-escalation loop**: an ambush tanks the victim's mood/safety;
Dread reads exactly those; Haunting is re-elected; 30 ticks later the
escalation promotes to Stalking; ambush again. The ambush execution
(Stalking arm, dist ≤ 1) never checks `ambush_cooldown` — that only
gates fresh stalk rolls — so the loop runs at the escalation period. The
loop also re-fed the fox every ~45 ticks, pinning satiation at 1.0:
**the escalation loop was hiding S1's own drive** (hunger never became
eligible all run). colony_score: health −18.4%, fulfillment −15.0%.

**Fix (ef08d805):** the third physical-predation entry gets the same
gate — Haunting → Stalking promotion requires
`satiation < shadow_fox_stalk_satiation_threshold`. A fed shadow-fox
keeps haunting (the drain still runs — it watches, and waits) and
promotes once cadence decay brings hunger back. Tests:
`fed_haunting_fox_does_not_escalate` / `hungry_haunting_fox_escalates`.

## Iteration 3 (`tuned-42-ef08d805-s1-900s` + 1800s `tuned-42-ef08d805`) — concordant

- 900s: deaths 0, ambushes 0, Haunting 16 / Tending 44 / Reconstituting
  391 / Seeding 530 — baseline-magnitude ecology (baseline: 10 haunts /
  1 ambush / 0 banished; iteration 2: 169 / 71 / 5). No waves.
- 1800s (150,563 ticks): **the genuine hunt-feed-rest cycle at soak
  scale** — 3 hunger elections at satiation **0.078 / 0.057 / 0.055**
  (contrast iteration 1's 0.987) producing exactly 1 ambush at tick
  1,332,805. Deaths: 1 Injury; continuity pass.
- Prediction overshoot, accepted as-is: expected "10–30 spread
  ambushes", got ~1 per 150k ticks. Shadow-foxes live at the corrupted
  edges and prey kills can hold them above the hunger regime; the
  scenario (`shadowfox_hunger_hunt_cycle`) proves the full cycle
  deterministically. Livelier predation regimes are S4/S5's business
  (DSE-shaped scoring, ward-snapshot retirement) — not a reason to
  hot-tune S1 constants.
- Drift flags on the accepted artifact (fulfillment −13.5%, happiness
  −7.2%, health +5.7%, shelter +9.7%) with zero ambushes = the
  knife-family drift class (+6 SimConstants fields, documented 265
  control-ladder behavior); fulfillment trend is a step-24 watch-item.

## Gate-policy change riding with S1 (user-approved)

`KnowledgePromoted` demoted from the per-soak never-fired set
(1effd660). It zeroed at 900s in three of five recent trajectory
families AND at the 150k-tick 1800s window — chain-rare at or beyond the
window size, so the per-soak gate was a coin flip costing a
doubled-window re-soak per tails (paid at step-21 gate 3, step 22, and
here). Mechanism-break protection: the `colony_knowledge_false_belief`
scenario asserts the Feature deterministically on every `cargo test`
(+ 17 unit tests). Ecological-starvation protection: the step-24
promotion-cadence watch-item. Footer-only change — the single production
read of `expected_to_fire_per_soak` is `never_fired_expected_positives()`.

## Also surfaced: soak harness frame-hitch double-emission

While byte-checking 1effd660 vs ef08d805-s1-900s (expected identical
minus footer), the REF run showed a frame-hitch artifact at tick
1,262,700: a batch of per-tick events emitted TWICE with identical
content (`CourtshipDrifted`, `DirectiveIssued`, `FoxPlanCreated`), three
`CatSnapshot` cadence emissions dropped shortly after (1263300–1263500),
and genuine behavioral fork from tick 1,263,235 — consistent with
per-tick systems re-running on an unadvanced tick under load and
double-applying state (courtship drift). The two streams were
byte-identical for 140,893 lines before the hitch. Consequences: (a)
byte-identity gates require hitch-free runs (re-run on divergence and
check for duplicate-emission signatures before diagnosing the change),
(b) run-to-run trajectory comparisons under machine load carry this
caveat. Ticket opened: see `just open-work-ready` (tooling-diagnostics).

## Verdict

S1 **accepted** at 1effd660. Satiation is now the single gate on all
three physical-predation entries (legacy roll, hunger election, haunting
escalation) — one mechanism, trace-visible via
`ShadowFoxHungerHuntEntered.satiation`. Rolled to S4: consider whether
the hunger drive should price prey-hunting (currently only cat-stalks),
and the latent near-zero-drive election sibling on the four 023 drives.
