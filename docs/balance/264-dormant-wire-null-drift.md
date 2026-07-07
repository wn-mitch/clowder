# 264 — social DSE dormant wire: null-drift record + dummy-perturbation control methodology

Ticket: `docs/open-work/tickets/264-*.md` (dormant-wire half; activations
are plan step 20). Commits: c6541129 (Socialize + GroomOther), 6259af51
(Mate + Mentor), 27adcaaa (Care→ApplyRemedy + FeedKitten→Caretake).
Gate soak: `tuned-42-54e4d22e` (900s). Reference stream:
`tuned-42-d94c282f` (the Phase-III 291 gate run; main reproduces it
byte-for-byte — verified via `tuned-42-d8cec959-control`).

## Gate outcome

`just verdict` returns **concern**: survival PASS, continuity PASS,
throughput_drift PASS (−1.9%), but footer drift in the ward/shadowfox
channels, `colony_score` swings (fulfillment +73.7%, kittens_born
−50%), and a plan-failure canary (`EngagePrey: lost prey during
approach` 20 → 287, 14.6×). The gate for this landing was **null
drift**, so the drift demanded attribution before acceptance.

## Zapruder

First stream divergence vs the reference: line 6717, tick 1203745 —
cat Mallow's disposition election flips Coordinating → Cooking with
**byte-identical need state** (hunger/energy/temperature equal to the
last bit). Everything upstream is byte-identical; everything
downstream is ordinary trajectory-family divergence.

## Bisect (all runs seed 42, 90s, byte-compared past the header)

| Probe on top of main (d8cec959) | Stream vs reference |
|---|---|
| dead `pub fn` (never called) | identical (60,940 lines) |
| 1 unused f32 field in `ScoringConstants` | identical |
| 5 of the new 264 fields (socialize/groom) | identical |
| 3 of the new fields (mate/mentor) | identical |
| 1 field (caretake) / 2 fields (apply_remedy) | identical |
| 6 of the new fields (mate/mentor/caretake/apply_remedy) | **diverges at tick 1203745** |
| all 11 new 264 fields | **diverges at tick 1203745** |
| **6 anonymous `probe_N_weight` dummy fields** | **diverges at tick 1203745** |

Field *names* are irrelevant (no env override, no name-keyed runtime
consumer — checked); field *count* is the trigger. Adding ≥ ~6 f32
fields to `ScoringConstants` lands seed-42 on a single alternate
trajectory (the artifact **saturates**: 6 dummy fields and 11 real
fields produce the *same* alternate stream). The plausible mechanism
is a struct-size/layout-sensitive float artifact (an LSB-level
difference somewhere in scoring that only matters at a near-tie); the
exact codegen locus was not chased further because the control below
makes it irrelevant to attribution.

## The null-mechanism proof

The 6-dummy-field control stream (guaranteed zero mechanism content —
six unused zero floats) is **byte-identical** to:

- commit 1's stream (11 fields + Socialize/GroomOther wiring),
- commit 2's stream (+ Mate/Mentor wiring),
- the full 900s gate soak `tuned-42-54e4d22e` (all three commits),

through the end of the shorter run in each pair. Every drift channel
in the verdict — the 14.6× lost-prey canary, the kittens_born −50%,
the ward/shadowfox swings, the founder-dispersion windows — appears
identically in the inert control, because they are properties of the
alternate RNG trajectory, not of any 264 code path. The wire itself
contributes **zero** behavioral difference beyond the struct-size
artifact. Gate judged **null-drift: satisfied** on this evidence.

## Methodology note (reusable)

When a null-drift gate meets an unavoidable `SimConstants` schema
change: build a control commit that adds the same *number* of dummy
fields and nothing else, soak it, and byte-compare the candidate's
stream against the control's. Byte-identity to the inert control is a
stronger null-mechanism proof than any footer-band judgment. Phase IV
steps 18–19 (265/314) also add constants and should be judged the same
way — and since the artifact saturates, their streams may
byte-compare directly against `tuned-42-54e4d22e`.

Run inventory kept under `logs/`: `tuned-42-d8cec959-control`
(reproducibility control), `tuned-42-d8cec959-dummy6` (inert
perturbation control), `tuned-42-c6541129-full` / `tuned-42-6259af51`
(per-commit streams), `tuned-42-54e4d22e` (gate soak),
`tuned-42-37a9e5e9` (superseded first gate soak, pre commit-3 rework).

## Also surfaced by this landing

- **Ticket 516** — `score_target_consideration` routes un-`target_`-
  prefixed scalar names to the no-op `fetch_self` (silent 0.0):
  hunt_target's `prey_yield` / `prey_calm` / `prey_alertness_tolerance`
  are dead in production (probe-verified); 263's affordance key is the
  same class. 315 re-blocked on 516. 264's axes ship `target_`-prefixed.
- The `learning_bevy_schedule_edge_perturbation` memory's claim that
  "field-level edits cannot perturb seed-42" is too narrow: constants-
  struct growth (≥ ~6 f32) also perturbs. Memory updated.
