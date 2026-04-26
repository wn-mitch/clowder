# Run the simulation
run *ARGS:
    cargo run -- {{ARGS}}

# Run with a specific seed
seed SEED:
    cargo run -- --seed {{SEED}}

# Load from autosave
load:
    cargo run -- --load saves/autosave.json

# Run headless simulation (default 60s)
headless *ARGS:
    cargo run -- --headless {{ARGS}}

# Canonical 15-min deep-soak at a fixed seed. Release build, writes to
# logs/tuned-<seed>/{events,narrative}.jsonl. See CLAUDE.md and
# docs/diagnostics/log-queries.md for verification.
#
# Refuses to overwrite an existing logs/tuned-<seed>/events.jsonl —
# rename it to a versioned name first (e.g.
# `mv logs/tuned-42 logs/tuned-42-<suffix>`).
soak SEED="42":
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -s "logs/tuned-{{SEED}}/events.jsonl" ]; then
      echo "REFUSED: logs/tuned-{{SEED}}/events.jsonl exists." >&2
      echo "  Rename to a versioned name first, e.g.:" >&2
      echo "    mv logs/tuned-{{SEED}} logs/tuned-{{SEED}}-\$(git rev-parse --short HEAD)" >&2
      exit 2
    fi
    mkdir -p logs/tuned-{{SEED}}
    cargo run --release -- --headless --seed {{SEED}} --duration 900 \
      --log logs/tuned-{{SEED}}/narrative.jsonl \
      --event-log logs/tuned-{{SEED}}/events.jsonl

# Run the canary checks (starvation, shadowfox, activation, wipeout)
# against an events.jsonl. Exits non-zero on any failure. Default target
# is the latest tuned run.
check-canaries LOGFILE="logs/events.jsonl":
    scripts/check_canaries.sh {{LOGFILE}}

# Run the continuity-canary checks (grooming, play, mentoring, burial,
# courtship, mythic-texture) against an events.jsonl footer. Exits
# non-zero when any canary class fired zero times in the soak.
# Continuity canaries gate behavioural range — substrate refactor
# Phase 3+ requires them to strengthen (not just non-regress).
check-continuity LOGFILE="logs/events.jsonl":
    scripts/check_continuity.sh {{LOGFILE}}

# Query tools over a sim run's JSONL logs. Wraps the jq recipes in
# docs/diagnostics/log-queries.md as seven parameterized subtools with
# a consistent envelope (query echo, scan stats, stable IDs, narrative,
# next-query hints). Intended as an agent-friendly drill-down surface,
# complementing /diagnose-run's fixed-shape report.
#
# Subtools: run-summary | events | deaths | narrative | trace |
#           cat-timeline | anomalies
# See `just q <subtool> --help` for flags, or .claude/skills/logq/SKILL.md.
#
# Examples:
#   just q run-summary logs/tuned-42
#   just q deaths logs/tuned-42 --cause=Starvation
#   just q trace logs/tuned-42 Simba --layer=L3
#   just q anomalies logs/tuned-42
#   just q cat-timeline logs/tuned-42 Simba --tick-range=3800..4000
q *ARGS:
    python3 scripts/logq/logq.py {{ARGS}}

# Run logq's envelope + subtool tests (stdlib unittest, no pytest dep).
# Runs the file directly because `unittest discover` requires the
# tests/ dir to be a Python package (which would mean adding an
# __init__.py and polluting the Rust test layout).
test-logq:
    python3 tests/logq/test_envelope.py -v

# Deep-soak with a focal-cat trace sidecar. Writes to
# logs/tuned-<seed>/{events,narrative,trace-<focal>}.jsonl. Trace
# records decompose per-tick L1/L2/L3 state for one focal cat per §11
# of docs/systems/ai-substrate-refactor.md.
soak-trace SEED="42" FOCAL_CAT="Simba":
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -s "logs/tuned-{{SEED}}/events.jsonl" ] || [ -s "logs/tuned-{{SEED}}/trace-{{FOCAL_CAT}}.jsonl" ]; then
      echo "REFUSED: logs/tuned-{{SEED}}/ already has soak-trace output." >&2
      echo "  Rename to a versioned name first, e.g.:" >&2
      echo "    mv logs/tuned-{{SEED}} logs/tuned-{{SEED}}-\$(git rev-parse --short HEAD)" >&2
      exit 2
    fi
    mkdir -p logs/tuned-{{SEED}}
    cargo run --release -- --headless --seed {{SEED}} --duration 900 \
      --focal-cat {{FOCAL_CAT}} \
      --log logs/tuned-{{SEED}}/narrative.jsonl \
      --event-log logs/tuned-{{SEED}}/events.jsonl \
      --trace-log logs/tuned-{{SEED}}/trace-{{FOCAL_CAT}}.jsonl

# Pairwise frame-diff over two focal-cat traces, with optional
# hypothesis overlay from a phase balance doc. Emits per-DSE drift
# stats ranked by |Δ mean| and an overall concordance verdict.
#
# Example:
#   just frame-diff logs/baseline-pre-substrate-refactor/trace-Simba.jsonl \
#                   logs/tuned-42/trace-Simba.jsonl \
#                   docs/balance/substrate-phase-3.md
frame-diff BASELINE NEW HYPOTHESIS="":
    #!/usr/bin/env bash
    if [[ -n "{{HYPOTHESIS}}" ]]; then
        uv run scripts/frame_diff.py {{BASELINE}} {{NEW}} --hypothesis {{HYPOTHESIS}}
    else
        uv run scripts/frame_diff.py {{BASELINE}} {{NEW}}
    fi

# One-call run validation. Composes check-canaries + check-continuity +
# diff-constants + footer-vs-baseline drift into a structured JSON
# envelope (or a human summary with --text). Exit codes:
#   0 = pass, 1 = concern, 2 = fail.
# Reads the active baseline from logs/baselines/current.json; falls back
# to logs/baseline-pre-substrate-refactor/events.jsonl. Pass --baseline
# to override.
#
# Examples:
#   just verdict logs/tuned-42
#   just verdict logs/tuned-42 --text
#   just verdict logs/tuned-42 --baseline logs/baseline-2026-04-25/events.jsonl
verdict *ARGS:
    uv run scripts/verdict.py {{ARGS}}

# Run a balance hypothesis end-to-end: baseline + treatment sweeps,
# concordance check, draft balance doc. Formalizes the four-artifact
# methodology (hypothesis / prediction / observation / concordance).
# Treatment runs read CLOWDER_OVERRIDES env var; the binary's
# SimConstants::from_env() applies the patch at boot, no rebuild.
#
# Spec template: docs/balance/hypothesis-template.yaml.
# Exit codes: 0 concordant, 1 inconclusive/off-magnitude, 2 wrong-direction.
#
# Examples:
#   just hypothesize docs/balance/my-hypothesis.yaml
#   just hypothesize SPEC --duration 60 --seeds 42 --reps 1   # smoke test
#   just hypothesize SPEC --text                              # human summary
hypothesize *ARGS:
    uv run scripts/hypothesize.py {{ARGS}}

# Per-metric "is this run in band?" verdict against
# docs/balance/healthy-colony.md. Pure sense-making companion to
# `just verdict` — emits per-field verdict (in-range / low / high /
# below-floor / above-cap) so a non-game-dev can read a soak.
#
# Exit codes: 0 every metric in band, 1 concerns, 2 failures (continuity
# tally at 0, starvation > 0, etc.).
#
# Examples:
#   just fingerprint logs/tuned-42
#   just fingerprint logs/tuned-42 --text
fingerprint *ARGS:
    uv run scripts/fingerprint.py {{ARGS}}

# Explain a SimConstants field: doc comment, current value (read from a
# recent events.jsonl header), every read site in src/, and (if Tier 4.2
# sensitivity map exists) per-metric Spearman rho. Resolves dotted paths
# (e.g. `magic.ward_decay_per_tick`).
#
# Examples:
#   just explain magic.ward_decay_per_tick
#   just explain fulfillment.social_warmth_socialize_per_tick --text
#   just explain --list                                # every constant path
explain *ARGS:
    uv run scripts/explain_constant.py {{ARGS}}

# Promote a soak directory to a named first-class baseline. Writes
# logs/baselines/<label>.json + (unless --no-current) updates
# logs/baselines/current.json so `just verdict` auto-reads it as the
# active baseline. Run this after a soak you want as a checkpoint.
#
# Examples:
#   just promote logs/tuned-42 post-state-trio
#   just promote logs/tuned-42 post-state-trio --no-current
#   just promote logs/tuned-42 post-state-trio --force
promote *ARGS:
    bash scripts/promote.sh {{ARGS}}

# Find the commit that introduced a canary regression. Builds a test
# script that rebuilds + soaks + probes the metric at each candidate;
# the loop is run via `jj edit` (or `git bisect run`) until the
# offending commit is isolated. Defaults to a 60s probe at seed 42.
#
# Examples:
#   just bisect-canary deaths_by_cause.Starvation @
#   just bisect-canary deaths_by_cause.ShadowFoxAmbush @ --threshold 10
#   just bisect-canary wards_placed_total @ --threshold 50 --duration 300
bisect-canary *ARGS:
    bash scripts/bisect_canary.sh {{ARGS}}

# Multi-seed × multi-rep headless sweep for Phase 5b balance verification.
# Writes to logs/sweep-<label>/<seed>-<rep>/{narrative,events}.jsonl.
# Defaults: 5 seeds × 3 reps = 15 runs, 4-way parallel. Requires a release
# build (run `just release-build` first if needed).
#
# Usage:
#   just sweep baseline-5b                       # natural weather
#   just sweep fog-activation-1                  # natural weather, post-activation
#   just sweep forced-fog fog                    # --force-weather fog on every run
#   just sweep custom "" "42 99" 5               # 2 seeds × 5 reps, no override
sweep LABEL FORCE_WEATHER="" SEEDS="42 99 7 2025 314" REPS="3" DURATION="900" PARALLEL="4":
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! -x target/release/clowder ]]; then
      echo "Release binary not found — building..."
      cargo build --release
    fi
    base="logs/sweep-{{LABEL}}"
    mkdir -p "$base"
    force_arg=""
    if [[ -n "{{FORCE_WEATHER}}" ]]; then
      force_arg="--force-weather {{FORCE_WEATHER}}"
    fi
    # Emit one job per (seed, rep) pair, pipe to xargs for parallel exec.
    jobs=$(mktemp)
    trap 'rm -f "$jobs"' EXIT
    for seed in {{SEEDS}}; do
      for rep in $(seq 1 {{REPS}}); do
        dir="$base/${seed}-${rep}"
        mkdir -p "$dir"
        echo "./target/release/clowder --headless --seed ${seed} --duration {{DURATION}} ${force_arg} --log ${dir}/narrative.jsonl --event-log ${dir}/events.jsonl > ${dir}/stderr.log 2>&1 && echo DONE ${seed}-${rep}" >> "$jobs"
      done
    done
    total=$(wc -l < "$jobs" | tr -d ' ')
    echo "Sweep {{LABEL}}: $total runs, {{PARALLEL}}-way parallel, ${force_arg:-natural weather}"
    # -S bumps BSD xargs's default 255-char per-substitution buffer.
    xargs -P {{PARALLEL}} -I CMD -S 4096 bash -c CMD < "$jobs"
    echo "Sweep complete — outputs in $base/"

# Capture a versioned baseline dataset under logs/baseline-<LABEL>/.
# Five-phase orchestrator (probe → aggregate sweep → focal traces →
# conditional weather → REPORT.md). Designed to be backgroundable;
# writes STATUS.txt + STATUS.json after every phase. See
# scripts/run_baseline_dataset.sh for env-var overrides
# (SEEDS, REPS, DURATION, PROBE_DURATION, PARALLEL, ALLOW_DIRTY,
# SKIP_PHASE_4).
#
# Examples:
#   just baseline-dataset 2026-04-25
#   SEEDS="42 99" REPS=1 DURATION=60 just baseline-dataset smoke
#   nohup just baseline-dataset 2026-04-25 > /tmp/baseline.log 2>&1 &
baseline-dataset LABEL:
    bash scripts/run_baseline_dataset.sh {{LABEL}}

# Render REPORT.md from an existing logs/baseline-<LABEL>/ tree without
# re-running soaks. Useful after editing baseline_report.py or for
# inspecting partial datasets that crashed mid-run.
baseline-report LABEL:
    python3 scripts/baseline_report.py --baseline-dir logs/baseline-{{LABEL}}

# Diff tuning constants between two runs. Empty diff means the runs are
# behaviorally comparable.
diff-constants BASE NEW:
    #!/usr/bin/env bash
    diff <(jq -c 'select(._header) | .constants' {{BASE}}) \
         <(jq -c 'select(._header) | .constants' {{NEW}})

# Build
build:
    cargo build

# Run tests
test:
    cargo test

# Check + clippy + step-resolver contract lint
check:
    cargo check --all-targets && cargo clippy --all-targets --all-features -- -D warnings && bash scripts/check_step_contracts.sh

# Generate a random template authoring prompt
template-prompt:
    cargo run --example template_prompt

# Audit narrative template coverage across action × mood × weather × season
template-audit:
    cargo run --example template_audit

# Inspect a cat's personality and decision history from the event log
inspect name *ARGS:
    cargo run --example inspect_cat -- {{name}} {{ARGS}}

# Per-metric statistical summary of a sweep directory, optionally
# comparing two sweeps with Welch's t / Cohen's d / effect-size bands.
# Replaces the retired `balance-report`, `score-diff`, and `sweep_compare.py`.
#
# Bands:
#   significant — |Δ| ≥ 30% AND p < 0.05 AND |d| > 0.5
#   drift       — 10% ≤ |Δ| < 30% (worth investigating)
#   noise       — |Δ| < 10%
#
# Examples:
#   just sweep-stats logs/sweep-baseline-5b
#   just sweep-stats logs/sweep-fog-activation-1 --vs logs/sweep-baseline-5b
#   just sweep-stats logs/sweep-X --text
#   just sweep-stats logs/sweep-X --charts          # opt-in matplotlib boxplots
sweep-stats *ARGS:
    uv run scripts/sweep_stats.py {{ARGS}}

# Generate game wiki and build mdBook site
wiki:
    uv run scripts/generate_wiki.py
    mdbook build docs/wiki

# Summary of open work by status (reads docs/open-work/tickets/ frontmatter)
open-work:
    #!/usr/bin/env bash
    for s in in-progress ready parked blocked; do
      n=$(rg -l "^status: $s\b" docs/open-work/tickets/ -g '!_*.md' 2>/dev/null | wc -l | tr -d ' ')
      printf "%-14s %s\n" "$s" "$n"
    done
    pe=$(ls docs/open-work/pre-existing/*.md 2>/dev/null | wc -l | tr -d ' ')
    printf "%-14s %s\n" "pre-existing" "$pe"

# List ready tickets with id and title
open-work-ready:
    #!/usr/bin/env bash
    for f in $(rg -l '^status: ready\b' docs/open-work/tickets/ -g '!_*.md' 2>/dev/null | sort); do
      id=$(rg '^id:' "$f" | head -1 | sed 's/id: *//')
      title=$(rg '^title:' "$f" | head -1 | sed 's/title: *//')
      printf "%-5s %s\n" "$id" "$title"
    done

# List in-progress tickets with id and title
open-work-wip:
    #!/usr/bin/env bash
    for f in $(rg -l '^status: in-progress\b' docs/open-work/tickets/ -g '!_*.md' 2>/dev/null | sort); do
      id=$(rg '^id:' "$f" | head -1 | sed 's/id: *//')
      title=$(rg '^title:' "$f" | head -1 | sed 's/title: *//')
      printf "%-5s %s\n" "$id" "$title"
    done

# Regenerate docs/open-work.md from per-ticket frontmatter
open-work-index:
    uv run scripts/generate_open_work.py

# Generate wiki and open in browser
wiki-serve:
    uv run scripts/generate_wiki.py
    mdbook serve docs/wiki --open

# Open the cat questionnaire in a browser
questionnaire:
    open tools/cat_questionnaire.html

# Start the Writer's Toolkit dev server (narrative editor, cat quiz,
# and simulation log dashboard). Deployed to GitHub Pages on merge to main.
narrative-editor:
    cd tools/narrative-editor && npm install --silent && npm run dev

# Open the simulation log dashboard straight to the #/logs route. Client-side
# only — drop events.jsonl / narrative.jsonl from disk to compare runs.
logs:
    cd tools/narrative-editor && npm install --silent && npm run dev -- --open /#/logs

# Open the focal-cat trace scrubber (#/trace). Drop a `trace-<name>.jsonl`
# produced by `just soak-trace <seed> <focal>` to step tick-by-tick through
# the IAUS L1/L2/L3 decision pipeline for that cat.
trace:
    cd tools/narrative-editor && npm install --silent && npm run dev -- --open /#/trace

# Build autotile atlases from Fan-tasy Tileset source images
atlas-build:
    python3 tools/build_grass_atlas.py

# Visual verification of blob autotile atlas coordinate mapping
atlas-test:
    python3 tools/verify_atlas.py

# Audit which Fan-tasy sprites are mapped vs unmapped
atlas-coverage:
    python3 tools/audit_sprites.py

# Run all checks
ci: check test

# Build optimized release binary
release-build:
    cargo build --release

# Tag and push a release (triggers GitHub Actions)
release VERSION:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Bumping to {{VERSION}}..."
    sed -i '' 's/^version = ".*"/version = "{{VERSION}}"/' Cargo.toml
    cargo check --quiet
    echo ""
    echo "Update CHANGELOG.md with the release date, then press Enter."
    read -r _
    jj commit -m "chore: release v{{VERSION}}"
    jj git push
    jj git export
    git tag "v{{VERSION}}"
    git push origin "v{{VERSION}}"
    echo "Pushed v{{VERSION}} — GitHub Actions will build and publish the release."
