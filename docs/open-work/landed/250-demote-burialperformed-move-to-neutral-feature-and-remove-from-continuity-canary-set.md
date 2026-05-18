---
id: 250
title: Demote BurialPerformed — move to neutral feature and remove from continuity canary set
status: done
cluster: ai-substrate
initiative: []
added: 2026-05-08
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 668415919e3a
landed-on: 2026-05-09
---

## Why
The seed-42 deep-soak now consistently produces zero deaths
(`deaths_by_cause: (none)` post-247 / 248), so `BurialPerformed` is
expected to fire zero times in a healthy run. Two canaries currently
fail on this:

- `survival` canary (`scripts/check_canaries.sh`):
  `never_fired_expected_positives` includes `BurialPerformed`,
  treating "no burials this soak" as a survival defect.
- `continuity` canary (`scripts/check_continuity.sh`): six tally
  classes must each fire ≥1 — `grooming`, `play`, `mentoring`,
  `burial`, `courtship`, `mythic-texture`. Healthy colonies see no
  deaths and so no burials, failing the gate.

Both gates fire `verdict: fail` for what is structurally a colony
*health* signal, not a defect. 247 and 248 verifications had to
explicitly note that `verdict: fail` was driven solely by burial=0
(pre-existing in baselines). The right shape: burial is a
*conditional* signal (predicated on death occurring), not an
unconditional positive. Per ticket 035 the `BurialPerformed` Feature
was added as a positive continuity signal because death was common
enough to make burial routine; the post-247 / 248 substrate now
keeps colonies healthy enough that burial is genuinely rare. Demote
both canary slots so the verdict surfaces real defects without
noise.

## Scope
- `src/resources/system_activation.rs` — move `Feature::BurialPerformed`
  from `Positive` valence to `Neutral`. Add the
  `Feature::BurialPerformed => false` arm in
  `expected_to_fire_per_soak()`. Update the per-valence count
  assertion (`positive: 54 → 53`, `neutral: 32 → 33`) and the
  classification-test comment block at line ~1228 to reflect the
  reclassification.
- `scripts/check_continuity.sh` — drop `burial` from the six-canary
  for-loop. The remaining five (grooming, play, mentoring,
  courtship, mythic-texture) are unconditional in any healthy
  colony. Continuity tallies in the footer continue to record
  burials when they happen — the change is purely whether zero
  burials gates the verdict.
- `CLAUDE.md` — update the "Continuity canaries" line under
  Verification to reflect five canaries, not six. Add a one-liner
  noting burial is observed-but-not-gated (footer tally still
  emits).
- `docs/systems/ai-substrate-refactor.md` §11.3 — update the canary
  set source-of-truth.

## Out of scope
- Removing the `BurialPerformed` Feature emission itself — keep the
  emission so footer tallies continue to record burials when they
  occur (useful diagnostic when investigating death-rate regressions).
- Reclassifying other "rare-when-colony-healthy" Features (e.g.,
  `DeathStarvation`, `ShadowFoxAmbush`). Those have explicit hard
  gates separate from never-fired-positives; not a parallel
  problem.

## Current state
- 248 landed at sha 111987ae. Verdict run on `logs/tuned-42` shows
  `verdict: fail` driven by `survival: fail` (BurialPerformed never
  fired) AND `continuity: fail:burial=0`. All other canaries pass;
  aggregate score and welfare are at parity with baseline.
- The same caveat applies to 247's verdict and the
  post-246-floor-restored baseline — pre-existing condition.

## Approach
Three small file edits, no behavioral logic change:

1. `system_activation.rs` move + exempt + count update.
2. `check_continuity.sh` drop `burial` from the canary loop.
3. Doc updates (CLAUDE.md, ai-substrate-refactor.md §11.3).

After the edits: re-run `just verdict logs/tuned-42` to confirm it
now passes cleanly (no canary fail, just whatever footer drift
remains). Run the existing
`expected_to_fire_per_soak_classification` and
`representative_classifications` tests to confirm count assertions
still hold after the update.

## Verification
1. `just check` clean.
2. `cargo test --lib system_activation` passes — 18/18 including
   `features_total_in_matches_category_counts` (positive 53 / neutral
   33 / negative 23) and the new
   `expected_to_fire_per_soak_classification` assertions for
   `BurialPerformed`.
3. `bash scripts/check_continuity.sh logs/tuned-42/events.jsonl`
   passes immediately (5/5 canaries — the script change drops burial
   from the for-loop and reads only the remaining tallies).
4. `bash scripts/check_canaries.sh logs/tuned-42/events.jsonl` will
   STILL fail on `never_fired_expected_positives` because that field
   is baked into the events.jsonl footer at run time — the existing
   logs/tuned-42 was written by the pre-250 binary. The next fresh
   soak will produce a footer without `BurialPerformed` in the list,
   at which point `just verdict <run-dir>` exits 0 cleanly. Locking
   the change in via unit tests rather than re-running a 15-minute
   soak just to re-bake an invariant the tests already cover.

## Log
- 2026-05-08: opened from 248's verification observation that
  `verdict: fail` was driven solely by burial=0 (pre-existing
  across post-246, post-247, post-248 baselines). User directive:
  "burial should move to neutral event and not count as a negative
  canary now that death isn't as common."
- 2026-05-09: Post-247 / 248 substrate keeps colonies healthy enough that deaths (and therefore burials) are genuinely rare; treating zero burials as a never-fired-canary defect produced false 'verdict: fail' across post-246 / 247 / 248 baselines. Feature::BurialPerformed: Positive → Neutral; expected_to_fire_per_soak() returns false; check_continuity.sh drops 'burial' from the canary loop. Footer tally still emits when burials happen. Unit tests cover the classification (53/33/23 valence counts + explicit BurialPerformed assertions); verdict will pass cleanly on the next fresh soak. Per CLAUDE.md continuity-canary line updated to five-canary list.
