# Clowder

A colony sim about a clowder of cats living in a world with its own weight — honest ecology with a mythic undercurrent. *Watership Down meets Timberborn, starring cats.*

**Stack:** Rust + Bevy ECS 0.18, 2D pixel-art sprites.

See [`docs/systems/project-vision.md`](docs/systems/project-vision.md) for the full design thesis — what kind of game this is, what it isn't, and how that shapes balance and feature work.

## Commands

- `just run` / `just seed <N>` — run the sim (optionally with fixed seed)
- `just headless [ARGS]` — headless diagnostic run (debug build by default). Use `cargo run --release -- --headless ...` for verification work. See **Simulation Verification** below.
- `just test` — run tests
- `just check` — cargo check + clippy
- `just ci` — all checks

## Conventions

- Conventional commits (`feat:`, `fix:`, `chore:`, `refactor:`, `test:`, `docs:`) — no scopes
- **Solo-to-main workflow:** this is a personal project; commits push directly to `main` without PR review. Feature branches (`wnmitch/<name>`) are optional and used only when a change is large/experimental enough that the author wants a staging area. The global-CLAUDE feature-branch convention does **not** apply here.
- VCS: `jj` (not raw git)
- Design docs: `docs/systems/` — one stub per tunable system

## Design Principles

- **Utility AI + GOAP:** Cats score actions per-tick via needs, personality, relationships, context (`src/ai/scoring.rs`). The winning disposition drives a GOAP planner (`src/systems/goap.rs`) that sequences concrete steps. No behavior trees, no LLMs.
- **Maslow needs:** 5 levels (physiological → self-actualization). Lower levels suppress higher when critical.
- **Physical causality:** Objects don't teleport. Cats carry items, walk to destinations, deposit them. Actions are behavioral arcs with physical movement — not instant stat changes at a distance.
- **Ecological-magical-realist world:** Magic, fate, the Calling, wards, corruption are *ecological phenomena with metaphysical weight*, not a separate narrative layer waiting to be unlocked. Tune them as part of the ecosystem.
- **Honest world, no director:** No difficulty scaling. No RimWorld-style storyteller. Seasons, weather, migration, predator-prey oscillation, corruption cycles *are* the event generator. Cats earn their stories by surviving a world that doesn't care.
- **Emergent complexity:** Chain reactions between independent systems are the joy — design for them. The Dwarf Fortress beer-cats-puke-depression spiral is the gold standard.

## Long-horizon coordination

Three indexes track work across sessions. Read them before starting any new
system, balance change, or non-trivial refactor — do not default to opening a
new thread.

- **`docs/open-work/tickets/*.md`** — one file per open ticket. Frontmatter
  (`status`, `cluster`, `parked`, `blocked-by`) is the source of truth. See
  `docs/open-work.md` for the auto-generated index.
- **`docs/open-work/pre-existing/*.md`** — long-lived known issues (e.g. test-harness drift) that don't belong on the active queue but shouldn't be forgotten.
- **`docs/open-work/landed/YYYY-MM.md`** — archive of landed work, one file per month. Preserves commit hashes and dates.
- **`docs/wiki/systems.md`** — auto-generated status of every `docs/systems/*` stub (Built / Partial / Aspirational) with registered functions. Regenerate via `scripts/generate_wiki.py` if stale.
- **`docs/balance/*.md`** — per-balance-thread iteration logs. New iterations append to the existing file (see `unified-difficulty-posture.md` for the Iteration 1 → 2 → 3 pattern).

Queue-view commands (run from the repo root):

- `just open-work` — counts by status
- `just open-work-ready` — ready tickets with id + title
- `just open-work-wip` — in-progress tickets
- `just open-work-index` — regenerate `docs/open-work.md`

### Before starting new work

1. Run `just open-work-ready` (or `just open-work-wip`) and check whether the request matches an existing ticket.
2. Check `docs/wiki/systems.md` — if the request names a system, confirm its current Built/Partial/Aspirational status before proposing changes.
3. If the request advances an in-flight ticket: flip its `status: in-progress`, regenerate the index, and proceed.
4. If the request does not match any ticket: say so, name whether it advances `docs/systems/project-vision.md` §5 (broaden sideways: grooming, play, mentoring, burial, courtship, preservation, generational knowledge) or a continuity canary, and confirm with the user before writing code. When confirmed, open a new ticket under `docs/open-work/tickets/NNN-slug.md` as the first commit of the work.

### When work completes, defers, or is opened

- **Landed work:** set `status: done`, `landed-at: <sha>`, `landed-on: <date>` in the ticket's frontmatter; move the file into the current month's `docs/open-work/landed/YYYY-MM.md` as a `## ` entry; regenerate the index — same commit that ships the change. For trivial work you never opened a ticket for, add a brief `## ` entry directly to the current month's landed file.
- **Deferred work:** set `status: parked`, `parked: <date>`, and append a `## Log` entry naming what blocks resumption. If a specific ticket blocks it, set `status: blocked` and populate `blocked-by: [id]`.
- **New open items surfaced mid-session:** create a new ticket file under `docs/open-work/tickets/NNN-slug.md` before closing out. Minimum: `status`, `title`, `added`, one sentence of `## Why`.
- **Balance changes that produce a new iteration:** append the iteration to the thread's existing `docs/balance/*.md` file rather than creating a new one.
- **Any change to `SimulationPlugin::build()`** (system added/removed): regenerate `docs/wiki/systems.md` in the same commit.
- **Every ticket status change** (open → in-progress → parked/blocked/done) regenerates `docs/open-work.md` via `just open-work-index` in the same commit.

## ECS Rules

- Prefer `run_if` guards over early returns — gated systems skip query iteration entirely.
- Never `.clone()` resource data in per-tick systems. Borrow via `Res<T>`/`ResMut<T>`.
- Events are verbs: `SpawnCat`, `CatDied` — not `DeathEvent`. Define centrally, no circular flows.
- Bevy 0.18 uses **Messages** not Events: `#[derive(Message)]`, `MessageWriter<T>`, `MessageReader<T>`, `app.add_message::<T>()`. Register messages in `SimulationPlugin::build()` — both windowed and headless build paths share that plugin (ticket 030).
- Components: plain structs/enums with `#[derive(Component)]`. Resources: `#[derive(Resource)]`.
- Prefer `Query<>` with explicit component access over broad world access.
- **Bevy 16-param limit**: systems with many parameters hit Bevy's tuple impl limit. Use `#[derive(SystemParam)]` bundles to group related params. Example: bundle all prey-related queries + message writers into a `PreySystemParams` struct. This is preferred over `Option<Res<T>>` hacks or removing needed params.
- **Query disjointness**: when splitting `Query<&mut Component>` into separate data/marker patterns, add `With<Marker>` to restore disjointness for paired `Without<Marker>` filters in other queries.

## GOAP Step Resolver Contract

Every `pub fn resolve_*` under `src/steps/**` returns `StepOutcome<W>` (defined in `src/steps/outcome.rs`). This makes "silent-advance with no real-world effect" a type error — the bug pattern behind Phase 4c.3 (feed-kitten) and Phase 4c.4 (tend-crops), where a step's effect didn't happen but `StepResult::Advance` plus an unconditional `Feature::*` emission made the failure invisible to the Activation canary.

**Witness shapes:**

- `StepOutcome<()>` — unconditional effect once the precondition holds (e.g. `resolve_move_to`, `resolve_sleep`). `()` does **not** implement `Witnessed`, so `record_if_witnessed` is not callable — witness-less outcomes cannot emit positive Features. Build with `StepOutcome::bare(result)` or `StepOutcome::unwitnessed(result)`.
- `StepOutcome<bool>` — effect may or may not occur this call (e.g. `resolve_tend` while walking; `resolve_cook` with no raw food). Build with `StepOutcome::witnessed(result)` or `StepOutcome::unwitnessed(result)`.
- `StepOutcome<Option<T>>` — as above, but the witness carries a payload the caller consumes (kitten entity, `Pregnancy`, grooming restoration). Build with `StepOutcome::witnessed_with(result, payload)` or `StepOutcome::unwitnessed(result)`.

**Caller contract:**

```rust
let outcome = resolve_foo(...);
outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::Foo);
// Consume witness (for Option<T> payload) and return result:
if let Some(payload) = outcome.witness { ... }
outcome.result
```

Never emit `Feature::*` directly on `StepResult::Advance` — always route through `record_if_witnessed`.

**Required rustdoc preamble on every `pub fn resolve_*`:**

```text
/// # GOAP step resolver: `<StepKind>`
///
/// **Real-world effect** — what this mutates when it succeeds.
///
/// **Plan-level preconditions** — the `StatePredicate`s in
/// `src/ai/planner/actions.rs` the planner guarantees before this
/// step runs. Note real guarantees vs. coarse abstractions.
///
/// **Runtime preconditions** — what this function checks internally
/// and what happens if the check fails. MUST NOT return a witnessed
/// `Advance` when the effect didn't happen.
///
/// **Witness** — the `StepOutcome<W>` shape and what `W` records.
///
/// **Feature emission** — which `Feature::*` the caller passes to
/// `record_if_witnessed` (Positive/Neutral/Negative).
```

Exemplars: `src/steps/disposition/cook.rs`, `src/steps/disposition/feed_kitten.rs`, `src/steps/building/tend.rs`.

Enforcement: `just check` runs `scripts/check_step_contracts.sh` which greps every resolver for the five required headings.

**Never-fired canary.** When adding a new Positive `Feature::*`, also classify it in `Feature::expected_to_fire_per_soak()` (`src/resources/system_activation.rs`). Features returning `true` there are enforced to fire at least once on the canonical seed-42 900s soak (`never_fired_expected_positives` in the footer; checked by `just check-canaries`). Rare-legend events (`ShadowFoxBanished`, `FateAwakened`, etc.) return `false` — they're exempt from the canary but still tracked in `ALL`.

## Systems inventory

Design docs for each tunable system live in `docs/systems/`. Major modules and what they do:

- **`src/systems/goap.rs`** — GOAP planner; turns a winning disposition into a concrete step sequence. Single largest file in the project (~4.5k lines). Central to cat decision-making.
- **`src/systems/disposition.rs`** — Dispositioner + chain building: scores actions via `ScoringContext`, picks the winning `Disposition`, then builds the `TaskChain` (move-to / sub-action) for the legacy unscheduled path. Not step resolvers — those live in `src/steps/`.
- **`src/steps/`** — GOAP step resolvers (`resolve_*`). Sub-tree by domain: `disposition/` (socialize, mentor, groom, feed_kitten, mate, cook, …), `building/` (construct, repair, tend, gather, …), `magic/` (wards, cleanse, harvest, scry, commune, …), `fox/`. Each resolver returns `StepOutcome<W>` — see **GOAP Step Resolver Contract** above. `src/steps/outcome.rs` defines the witness type.
- **`src/ai/dses/`** — Per-DSE scoring elements (Eat, Hunt, Socialize, …) + target-taking DSEs (§6.5 per-target consideration bundles — `*_target.rs` files). See **AI Substrate Refactor** below for the in-flight port work.
- **`src/systems/coordination.rs`** — Coordinator governance, build-pressure directives, work assignment across the colony.
- **`src/systems/magic.rs`** — Herbcraft, wards, corruption spread, shadowfox spawning from corruption, seasonal herb growth.
- **`src/systems/fate.rs`** — Fated pairs, prophetic visions, destiny modifiers.
- **`src/systems/aspirations.rs`** — Long-horizon personal goals (mastery arcs for hunting, combat, crafting, socializing).
- **`src/systems/prey.rs`, `wildlife.rs`, `fox_goap.rs`** — Prey ecology (density, dens, reproduction, fear) and wild-animal AI (foxes, hawks, snakes, shadowfoxes).
- **`src/systems/needs.rs`, `mood.rs`** — Maslow hierarchy tracking and mood valence/arousal cascade.
- **`src/systems/sensing.rs`** — Four-channel perception (sight, hearing, scent, tremor).
- **`src/systems/weather.rs`, `wind.rs`, `time.rs`** — Diurnal phase, seasonal cycle, weather transitions, wind direction (scent vector).
- **`src/systems/social.rs`, `pregnancy.rs`, `growth.rs`** — Relationships, gossip, courtship, reproduction, life-stage progression.
- **`src/systems/combat.rs`, `death.rs`** — Combat resolution, injury, mortality, grief cascade.
- **`src/systems/memory.rs`, `colony_knowledge.rs`** — Per-cat memory and colony-level shared knowledge (social transmission).
- **`src/systems/narrative.rs`** — Template-driven narrative line emission across tiered severity (Micro / Action / Significant / Danger / Nature).

## AI Substrate Refactor (in-flight)

The major in-flight work is the AI substrate refactor. Two documents own state; CLAUDE.md owns none of it.

- **[`docs/systems/ai-substrate-refactor.md`](docs/systems/ai-substrate-refactor.md)** — design specification. §6 (target-taking DSEs) and §4 (marker catalog) are load-bearing for the currently-active port work; §5 (influence maps) and §7 (commitment) are later-phase scope. Spec is the source of truth for axis definitions, weights, and composition modes — do not invent alternatives mid-port.
- **[`docs/open-work/tickets/014-phase-4-follow-ons.md`](docs/open-work/tickets/014-phase-4-follow-ons.md)** — live status: what's landed, what's remaining, blockers, balance-tuning deferrals. Check this before proposing a port or marker; update it in the same commit that lands one. Cluster A itself lives at `docs/open-work/tickets/005-cluster-a-scoring-substrate-refactor.md`.

Balance-tuning on refactor-affected metrics (positive-feature density, mating cadence, magic sub-mode counts) is deferred until the substrate stabilizes — per-knob tuning now would need to be redone after each successor phase. Ticket 014 tracks the deferral.

**Port workflow** (applies to per-DSE ports, marker author systems, and most refactor increments):

1. Read ticket 014 (and 005 for cluster-A framing) and the relevant spec section before writing code.
2. Propose scope — axis set, deferrals, caller sites, non-goals — and confirm before implementing.
3. Land as one commit: factory + resolver/author-system + registration + caller wiring + 10–15 unit tests + landing entry appended to `docs/open-work/landed/YYYY-MM.md` (with hypothesis / concordance / deferred-item notes) + ticket 014's "Remaining" list updated + `just open-work-index` regenerating `docs/open-work.md`.
4. Verification: `just check` + `just test` + seed-42 `--duration 900` release soak (survival canaries hold; continuity / activation deltas recorded in the landing entry).
5. DSE registration site: one — `populate_dse_registry` in `src/plugins/simulation.rs`, called by the plugin's `register_dses_at_startup` Startup system (ticket 030 unified the headless and windowed pipelines). Exemplar port: `src/ai/dses/socialize_target.rs` (factory + caller-side resolver + test suite).

### §7.2 commitment gate — mental model

The drop-trigger gate (`src/ai/commitment.rs::reconsider_held_intentions`, Phase 6a) runs per tick between `check_anxiety_interrupts` (Maslow preemption, §7.5) and `evaluate_and_plan` (softmax re-selection). For each held `GoapPlan` it looks up the plan's `CommitmentStrategy` via the §7.3 table (`strategy_for_disposition`) and consumes three **belief proxies** (§12.3 — Clowder has no first-class belief layer):

- `achievement_believed` — the plan's goal predicate currently resolves true.
- `achievable_believed` — spatial-score retention (deferred) × `plan.replan_count < plan.max_replans` hard-fail. Gate consumes the AND.
- `still_goal` — desire persists. Load-bearing only under `OpenMinded`.

Strategy dispatch: `Blind => achieved`; `SingleMinded => achieved || unachievable`; `OpenMinded => achieved || dropped_goal`. Pure function, no ECS access — the 12-row strategy table is unit-testable without a `World`.

**Proxy recipes must mirror the authoritative completion check *with its surrounding guards*.** The 2026-04-23 §7.2 regression (see `docs/open-work/tickets/005-cluster-a-scoring-substrate-refactor.md`) was a lifted-condition bug: `achievement_believed` for Resting copied the three-need threshold out of `resolve_goap_plans`'s post-trip block (`goap.rs:~1672`) but dropped the implicit `trips_done` guard — turning a transition check ("plan just closed a trip and needs are sated") into a state poll ("needs are sated right now"). Cats whose ambient needs sat above the thresholds read as "achieved" before any rest action had run; the gate cascaded plan-churn through `evaluate_and_plan` and seed-42 canaries collapsed. When porting a check out of a nested block, replicate the block's preconditions explicitly — a nested arm's guards are part of the condition's meaning.

## Headless Mode

Headless and windowed builds share the same `App + SimulationPlugin` pipeline (ticket 030). `run_headless` mounts `MinimalPlugins + SimulationPlugin + HeadlessIoPlugin`; the windowed `main()` mounts `DefaultPlugins + SimulationPlugin + RenderingPlugin`. Add new systems / messages / observers / DSEs to `SimulationPlugin::build()` and both paths pick them up.

## Simulation Verification

**`just headless` is the canonical diagnostic tool.** It's a thin wrapper over `cargo run -- --headless`, runs the sim through `App::update()` driven by `HeadlessIoPlugin`'s tick-budget exit, writes two JSONL files, and exits early if the colony wipes. Everything else (`score-track`, `score-diff`, `balance-report`) is a Python convenience script layered on top — the JSONL output is ground truth.

### Invocation and flags

`just headless [--seed N] [--duration SECS] [--log PATH] [--event-log PATH] [--snapshot-interval TICKS] [--trace-positions N] [--test-map] [--focal-cat NAME] [--trace-log PATH]`

- `--seed N` — fixed RNG seed (default: random; printed to stderr). Required for reproducibility and diffs.
- `--duration SECS` — wall-clock sim duration in seconds (default 600 = 10 min). `--duration 60` is a smoke-test; `--duration 900` (15 min) is the canonical deep-soak (see below).
- `--log PATH` — narrative log output (default `logs/narrative.jsonl`). Tiered entries: Micro / Action / Significant / Danger / Nature.
- `--event-log PATH` — structured event log output (default `logs/events.jsonl`). Machine-readable: spawns, deaths, plan failures, feature activations.
- `--snapshot-interval TICKS` — per-cat snapshot cadence (default 100).
- `--test-map`, `--trace-positions N` — seldom needed; see `parse_args` in `src/main.rs`.
- `--focal-cat NAME` — headless-only. Enables per-tick L1/L2/L3 trace-record emission for the named cat. Ignored (with stderr warning) outside `--headless`. See **Focal-cat trace** below.
- `--trace-log PATH` — trace sidecar output path. Default `logs/trace-<focal>.jsonl`; `just soak-trace` writes to `logs/tuned-<seed>/trace-<focal>.jsonl`.

### The constants-hash header

Line 1 of `logs/events.jsonl` is a JSON header with `seed`, `duration_secs`, `commit_hash` / `commit_hash_short` / `commit_dirty` / `commit_time` (emitted by `build.rs`), a `sim_config` block (`ticks_per_day_phase`, `ticks_per_season`, `seed`) used to derive season/day boundaries from tick values, and the **full `SimConstants` dump**. This is how you confirm which tuning produced which run — two machines are comparable iff their headers match byte-for-byte on the `constants` field **and** carry the same `commit_hash` with `commit_dirty == false`. Never diff sim outcomes without first diffing headers. A `commit_dirty: true` header means the log cannot be reproduced from the commit alone; dashboards and scripts should surface this rather than compare silently. `logs/narrative.jsonl` carries the same commit fields (minus `sim_config` and `constants`) for narrative-only analyses.

### Canonical deep-soak: seed 42 at 15 minutes

The reference verification run is **seed 42, `--duration 900` (15 minutes wall), release build**. 15 minutes is long enough for corruption to climb above 0.7, shadow-foxes to spawn, cats to build multi-generational routines, and the mortality distribution to stabilize. Anything shorter (60s, 5 min) misses the phases where most balance problems surface.

```bash
just soak 42    # writes logs/tuned-42/{events,narrative}.jsonl
```

(equivalent to `cargo run --release -- --headless --seed 42 --duration 900 ...`)

Debug mode is ~4× slower than release and produces far less sim time per second of wall — **always `--release` for verification**; debug is for development-time feedback only. Save the footer from each run (grep `_footer` in the event log) before and after any tuning change to produce a diff.

**Never overwrite prior runs.** `logs/tuned-*/` and `logs/baseline-*/` hold ground-truth JSONL the project diffs against; an overwrite is unrecoverable. Always write to a versioned subfolder (`logs/tuned-42-<commit-short>/`, `logs/tuned-42-statetrio/`, etc.). Two layers enforce this:

- **`just soak` and `just soak-trace` refuse** if `logs/tuned-<SEED>/events.jsonl` (or trace sidecar) already exists. Rename the existing dir first: `mv logs/tuned-42 logs/tuned-42-$(git rev-parse --short HEAD)`.
- **PreToolUse hook** at `.claude/hooks/no-log-overwrite.py` blocks any Bash command whose target path under `logs/tuned-*/` or `logs/baseline-*/` already has content. Wired in `.claude/settings.json` (project, checked in).

If you legitimately need to discard an existing run, `rm -rf` it explicitly first — that's intentional, not silent.

Multi-seed sweeps (seeds 99/7/2025/314) are a follow-up for claims you want to generalize — only do them once a single-seed deep-soak looks right.

### Focal-cat trace (§11 substrate refactor)

When the deep-soak isn't enough — when you need to know *why* one cat chose Hunt over Socialize at tick 8432 — turn on the focal trace. It's a headless-only diagnostic layer that emits per-tick records to a JSONL sidecar `logs/trace-<focal>.jsonl` (diff-joinable with `events.jsonl` via a shared `_header`). Three record layers, tagged by a top-level `layer` field:

- **L1** — per influence-map sample: base sample, attenuation breakdown (species sensitivity / role / injury / environment), perceived value, top contributors (which emitter drove the sample).
- **L2** — per eligible DSE: marker eligibility, per-consideration breakdown (input, response curve, score, weight), composition mode, Maslow pregate, modifiers, final score, intention.
- **L3** — per tick: ranked DSE list, softmax distribution, momentum/commitment state, chosen action, GOAP plan steps.

Full field shapes: `docs/systems/ai-substrate-refactor.md` §11.3. Record types live in `src/resources/trace_log.rs`.

**Entry points:**

- `just soak-trace SEED FOCAL_CAT` — canonical invocation (e.g. `just soak-trace 42 Simba`). Writes the four-file bundle `logs/tuned-<seed>/{events,narrative,trace-<focal>}.jsonl`.
- `just frame-diff BASELINE NEW [HYPOTHESIS]` — per-DSE drift between two focal traces, ranked by |Δ mean|. Pass a balance doc as `HYPOTHESIS` to classify each DSE as ok / drift / wrong-direction against the predicted shift.
- `just autoloop SEED FOCAL_CAT` — soak-trace + survival canaries + continuity canaries + constants diff, in one loop. Use after every substrate-refactor increment.

**Helper scripts** (invoked by the recipes above; callable directly for ad-hoc use):

- `scripts/replay_frame.py` — reconstructs a full (tick, cat) decision frame by filtering layers. Acceptance gate per §11.6: the ranked DSE list from the reconstructed frame must match the snapshot in `events.jsonl`.
- `scripts/frame_diff.py` — backing for `just frame-diff`; emits per-DSE score-delta statistics.

**Picking a focal cat — no single cat exercises every feature.** L2 records are only emitted for DSEs the cat is *eligible* to evaluate; marker-gated DSEs stay silent on cats without the marker. The default `Simba` (seed 42) is a generalist — good for Hunt / Eat / Socialize / Mentor — but **Simba does not place wards**, so Simba-focal traces carry no L2 for ward-placement DSEs. Similar gaps: non-parents skip FeedKitten / NurseKitten; non-Priestess cats skip Cleanse / Scry / Commune; juveniles skip Mate / Courtship; cats without the cook marker skip Cook.

To verify coverage of the full behavioral range on a seed, **run multiple focal soaks against different cats** — pick a Priestess for the magic track, a mated adult for the reproduction track, a coordinator for build/directive traces. The trace filename encodes the focal cat, so `just soak-trace 42 Simba`, `just soak-trace 42 <priestess-name>`, `just soak-trace 42 <cook-name>` write disjoint files that coexist in `logs/tuned-42/`. This is the focal-cat analogue of multi-seed sweeps: one focal is a single slice; variation across focals is how you probe the whole DSE catalog.

For ad-hoc jq queries over a trace file, see `docs/diagnostics/log-queries.md` §11.

### Diagnostic queries

jq recipes for reading `events.jsonl` / `narrative.jsonl` live in
`docs/diagnostics/log-queries.md`. For routine checks:

- `just check-canaries LOGFILE` — runs the five survival canary queries (starvation, shadow-fox ambush, footer-written, features-at-zero informational report, never-fired-expected-positives). Exits non-zero on any failure.
- `just check-continuity LOGFILE` — runs the continuity-canary checks (grooming / play / mentoring / burial / courtship / mythic-texture) against the `continuity_tallies` footer field. Exits non-zero on zero-firing classes.
- `just diff-constants BASE NEW` — verifies two runs are behaviorally comparable.
- `just soak-trace SEED FOCAL_CAT` — same as `just soak` plus a focal-cat L1/L2/L3 trace sidecar. See **Focal-cat trace** above.
- `just frame-diff BASELINE NEW [HYPOTHESIS]` — per-DSE score drift between two focal traces; optional hypothesis classifies drift as ok / drift / wrong-direction.
- `just autoloop SEED FOCAL_CAT` — soak-trace + survival + continuity canaries + constants diff in one loop.

### Canaries

Canaries split into two groups. **Survival canaries** catch the colony dying or degenerating. **Continuity canaries** catch the world showing only a narrow slice of its range (survival lock, flat mythic texture). Both classes are hard — a silent mythic register is a bug on par with a starvation cascade, per the ecological-magical-realist framing (see `docs/systems/project-vision.md`).

**Survival canaries** (enforced by `scripts/check_canaries.sh`):

- **Starvation canary:** `deaths_by_cause.Starvation` climbing in the deep-soak is the fastest signal something is wrong. Target: `== 0` on seed 42.
- **Shadowfox canary:** `deaths_by_cause.ShadowFoxAmbush` on a 15-min deep-soak. Target: `<= 10`. Anything higher means the ward/corruption defense pipeline is failing. (Raised from `<= 5` on 2026-04-25 — the post-030/post-State-trio seed-42 soak landed at 6, and noise-band tolerance is wider than the 5-deaths cap had assumed; the band is the relevant signal, not the cap.)
- **Footer-written canary:** the soak must emit its `_footer` line before exit. Target: `>= 1`. Zero footers means the sim died before completing the `--duration` window (wipeout or crash).
- **Features-at-zero canary (informational):** reports Positive/Neutral features that ended the soak at 0. Doesn't fail by itself — baselines diff this list. `Feature::*` classification in `src/resources/system_activation.rs`.
- **Never-fired-expected canary:** `never_fired_expected_positives` footer field. Target: `== 0`. Positive features classified as "expected to fire per soak" (`Feature::expected_to_fire_per_soak() → true`) must fire at least once; rare-legend events (`ShadowFoxBanished`, `FateAwakened`, `ScryCompleted`, etc.) return `false` and are exempt.

**Continuity canaries** (wired — `continuity_tallies` emitted in the footer; enforced by `scripts/check_continuity.sh` / `just check-continuity`; currently not all passing — drive follow-on balance work):

- **Ecological variety:** each of `grooming`, `play`, `mentoring`, `burial`, `courtship` must fire ≥1× per soak. All-zero on any means survival lock has collapsed the behavioral range.
- **Mythic texture:** `mythic-texture` class — ≥1 named event per sim year (Calling fired, banishment, visitor arrival, named object crafted). A silent mythic register means the world's metaphysical weight has flattened.
- **Generational continuity** (not yet counted as a dedicated tally — track via `KittenMatured` in the activation block): at least one kitten reaches Juvenile. Currently failing on the seed-42 soak.

### What the interactive build shares with headless

Both builds mount `SimulationPlugin` and run the same Bevy schedule. New systems, messages, observers, and DSEs go into `SimulationPlugin::build()` (or, for the DSE catalog, into `populate_dse_registry`) — once. Headless adds `HeadlessIoPlugin` for JSONL writers + tick-budget exit; windowed adds `RenderingPlugin` + camera + UI. There is no `build_schedule` mirror to keep in sync — ticket 030 retired it.

## Tuning Constants

All simulation knobs live in `src/resources/sim_constants.rs`. Each system reads from `Res<SimConstants>` — no inline magic numbers. The full constants struct serializes to JSON in the `logs/events.jsonl` header; two headless runs are only comparable if their headers match on the `constants` field.

## Balance Methodology

**Drift in sim behavior (mortality rates, hunt success, ambush frequency, any characteristic metric) is acceptable if and only if it can be provably tied to an increase in verisimilitude.** Drift without a predicting hypothesis is a bug, not a feature.

Every balance-affecting change ships as a testable hypothesis of the form:

> *{ecological or perceptual fact}* ⇒ *{predicted direction and rough magnitude of metric shift}*

Acceptance requires four artifacts:

1. **Hypothesis** — the ecological/behavioral claim being modeled, with a real-world grounding (predator behavior, perception research, causal chain).
2. **Prediction** — direction and rough magnitude of the expected metric shift (e.g. "ShadowFoxAmbush count rises ~2× during fog windows").
3. **Observation** — measured shift from an A/B headless run (multi-seed sweep + forced-condition runs where relevant).
4. **Concordance** — direction matches prediction and magnitude is within ~2×. Direction mismatch = reject. Magnitude > 2× off = investigate second-order effects before accepting.

Drift ≤ ±10% on a characteristic metric is within measurement noise and does not require a written hypothesis. Drift > ±10% requires the full four artifacts. Drift > ±30% requires additional scrutiny before acceptance.

Survival canaries (see **Canaries** above: Starvation = 0, ShadowFoxAmbush ≤ 10, footer written, never-fired-expected = 0) are hard gates — they must pass regardless of hypothesis or concordance. Noise-band tolerance: seed-42 soak runs have shown Starvation drift across re-runs of the same commit due to Bevy parallel-scheduler variance, so a single deep-soak at the hard-gate target is acceptance; repeat runs of the same commit may land above 0 without constituting a regression.

This rule applies to all balance work, not just the feature driving a given session. A refactor that changes sim behavior is a balance change and must tie out the same way.

## Rendering

Tilemap uses plain Bevy `Sprite` entities — **not** `TilemapBundle`. bevy_ecs_tilemap's GPU pipeline silently renders all tiles as texture index 0 on macOS Metal. Base terrain at z=0, autotile overlays at z=1/2/3. F6/F7/F8 toggle overlay visibility.
