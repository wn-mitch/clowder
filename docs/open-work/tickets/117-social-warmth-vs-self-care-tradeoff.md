---
id: 117
title: Characterize social-warmth max shift under 047 substrate (Phase 3 surfaced -96% on max)
status: ready
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: [118]
supersedes: []
related-systems: [ai-substrate-refactor.md, recreation.md]
related-balance: [047-acute-health-adrenaline.md]
landed-at: null
landed-on: null
---

## Why

Surfaced during ticket 047 Phase 3 hypothesize sweep:

- `welfare_axes.social_warmth.max`: 0.225 → 0.009 (-96%, p=0.17, d=-0.71)
- `welfare_axes.social_warmth.stdev`: 0.079 → 0.004 (-95%)

This is **characterization work, not a regression fix.** Cats live longer in injured states under the 047 substrate; the colony equilibrium shifts. The social-warmth max drop could be:

- Downstream of ticket 118's momentum gap (Sleep-locked cats between plan completions don't socialize)
- Real ecological shift (longer-lived but more cautious colony has different interaction patterns)
- An artifact of the welfare_axes.max metric specifically (max vs mean — perhaps mean held but the previous tail of high-warmth cats was removed)

The ticket tracks the signal so it isn't lost across iterations, not as an action item demanding a fix. Re-measure after ticket 118 lands.

## Scope

- Verify the hypothesis: re-run the 047 sweep after 118 lands; check whether social_warmth recovers.
- If yes: this ticket closes as a 118-side-effect.
- If no: tune `acute_health_adrenaline_sleep_lift` downward in increments (0.50 → 0.30 → 0.20) and re-sweep until social_warmth holds within ±10% of baseline.

## Verification

- Re-run hypothesize spec at `docs/balance/047-acute-health-adrenaline.yaml` post-118 with social_warmth as the cross-check metric.

## Out of scope

- The 094 stockpile-satiation interaction (separate composition concern).
- The 088 BodyDistressPromotion lift (separate ticket 111 retires it once kind-specific modifiers cover its surface).

## Log

- 2026-05-01: Opened from ticket 047 Phase 3 sweep findings. Likely a downstream symptom of ticket 118's momentum gap, but tracked separately so the social-warmth signal isn't lost if 118 doesn't fully resolve it.
