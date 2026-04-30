---
id: 2026-04-23
title: §13.1 rows 1–3 — Incapacitated consumer cutover + inline-branch retirement
status: done
cluster: null
landed-at: null
landed-on: 2026-04-23
---

# §13.1 rows 1–3 — Incapacitated consumer cutover + inline-branch retirement

Track C closeout for the Incapacitated-pathway half of §13.1.
Retires the inline `if ctx.is_incapacitated` early-return branch
at `src/ai/scoring.rs:574–598` by routing the same gating
through §4.3 marker eligibility on each non-Eat/Sleep/Idle DSE.
Behavior-preserving once the surviving Eat/Sleep Logistic curves
+ Idle's canonical axes handle the action set without the
bespoke `incapacitated_*_urgency_{scale,offset}` multipliers.
Shipped as Session A of the three-way fan-out; Sessions B and C
landed in the same integrated stack.

**`.forbid("Incapacitated")` added to every non-Eat/Sleep/Idle
DSE factory** — 21 cat DSE files touched in `src/ai/dses/`
(`build.rs`, `caretake.rs`, `cook.rs`, `coordinate.rs`,
`explore.rs`, `farm.rs`, `fight.rs`, `flee.rs`, `forage.rs`,
`groom_other.rs`, `groom_self.rs`, `herbcraft_gather.rs`,
`herbcraft_prepare.rs`, `herbcraft_ward.rs`, `hunt.rs`,
`mate.rs`, `mentor.rs`, `patrol.rs`, `socialize.rs`,
`wander.rs`) + all six `practice_magic.rs` siblings (Scry,
DurableWard, Cleanse, ColonyCleanse, Harvest, Commune), plus
fox DSEs that follow the same pattern. Each DSE's
`EligibilityFilter` gains the forbid line in its factory
constructor with a short §13.1 rustdoc comment.

**Inline branch retirement.** The 17-line
`if ctx.is_incapacitated { ... return ScoringResult { ... }; }`
early-return block in `score_actions` retires — replaced by a
rustdoc paragraph explaining the §4 marker path. The
`ScoringContext.is_incapacitated` field itself STAYS (other
consumers may still read it for non-scoring reasons per §13.1
spec contract); only the scoring-branch read retires.

**Five `ScoringConstants` deletions in `src/resources/sim_constants.rs`:**

- `incapacitated_eat_urgency_scale`
- `incapacitated_eat_urgency_offset`
- `incapacitated_sleep_urgency_scale`
- `incapacitated_sleep_urgency_offset`
- `incapacitated_idle_score`

Both struct-def entries and `Default` impl entries.

**Test updates in `src/ai/scoring.rs`:**

- `incapacitated_cat_only_scores_basic_actions` reshaped to
  exercise the new path. Builds a per-test `MarkerSnapshot` with
  `Incapacitated` set for the cat entity (the shared cached
  snapshot only carries colony markers); asserts Eat/Sleep score
  above `jitter_range`, Hunt/Fight/Flee score at most
  `jitter_range` in magnitude (forbidden DSEs return 0.0 from
  evaluator + jitter noise from the push path). Idle's score
  no longer asserted above jitter because the retired
  `incapacitated_idle_score = 0.2` offset is gone — Idle's
  canonical axes scoring near-zero for a hungry/tired cat is
  correct behavior.

**Non-goals (Session A scope-fence):** did NOT touch rows 4–6
(corruption-axis migrations — Session B), did NOT touch §7
commitment (Session C), did NOT delete
`ScoringContext.is_incapacitated` field (other non-scoring
consumers), did NOT update `docs/open-work.md` (plan-maintainer
scope).

**Verification:** `just check` clean. `just test` — all tests
pass on the integrated stack (session-isolated testing was
interrupted mid-verification by OOM from three concurrent
worktree compiles; re-verified post-integration).

**Integrated-stack soak footer (seed 42, `--duration 900`,
release, commit_dirty=false):** TBD (shared with sibling §13.1
rows 4–6 + Phase 6a landings).

**Specification cross-ref:** `docs/systems/ai-substrate-refactor.md`
§2.3 rows 1–3 + §4.3 `Incapacitated` marker row. Original kickoff:
`docs/systems/a1-4-retired-constants-kickoff.md`.
