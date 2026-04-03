# Run the simulation
run *ARGS:
    cargo run -- {{ARGS}}

# Run with a specific seed
seed SEED:
    cargo run -- --seed {{SEED}}

# Build
build:
    cargo build

# Run tests
test:
    cargo test

# Check + clippy
check:
    cargo check && cargo clippy -- -D warnings

# Run all checks
ci: check test
