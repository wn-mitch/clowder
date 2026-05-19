---
id: 420
title: Play continuity regression 254→5 on seed-42 post-084 — rule out unrelated cause
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-19
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: pending
landed-on: 2026-05-19
---

## Why

Ticket 084 Commit 3's verification soak (`logs/tuned-42/`, commit `fe8e1f77`) showed `continuity_tallies.play = 5` vs the pre-084 baseline's 254 — a 98% drop. This is alarming on its face, but `play` has shown wide variance across seed-42 baselines (160–300 range across recent landed-tickets), and the 084 substrate touches the herb economy / Farm DSE, neither of which has an obvious causal path to Play.

Likely a coincidence — `play` is a continuity canary that's particularly noise-sensitive on small populations (peak 8 in this run; one fewer cat triggering Play multiple times early-soak swings the count by hundreds). But "likely a coincidence" should be verified rather than assumed before pushing the 084 follow-ons further.

## Scope

- Run `just bisect-canary play` against the 084 commit range (Commit 1 → Commit 2 → Commit 3) to localize where the drop happened.
- If the bisect identifies Commit 2 (plan-template change) as the cause: investigate why the new gather→deposit chain would suppress Play. Hypothesis: longer plans hold cats longer in committed dispositions, reducing the L3 windows where Play can win.
- If the bisect identifies no clear single-commit cause and the drop spreads across commits: likely noise variance; document and close.

## Out of scope

- Adjusting Play DSE scoring constants — that's a balance change, not a regression fix.

## Approach

`just bisect-canary play 254 logs/tuned-42-pre-084` (or equivalent). Per-bisect-commit re-soak.

## Verification

Either a layer-walk identifying the causal path (and a fix that restores `play ≥ 100`), or a documented "this is variance" closure with evidence from a second seed.

## Log

- 2026-05-19: opened as 084 follow-on. Lowest-priority of the three — likely noise.
- 2026-05-19: **fixed-by-418 hypothesis falsified.** Re-soaked seed-42 post-418-fix (`logs/tuned-42`, commits ab1f3f38 + c2ad7967): `continuity_tallies.play = 5` (identical to pre-fix). 419 (WardPlaced regression) cleared decisively in the same soak (2 → 21), confirming the marker-snapshot fix landed. So the play regression survives independent of the CanWardFromSupply silent-fail — it's a genuine 084 substrate effect, not a downstream artifact. Re-frame: which of Commits 1 / 2 / 3 introduced it? Likely Commit 2 (plan-template change forces longer commitment chains — fewer L3 windows where Play can win). Next step is `just bisect-canary play 254` against the Commit 1 / 2 / 3 range.
- 2026-05-19: Closed without code. The 084 Iteration-2 §Observation table cell read 'Pre-084 baseline play=254 → Post-084 play=5' but the '254' is a stale carryover from Iter-1's 2026-04-29 table (logs/tuned-42-083, commit e838bb7). The actual baseline-of-record for Iter-2 is logs/tuned-42-pre-084 (commit 3e0153fe, 2026-05-18) which had play=7, not 254. Today's logs/tuned-42 (fe8e1f77) has play=5 — a same-binary delta of 7→5, well within continuity-canary noise. A real long-arc decline did happen (play ~250 stable through 2026-04-29, then stepwise down to single digits via 2026-05-04 e9d9ac1d → 2026-05-05 3c7c6c35 → 2026-05-06 25e28f25), but that decline predates 084 by two weeks and is unrelated to the herb-stash / chronicity work. Inverse-correlated with grooming explosion over the same window (60s → 1000–2900). If worth chasing, open a fresh long-arc bisect ticket against the 2026-05-04 → 2026-05-06 commit range — not as a 084 follow-on.
