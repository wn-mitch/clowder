---
id: 302
title: investigate just soak vs just sweep non-determinism on identical seed/binary
status: ready
cluster: process-discipline
orchestration: substrate-sensitive
added: 2026-05-12
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [300-ward-candidate-step.md]
landed-at: null
landed-on: null
---

## Why

`just soak 42` and `just sweep <...>` produce **different deterministic outputs** on the same binary, same `--seed 42 --duration 900` flags, same source-tree commit, sequential runs (no overlap). Surfaced during ticket 300's pre-flight (2026-05-12) — `docs/balance/300-ward-candidate-step.md` Side-finding section has the empirical readout.

Concrete divergence on commit `5fedc33b` (dirty), seed 42, default constants, binary `target/release/clowder` at timestamp `2026-05-12 17:42`:

| Field | `just soak 42` | `just sweep` baseline |
|---|---|---|
| `deaths_by_cause.Starvation` | 0 | 1 |
| `colony_score.aggregate` | 2189.8 | 2162.4 |
| `negative_events_total` | 24524 | 25076 |
| `WardPlaced` event count | 16 | 16 |
| `WardPlaced` positions/ticks/cats | (matching) | (matching) |
| `continuity_tallies.courtship` | 3682 | 3804 |

Same upstream macro decisions (placements match exactly), divergent downstream colony state. The harness-level RNG must be perturbed by something that differs between the two invocation paths — `cargo run --release` (soak) vs `./target/release/clowder` direct (sweep). Both have `--seed 42 --duration 900`; the binary file is the same inode.

This matters because:
- **Comparability invariant.** CLAUDE.md guarantees "runs are only comparable iff their headers match on `constants` and carry the same non-dirty `commit_hash`." Both runs satisfy this, yet they diverge. The invariant is incomplete — invocation path is also load-bearing.
- **Cross-cluster baseline contamination.** `just verdict` reads `logs/baselines/current.json`. If a baseline was promoted from a `just soak` run and the next verdict comparison runs against a `just sweep` outcome (or vice versa), the drift report would attribute environmental noise to the change under test.
- **Hard-gate ambiguity.** ticket 300 saw `Starvation=0` (soak) vs `Starvation=1` (sweep) on identical config. Either gate-result is treated as authoritative depending on which harness ran. The "canonical seed-42 deep-soak" gate language doesn't disambiguate.
- **Prior balance landings may need re-reading.** 285 / 296 / 297's "byte-identical placement on all three seeds" was measured within the hypothesize harness. The architectural inference is still load-bearing (both baseline and treatment are run under the same harness, so internal comparison is consistent), but absolute footer comparisons against `just soak`-derived archives could mislead.

## Scope

- Diff `just soak` and `just sweep` recipes for environment variables, cwd, file descriptor inheritance, parallelism settings (rayon/tokio thread counts), `cargo run` vs direct-binary invocation effects.
- Reproduce minimally: same binary, same seed, same duration, two invocation paths, capture deltas. Should land a small repro script.
- Identify the source of non-determinism — likely candidates: Bevy ECS parallel-system thread scheduling (rayon thread pool size), file-system-mtime entropy, environment-variable propagation, `cargo run` rebuild check, `RUST_LOG`/`RAYON_NUM_THREADS` defaults.
- Land a fix that makes the two harnesses produce byte-identical outputs at identical config, OR document the irreducible divergence and update CLAUDE.md's comparability invariant to name the harness as a factor.

## Out of scope

- Re-running prior balance hypothesize sweeps against `just soak`-derived baselines (separate audit ticket if needed).
- Investigating non-determinism within a single harness across reruns (no evidence this happens; the divergence is *between* harnesses, not within).
- Bevy upstream investigation into ECS-scheduler determinism (the sim should be deterministic at the Clowder level; if it isn't, that's a Clowder-side bug to fix, not a Bevy issue).

## Current state

The two recipes (from `justfile`):
- `soak SEED="42"` runs `cargo run --release -- --headless --seed {{SEED}} --duration 900 ...`
- `sweep LABEL ...` runs `./target/release/clowder --headless --seed ${seed} --duration {{DURATION}} ${force_arg} ...` after `cargo build --release` if needed.

`cargo run` may set additional env vars or trigger an incremental rebuild check that `./target/release/clowder` direct invocation skips. `just sweep` also uses `xargs -P {{PARALLEL}}` (default 4) for job parallelism, but with a single seed × single rep that should have no effect (the inner binary invocation is the same regardless).

No reproduction harness yet; ticket 300's side-finding has two example output archives that exhibit the divergence:
- `logs/tuned-42/` (just soak 42, Starvation=0, score=2189.8)
- `logs/sweep-baseline-halving-ward-placement-candidate-step-from-5-to-2-lets-the-s/42-1/` (just sweep via hypothesize, Starvation=1, score=2162.4)

## Approach

1. Build a minimal repro: same binary, same seed, same duration, two ways of invoking it (`cargo run --release --` vs `./target/release/clowder` direct). Capture footer + ward-event diff. If reproducible without xargs/just/uv layers, the divergence is at the binary's entrypoint.
2. Bisect what perturbs the run:
   - `env -i ./target/release/clowder --headless --seed 42 --duration 900 ...` (cleared environment) — does it match either harness? Neither?
   - Set `RAYON_NUM_THREADS=1` explicitly — eliminates ECS parallelism as a variable.
   - Strace / DTrace the two invocations for syscall divergence at startup.
3. Once the perturbation source is identified:
   - If it's environment / RNG-thread-count related, set the deterministic config explicitly in `main.rs` and document the requirement.
   - If it's `cargo run`'s rebuild check, switch `just soak` to use `./target/release/clowder` directly (matching `just sweep`).
   - If it's irreducible (e.g., timestamp-based jitter we can't remove), document and update CLAUDE.md.
4. Run a parity soak across both harnesses on the fix → byte-identical footer + ward events.
5. Document in CLAUDE.md what the comparability invariant now requires (likely just adds "and the same invocation path" until the fix lands; can be dropped after the fix).

## Verification

- Reproducible minimal divergence demonstrated → cause identified.
- Byte-identical footer + WardPlaced event timeline between `just soak 42` and `just sweep`-style direct invocation on the same commit + binary.
- All five continuity canaries match within ±0% between the two harnesses.
- CLAUDE.md comparability-invariant language updated if the divergence has an irreducible component.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **189** (done, ai-substrate, score 0.85 (cross-cluster)) — Post-178 food_available regression — layer-walk diagnosis
- ✓ landed **196** (done, process-discipline, score 0.84) — verdict.py substrate-fired-≥1× probe (194 P7)
- ✓ landed ** 83** (done, ai-substrate, score 0.84 (cross-cluster)) — L2 PairingActivity Farming dormancy reconciliation

<!-- linkages:end -->
## Log

- 2026-05-12: opened on the back of ticket 300's pre-flight side-finding. Empirical anchor: `logs/tuned-42/` (soak) vs `logs/sweep-baseline-halving-ward-placement-candidate-step-from-5-to-2-lets-the-s/42-1/` (sweep) — same commit `5fedc33b` dirty, same binary timestamp `2026-05-12 17:42`, divergent Starvation count.
