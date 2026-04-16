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

# Start the narrative editor dev server
narrative-editor:
    cd tools/narrative-editor && npm install --silent && npm run dev

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
