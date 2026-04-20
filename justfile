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
soak SEED="42":
    mkdir -p logs/tuned-{{SEED}}
    cargo run --release -- --headless --seed {{SEED}} --duration 900 \
      --log logs/tuned-{{SEED}}/narrative.jsonl \
      --event-log logs/tuned-{{SEED}}/events.jsonl

# Run the canary checks (starvation, shadowfox, activation, wipeout)
# against an events.jsonl. Exits non-zero on any failure. Default target
# is the latest tuned run.
check-canaries LOGFILE="logs/events.jsonl":
    scripts/check_canaries.sh {{LOGFILE}}

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

# Check + clippy
check:
    cargo check && cargo clippy -- -D warnings

# Generate a random template authoring prompt
template-prompt:
    cargo run --example template_prompt

# Audit narrative template coverage across action × mood × weather × season
template-audit:
    cargo run --example template_audit

# Inspect a cat's personality and decision history from the event log
inspect name *ARGS:
    cargo run --example inspect_cat -- {{name}} {{ARGS}}

# Run multi-seed benchmark and record scores against current changeset
score-track *ARGS:
    uv run scripts/score_track.py {{ARGS}}

# Compare scores between recent changesets
score-diff *ARGS:
    uv run scripts/score_diff.py {{ARGS}}

# Generate balance report from most recent diagnostic run
balance-report *ARGS:
    uv run scripts/balance_report.py {{ARGS}}

# Generate game wiki and build mdBook site
wiki:
    uv run scripts/generate_wiki.py
    mdbook build docs/wiki

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
