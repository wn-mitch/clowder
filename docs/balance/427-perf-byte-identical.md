# 427 — per-tick allocation hygiene: byte-identical output hypothesis

## Hypothesis

All seven steps of ticket 427 (scratchpad/arena hoisting for DSE
target filters, route_cost, A* planner, Relationships, tile-occupancy
HashMaps, and `scores.clone` → `clone_from`) are **allocator
hygiene** — they change *where* memory is allocated, not *what* the
sim computes. By construction:

- L1 marker authoring: unchanged (no marker logic touched)
- L2 DSE scores: identical (per-target lookup tables hold the same
  values, just in pre-allocated buffers)
- L3 softmax selection: identical (`scores` Vec contents unchanged)
- GOAP plan: identical (A* search visits the same states in the same
  order; arena reuse preserves insertion order)
- Step resolution: identical (no step contracts touched)

## Prediction

Every per-DSE row in `frame-diff` should be `ok`:

| DSE | Predicted Δ mean(final_score) | Rationale |
|-----|------------------------------|-----------|
| All target-taking DSEs (12) | ≈ 0 (within fp noise) | Same scoring inputs from scratchpad slots |
| All cat-side DSEs | ≈ 0 (within fp noise) | No scoring code paths touched |
| Wildlife planners | ≈ 0 (within fp noise) | Same A* expansion order |

`elapsed_ticks` should be **higher** in the post-427 run vs the
pre-427 baseline (Step 0 baseline: 73035 ticks / 900s = 81.15
ticks/sec). Survey-projected improvement: 3–5% (≥ 2.5k additional
ticks per soak).

## Concordance

Direction-match: post-427 `elapsed_ticks` > 73035. Magnitude: within
~2× of the 3–5% projection. Any non-zero Δ mean on a DSE row indicates
a real bug in the scratch refactor (scoring divergence).
