# Verification

`just headless` is the canonical diagnostic; `just soak [seed]` is the canonical 15-min release deep-soak; **`just verdict <run-dir>` is the one-call gate.** Always release for verification — debug is ~4× slower.

## Log hygiene

- **Never overwrite** `logs/tuned-*/` or `logs/baseline-*/`. `just soak` and `just soak-trace` refuse, and `.claude/hooks/no-log-overwrite.py` enforces.
- Line 1 of `events.jsonl` is a header with seed + commit + full `SimConstants` + `start_tick`. Runs are only comparable iff their headers match on `constants` and carry the same non-dirty `commit_hash`.
- **Ticks on disk are absolute, never zero-based.** Every run begins at `start_tick = 60 × ticks_per_season ≈ 1,200,000` so founder cats can have varied ages. Rationale: `src/plugins/setup.rs:297-301`, `docs/balance/activation-1-status.md`.
- jq recipes for ad-hoc queries: `docs/diagnostics/log-queries.md`.

## Hard survival gates

Must pass on the canonical seed-42 deep-soak:

- `deaths_by_cause.Starvation == 0`
- `deaths_by_cause.ShadowFoxAmbush <= 10`
- Footer line written
- `never_fired_expected_positives == 0`

## Continuity canaries

Each ≥1 per soak; collapse means survival lock:

- `grooming`
- `play`
- `mentoring`
- `courtship`

Generational continuity tracked via `KittenMatured` in the activation block.

**Demoted from the canary set:**

- **`burial`** — ticket 250 demoted because post-247 / 248 substrate stability makes deaths (and therefore burials) genuinely rare in healthy colonies. The footer tally still records burials when they happen.
- **`mythic-texture`** — ticket 445 demoted similarly. Contributing events (`EventKind::ShadowFoxBanished` requires posse-driven combat below banish threshold; `EventKind::MythicTexture` is not yet wired by any emitter) are rare-legend. The `BondFormed` / `Adopted` named events that would carry the canary in a healthy colony are blocked on 403/404. The footer tally still records mythic events when they happen; re-enroll when 403/404 wire `EventKind::MythicTexture` into adoption events.

## Drift threshold

Drift > ±10% on a characteristic metric requires a hypothesis `{ecological/perceptual fact} ⇒ {predicted direction + magnitude}` and four artifacts:

1. **Hypothesis** — the ecological/perceptual fact.
2. **Prediction** — direction + magnitude.
3. **Observation** — what the sweep returned.
4. **Concordance** — direction match + magnitude within ~2×.

`just hypothesize <spec.yaml>` runs this end-to-end. Drift > ±30% needs additional scrutiny. Survival canaries are hard gates regardless.

## "A refactor that changes sim behavior is a balance change"

Append iterations to the existing `docs/balance/*.md` thread. Doctrine: `docs/balance/*.md`.

## Common pitfalls

- **Footer rate arithmetic** — divide by `elapsed_ticks`, never `final_tick`. Runs start at ~1.2M; dividing by `final_tick` under-counts by ~13.6×. Use `just q run-summary` for the rate column. (Memory: `feedback_footer_rate_arithmetic`.)
- **Binary commit is truth** — `commit_hash` in events.jsonl header is canonical; directory names / filenames / balance-doc cells drift across `jj amend` workflows. `just frame-diff`'s "advisory" warnings on cross-commit are silent failures. (Memory: `feedback_binary_commit_is_truth_not_label`.)
- **Soak pre-flight** — verify the focal cat exists in the seed's roster (grep events.jsonl) and `cargo build --release` after any commit; `build.rs::GIT_DIRTY` is compile-time-baked and stale binaries produce dirty headers on clean trees. (Memory: `feedback_soak_pre_flight`.)
