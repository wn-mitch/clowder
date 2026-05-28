# Clowder

A colony sim about a clowder of cats living in a world with its own weight — honest ecology with a mythic undercurrent. *Watership Down meets Timberborn, starring cats.* **Stack:** Rust + Bevy ECS 0.18, 2D pixel-art sprites. Vision: [`docs/systems/project-vision.md`](docs/systems/project-vision.md).

## Quick reference

Read the linked doc before doing the work — these are authoritative; CLAUDE.md is the index.

- **Commands cheat sheet** — [`docs/workflow/commands.md`](docs/workflow/commands.md). Daily / verifying / balance / inspecting / parallel-sessions / skills.
- **Ticket lifecycle** — [`docs/workflow/ticket-lifecycle.md`](docs/workflow/ticket-lifecycle.md). `just land` / `just open-ticket` are the only sanctioned ways; two-axis tagging; antipattern-migration follow-ons.
- **Parallel sessions / polecats / refinery** — [`docs/workflow/parallel-sessions.md`](docs/workflow/parallel-sessions.md).
- **Design pillars** — [`docs/discipline/design-pillars.md`](docs/discipline/design-pillars.md). The four load-bearing rules — see one-liners below.
- **Bugfix discipline** — [`docs/discipline/bugfix.md`](docs/discipline/bugfix.md). Structural-option menu, layer-walk audit, reframe discipline, scenario-before-soak, sub-agent dispatch, open-time prefill.
- **Verification** — [`docs/discipline/verification.md`](docs/discipline/verification.md). `just verdict` is the one-call gate. Hard survival gates + continuity canaries + drift hypothesis (4-artifact methodology).
- **Conventions** — [`docs/conventions/`](docs/conventions/) — substrate-stubs, silent-canary, compile-time-contracts, commits-and-vcs. CI scripts under `just check` enforce these.
- **GOAP resolver contract** — [`docs/systems/goap-resolver-contract.md`](docs/systems/goap-resolver-contract.md). Five required rustdoc headings, never-fired canary.
- **ECS rules (Bevy 0.18)** — [`docs/systems/ecs-rules.md`](docs/systems/ecs-rules.md). Messages-not-Events, default-to-event-driven, 16-param limit, query disjointness.
- **AI substrate refactor (major in-flight)** — spec [`docs/systems/ai-substrate-refactor.md`](docs/systems/ai-substrate-refactor.md) (§4.7 substrate-vs-search-state required reading before any substrate-migration ticket); status [`docs/open-work/tickets/060-ai-substrate-refactor-epic.md`](docs/open-work/tickets/060-ai-substrate-refactor-epic.md).

## Architecture (load-bearing context, always)

- **Utility AI + GOAP.** Cats score per-tick (`src/ai/scoring.rs`); winning disposition drives the GOAP planner (`src/systems/goap.rs`) that sequences `resolve_*` steps under `src/steps/`. No behavior trees, no LLMs.
- **Maslow needs.** 5 tiers (physiological → self-actualization); lower tiers suppress higher when critical. ("Tier 1..5" is Maslow rank — distinct from the AI substrate's `L1/L2/L3` shorthand at [`docs/systems/ai-substrate-refactor.md:551`](docs/systems/ai-substrate-refactor.md), which names markers / DSE scoring / softmax layers.)
- **Ecological-magical-realist world.** Magic, fate, the Calling, wards, corruption are *ecological phenomena with metaphysical weight* — tune as part of the ecosystem, not an unlockable layer.
- **No director.** No difficulty scaling, no out-of-fiction storyteller. (In-fiction coordinator cats *can* issue directives; those are perceivable substrate that recipients score and may refuse — not a thumb on the scale.) Seasons / weather / corruption *are* the event generator.

## Design pillars (one-liners — expansion + precedents in linked doc)

Four load-bearing rules that decide *which kind of fix is allowed* before parameter tuning is on the table. Each has a "ruined us" precedent. See [`docs/discipline/design-pillars.md`](docs/discipline/design-pillars.md).

1. **Items are real, and items have bite.** Items are spatial world entities with real physical constraints — never abstract resources. Effects are identity/material-keyed and compose through a uniform modifier-aggregation layer that surfaces in the L2/resolver trace. No random affixes, no `+5`-style stat sticks.
2. **Substrate over hacks.** Prefer substrate-side levers (DSE axes, considerations, markers, eligibility filters, scoring shape visible in the L2 trace) over hidden side-channels. Substrate axes land first, the corresponding hack retires second — never the reverse.
3. **Richer perception, better strategy.** Decompose signals into orthogonal axes that each encode a distinct situation, not a louder single alarm. Compose personality / phobias / ambient context at the modifier layer, never inside the underlying perception scalar.
4. **Commitment is one mechanism, not two.** §L2.10.6 softmax + §7.4 persistence-bonus is the single commitment layer. HTN methods (decomposition) are fine; frame-pins (parallel commitment) are not. If the L2 trace doesn't show the held Intention's persistence-bonus offset, the encoding is wrong.

## Tactical inline (changes affect every edit)

### Bevy / Rust quirks

- **Messages, not Events** — `#[derive(Message)]`, `MessageWriter<T>` / `MessageReader<T>`, `app.add_message::<T>()` in `SimulationPlugin::build()`. Names are verbs (`SpawnCat`), not `*Event`.
- **Default event-driven; justify per-tick.** Only plan exec, sense+score, decay, and physics belong per-tick. Everything else fires on a Message against cached state. Precedent: ticket 431.
- **Bevy 16-param limit** — bundle in `#[derive(SystemParam)]` structs; preferred over `Option<Res<T>>` hacks.
- **Query disjointness** — splitting `Query<&mut C>` by marker: pair `With<M>` and `Without<M>` against siblings.
- Prefer `run_if` over early returns. Never `.clone()` resource data in per-tick systems — borrow via `Res<T>` / `ResMut<T>`.

Full rules: [`docs/systems/ecs-rules.md`](docs/systems/ecs-rules.md).

### Tuning constants

All knobs in `src/resources/sim_constants.rs` (`#[derive(Resource)]`; **no inline magic numbers**). The full struct serializes into the `events.jsonl` header — that's the comparability invariant. `just explain <constants.path>` shows doc-comment + current value + every read-site + (if `rebuild-sensitivity-map` was run) Spearman rho per metric.

### Rendering landmine

Tilemap uses plain Bevy `Sprite` entities — **NOT `TilemapBundle`**. bevy_ecs_tilemap's GPU pipeline silently renders all tiles as texture index 0 on macOS Metal. Base terrain at z=0, autotile overlays at z=1/2/3. F6/F7/F8 toggle overlay visibility.

### Verification one-liner

Always run `just verdict <run-dir>` after a soak. Hard gates: `Starvation == 0`, `ShadowFoxAmbush <= 10`, footer line written, `never_fired_expected_positives == 0`. Continuity canaries (each ≥1): grooming · play · mentoring · courtship. Full doc: [`docs/discipline/verification.md`](docs/discipline/verification.md).

### Footer rate arithmetic

Divide by `elapsed_ticks`, **never** `final_tick`. Runs start at absolute ~1.2M; dividing by `final_tick` under-counts by ~13.6×. Use `just q run-summary` for the rate column.

## Conventions (one-liners — full rules under [`docs/conventions/`](docs/conventions/))

- **Conventional commits**, no scopes (`feat:` / `fix:` / `chore:` / `refactor:` / `test:` / `docs:`).
- **Solo-to-main**: commits push to main directly; feature branches optional. Global `wnmitch/<name>` convention does not apply here.
- **VCS: jj, not raw git.** In multi-workspace parallel sessions, `@` is *this* workspace and `<name>@` entries belong to other sessions — don't modify them.
- **Substrate stubs forbidden** — every marker ships with reader+writer in the same commit; every DSE-`.require`'d marker ships with a `MarkerSnapshot::set_*` call in `evaluate_and_plan`; every `impl InfluenceMap` ships with a `populate_influence_map_registry` call. Enforced by three CI scripts via `just check`. See [`docs/conventions/substrate-stubs.md`](docs/conventions/substrate-stubs.md).
- **Silent-canary surfaces forbidden** — classifier functions for silent-failure detection (`expected_to_fire_per_soak`, `category`, `feature_name`) use exhaustive `match`, no catch-all arms. Iteration arrays parallel to an enum need coverage tests; multi-file enum-variant cases prefer `linkme::distributed_slice`. See [`docs/conventions/silent-canary.md`](docs/conventions/silent-canary.md).
- **Prefer compile-time contracts to runtime checks.** Cost hierarchy: trait/exhaustive-match/distributed-slice (zero ongoing cost) → CI script → runtime divergence (silent failure, weeks of investigation). See [`docs/conventions/compile-time-contracts.md`](docs/conventions/compile-time-contracts.md).
- **Design docs** — `docs/systems/`, one stub per tunable system. Auto-generated status: `docs/wiki/systems.md` (`just wiki` regenerates after `SimulationPlugin::build()` changes).

## When in doubt

- "How do I X?" — check [`docs/workflow/commands.md`](docs/workflow/commands.md) first.
- "Why did the colony do Y?" — `/diagnose-run` or `/diagnose-collapse` skills; never raw `grep` / `jq` on `logs/`.
- "Is this run OK?" — `just verdict <run-dir>` or `/verdict`.
- "What should I work on?" — `/work` or `just open-work-ready`.
- "Have we seen this before?" — `just similar <text-or-ticket-id>` or `/similar`.
