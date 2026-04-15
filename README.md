# Clowder

Cat colony simulation set in a Redwall-inspired fantasy world. Built with Rust and [Bevy](https://bevyengine.org/) 0.18, rendered as 2D pixel-art sprites.

## Features

- **Utility AI** — cats score actions each tick using a Maslow hierarchy of needs (physiological through self-actualization)
- **Personality** — 18-axis trait system (drives, temperament, values) plus zodiac signs shape behavior
- **Social bonds** — relationship formation, decay, and personality-driven friction between cats
- **Weather & seasons** — seasonal transitions that affect mood, activity, and the world
- **Wildlife** — predators, prey, and herbs with ecosystem dynamics
- **Combat** — injury, morale, and ward defense systems
- **Magic & corruption** — spellcasting with misfire mechanics
- **Narrative** — RON-based template system generating prose from mood, weather, season, and action context
- **Building** — construction with resource gathering and multi-step task chains
- **Day/night cycle** — ambient lighting shifts through day phases
- **Save/load** — JSON persistence with autosave
- **Headless mode** — run simulations without graphics for benchmarking and analysis

## Requirements

| Tool | Required for | Install |
|------|-------------|---------|
| **Rust** (stable) | Building | [rustup.rs](https://rustup.rs/) |
| **just** | Task runner | `cargo install just` |
| **uv** | Python analytics scripts | [docs.astral.sh/uv](https://docs.astral.sh/uv/) |
| **Python 3.10+** | Analytics scripts (managed by uv) | — |
| **mdbook** | Wiki generation only | `cargo install mdbook` |

### Linux system dependencies

Bevy requires several system libraries on Linux:

```sh
sudo apt-get install -y \
  libasound2-dev \
  libudev-dev \
  pkg-config \
  libwayland-dev \
  libxkbcommon-dev
```

macOS and Windows need no additional system packages.

## Quick start

```sh
git clone https://github.com/wn-mitch/clowder.git
cd clowder
just run          # launch with a random seed
just seed 42      # launch with a deterministic seed
```

## CLI flags

Pass flags after `--` when using cargo directly, or via `just run`:

```sh
just run --seed 42 --log game.log
```

| Flag | Default | Description |
|------|---------|-------------|
| `--seed N` | random | Deterministic RNG seed (u64) |
| `--load PATH` | — | Load a save file |
| `--headless` | off | Run without graphics |
| `--duration N` | 600 | Headless run duration in seconds |
| `--log PATH` | — | Write game log to file |
| `--load-log PATH` | — | Load event log from a previous run |
| `--event-log PATH` | — | Write structured event log for analysis |
| `--test-map` | off | Use a fixed test map instead of procedural generation |
| `--trace-positions N` | 0 | Trace position updates for N entities |
| `--snapshot-interval N` | 100 | Interval between world snapshots |

## Controls

| Key | Action |
|-----|--------|
| `Esc` | Close open panel, or quit |
| `P` | Pause / unpause |
| `]` | Cycle simulation speed |
| `F6` / `F7` / `F8` | Toggle tilemap overlay visibility |
| Click | Inspect entity |

## Just recipes

Run `just --list` to see all recipes. Grouped reference below.

### Simulation

| Recipe | Description |
|--------|-------------|
| `just run [ARGS]` | Run the simulation, forwarding any CLI flags |
| `just seed N` | Run with a specific numeric seed |
| `just load` | Load from `saves/autosave.json` |
| `just headless [ARGS]` | Run without graphics (default 600s) |

### Build & test

| Recipe | Description |
|--------|-------------|
| `just build` | Cargo build (dev profile) |
| `just check` | `cargo check` + `cargo clippy -- -D warnings` |
| `just test` | `cargo test` |
| `just ci` | Run `check` then `test` |
| `just release-build` | Optimized release binary |

### Analytics

These recipes require `uv` and Python 3.10+.

| Recipe | Description |
|--------|-------------|
| `just score-track [ARGS]` | Run multi-seed benchmarks, append results to `logs/score_history.jsonl` |
| `just score-diff [ARGS]` | Compare benchmark scores between changesets |
| `just balance-report [ARGS]` | Generate per-cat balance charts from a diagnostic run |

### Content tools

| Recipe | Description |
|--------|-------------|
| `just template-prompt` | Generate a random narrative template authoring prompt |
| `just template-audit` | Audit template coverage across action, mood, weather, and season |
| `just inspect NAME [ARGS]` | Inspect a cat's personality and decision history from the event log |
| `just questionnaire` | Open the cat personality questionnaire in a browser |

### Wiki

Requires `uv` and `mdbook`.

| Recipe | Description |
|--------|-------------|
| `just wiki` | Generate game wiki and build the mdBook site |
| `just wiki-serve` | Generate wiki and open in browser with live reload |

### Release

| Recipe | Description |
|--------|-------------|
| `just release VERSION` | Bump `Cargo.toml` version, commit, tag, and push — triggers the GitHub Actions release workflow that builds binaries for Linux, macOS (ARM + Intel), and Windows |

## License

[PolyForm Noncommercial 1.0.0](LICENSE) — free for personal, educational, and nonprofit use.
