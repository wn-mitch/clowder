# Clowder

A colony sim about a clowder of cats living in a world with its own weight — honest ecology with a mythic undercurrent. *Watership Down meets Timberborn, starring cats.* **Stack:** Rust + Bevy ECS 0.18, 2D pixel-art sprites. Vision: [`docs/systems/project-vision.md`](docs/systems/project-vision.md).

## Commands you should reach for

### Daily
- `just check` / `just test` / `just ci` (`check` includes step-resolver and time-unit linters)
- `just open-work` / `just open-work-ready` / `just open-work-wip`
- `just land <id>` / `just open-ticket "<title>"` — **the only sanctioned way to land or open a ticket.** `land` rewrites frontmatter, moves `tickets/NNN.md` → `landed/NNN.md`, drops the id from every dependent's `blocked-by`, auto-promotes newly-unblocked tickets to `ready`, regenerates `docs/open-work.md`. Add `--commit "<msg>"` to bundle the jj landing (saves ~7 commands), `--sha <hex>` to backfill `landed-at: pending`, `--log "<entry>"` to append a `## Log` line. `open-ticket` picks the next id, instantiates the template, fills frontmatter, and regens the index; `--bugfix` selects the bugfix template, `--cluster <name>` sets the cluster, `--blocked-by <ids>` sets `status: blocked` automatically.
- `just q <subtool> <run-dir>` — logq drill-down (`run-summary` · `events` · `deaths` · `narrative` · `trace` · `cat-timeline` · `anomalies`); reach for it whenever you ask "why did X happen in this run?"
- `just logdb-build` / `logdb-query SQL` / `logdb-shell` / `logdb-chart <recipe>` — cross-run SQL over every archive in `logs/` (DuckDB at `logs/runs.duckdb`). Use whenever the question spans seeds, commits, or archives ("MatingOccurred across iterations", "softmax mass diff between commits for Mallow", "final colony score by archive"). Complementary to logq: logq drills *into one run*, logdb compares *across many*. Heavy tables (`cat_snapshot_scores`, traces) are opt-in via `--with-scores` / `--with-traces`. Schema + chart recipes: [`docs/diagnostics/logdb.md`](docs/diagnostics/logdb.md).

### Verifying a change
- `just scenario <name>` — fast (~3s) deterministic microexperiment harness (preset cats, preloaded state). **Preferred over `just soak` for hypothesis triage** during bugfix loops; `just soak` remains for whole-colony verification once a fix is drafted. See ticket 162.
- `just soak [seed]` — canonical 15-min release deep-soak (writes `logs/tuned-<seed>/`; refuses overwrite)
- `just verdict <run-dir>` — **one-call gate; always run after a soak.** Composes canaries + continuity + constants drift + footer-vs-baseline. Exit 0/1/2 = pass/concern/fail.
- `just fingerprint <run-dir>` — per-metric in-band readout vs `docs/balance/healthy-colony.md`

### Balance work
- `just hypothesize <spec.yaml>` — runs the four-artifact methodology end-to-end (baseline + treatment sweeps + concordance check + draft balance doc)
- `just sweep <label>` — multi-seed × multi-rep headless sweep
- `just sweep-stats <dir> [--vs <other>]` — Welch's t / Cohen's d / effect-size bands
- `just promote <dir> <label>` — lock in a named baseline (`verdict` auto-reads `logs/baselines/current.json` next)
- `just bisect-canary <metric>` — find the commit that introduced a canary regression
- `just baseline-dataset <label>` — 5-phase versioned-baseline orchestrator (probe → sweep → focal traces → conditional weather → REPORT.md)
- `just rebuild-sensitivity-map` — quarterly perturbation sweep; powers `just explain`'s rho column

### Inspecting one cat / one knob
- `just inspect <name>` — cat personality + decision history from the event log
- `just explain <constants.path>` — doc-comment + current value (from a recent run header) + every read-site + (if rebuilt) Spearman rho per metric
- `just soak-trace <seed> <cat>` — focal-cat L1/L2/L3 trace sidecar (per §11 of the substrate-refactor spec). Multi-focal sweeps probe the full DSE catalog: marker-gated DSEs stay silent on cats without the marker. **Multi-focal convention (ticket 227):** when the tuning ticket targets a marker-gated or eligibility-filtered DSE, run a second `soak-trace` on a cat that satisfies the gate alongside the default generalist (Simba). Find the eligible cat name before writing the balance doc: use `just q events <run-dir> CoordinatorElected` for Coordinate; filter for `hungry_kitten_urgency > 0` presence for Caretake. Without an eligible focal cat, the gated DSE's per-cat `frame-diff` row is structurally absent (not scored zero), making per-cat verification impossible.
- `just frame-diff <baseline> <new> [hypothesis.md]` — per-DSE drift ranked by |Δ mean|; hypothesis classifies each DSE as ok / drift / wrong-direction

Also: `just logs` · `just trace` · `just narrative-editor` (Writer's Toolkit — drop JSONL onto the page) · `just template-audit` · `just wiki`.

## Conventions

- Conventional commits (`feat:` / `fix:` / `chore:` / `refactor:` / `test:` / `docs:`) — no scopes.
- **Solo-to-main: commits push to main directly; feature branches optional. Global `wnmitch/<name>` convention does not apply here.**
- VCS: `jj` (not raw git).
- Design docs: `docs/systems/` — one stub per tunable system. Auto-generated status: `docs/wiki/systems.md`.
- **Substrate stubs are forbidden.** Every marker in `src/components/markers.rs` ships with a reader (`Has<>` / `With<>` / `X::KEY`) AND a writer (`.insert(X)` / `.remove::<X>()` / `MarkerSnapshot::set_*`) in the same commit, or with an entry in `scripts/substrate_stubs.allowlist` naming the wiring ticket. Enforced by `scripts/check_substrate_stubs.sh` via `just check`. Catalogue: [`docs/open-work/pre-existing/substrate-stub-catalogue.md`](docs/open-work/pre-existing/substrate-stub-catalogue.md). Precedent: ticket 158.
- **InfluenceMap registry stubs are forbidden.** Every `impl InfluenceMap for <Type>` in `src/` ships with a `populate_influence_map_registry` call in `src/plugins/simulation.rs` (or an allowlist entry naming the wiring ticket). Enforced by `scripts/check_influence_map_registry.sh`. Precedent: ticket 207.

## Architecture

- **Utility AI + GOAP.** Cats score per-tick (`src/ai/scoring.rs`); winning disposition drives the GOAP planner (`src/systems/goap.rs`) that sequences `resolve_*` steps under `src/steps/`. No behavior trees, no LLMs.
- **Maslow needs.** 5 tiers (physiological → self-actualization); lower tiers suppress higher when critical. ("Tier 1..5" refers to Maslow rank — distinct from the AI substrate's `L1/L2/L3` shorthand at [`docs/systems/ai-substrate-refactor.md:551`](docs/systems/ai-substrate-refactor.md), which names markers / DSE scoring / softmax layers.)
- **Ecological-magical-realist world.** Magic, fate, the Calling, wards, corruption are *ecological phenomena with metaphysical weight* — tune as part of the ecosystem, not as an unlockable layer.
- **No director.** No difficulty scaling, no out-of-fiction storyteller. (In-fiction coordinator cats *can* issue directives; those are perceivable substrate that recipients score and may refuse — not a thumb on the scale.) Seasons / weather / corruption *are* the event generator.

## Design pillars

Three load-bearing rules that decide *which kind of fix is allowed* before parameter tuning is on the table. Each has at least one "ruined us" precedent.

- **Items are real.** Items are spatial world entities with real physical constraints — never abstract resources, invisible inventory, or stat sticks. Effects live on action resolvers keyed to item identity, not on numeric modifier fields on the item type. *Why:* tickets 175 (Inventory.add succeeds before Stores.remove + despawn — an item is never both held and on the ground), 189 (carrying-cost is a load-bearing tradeoff, not tunable away), 193 (zone-mismatch defects surface *because* items occupy zones). *Apply:* add capability to the resolver that reads the item, not to the item type. Doctrine: [`docs/systems/crafting.md`](docs/systems/crafting.md), [`docs/systems/slot-inventory.md`](docs/systems/slot-inventory.md).
- **Substrate over hacks.** Prefer substrate-side levers (DSE axes, considerations, markers, eligibility filters, scoring shape visible in the L2 trace) over hidden side-channels (interrupts, overrides, gates, silent-advances, post-hoc modifier passes that mutate per-Action scores after L2 emit). *Why:* tickets 087 / 093 / 163 made the antipattern visible and started retiring it; 091 and 111 showed the failure mode of getting the sequencing wrong (partial substrate adoption or premature umbrella retirement collapses behavior during transition). *Apply:* substrate axes land first, the corresponding hack retires second — never the reverse; if the L2 trace doesn't explain the choice, the encoding is wrong.
- **Richer perception, better strategy.** As cats understand their environment in good chunks — orthogonal axes that each encode a distinct situation, not a louder single alarm — they make more strategic decisions and welfare improves. *Why:* substrate refactors that decomposed single-channel signals into orthogonal axes (e.g., 087 / 148) shifted behavior from blanket response to situation-appropriate; the inverse — substrate that elevates an action without decomposing far enough to price its true cost — produced the L3 patrol absorption cascade (181 iter-2; memory `project_l3_patrol_absorption_cascade`), where Patrol elevation exposed cats to ShadowFoxes and starved the colony 24k ticks later. *Apply:* prefer adding orthogonal axes over amplifying existing ones; compose personality / phobias / ambient context at the modifier layer, never inside the underlying perception scalar (memory `feedback_single_axis_perception_scalars`); welfare canaries must hold across any perception-layer change (`just verdict` vs baseline).

## Long-horizon coordination

**Indexes** (read before any new system / balance change / non-trivial refactor): `docs/open-work/tickets/<NNN>-<slug>.md` (frontmatter — `status`, `cluster`, `initiative`, `parked`, `blocked-by` — is source of truth; index at `docs/open-work.md`) · `docs/open-work/pre-existing/*.md` (long-lived issues) · `docs/open-work/landed/<NNN>-<slug>.md` (per-file landed archive — same layout as active tickets, with `landed-at` + `landed-on` frontmatter) · `docs/open-work/clusters.md` (categorical bucket taxonomy) · `docs/open-work/initiatives/*.md` (thematic outcomes) · `docs/wiki/systems.md` (Built / Partial / Aspirational per system) · `docs/balance/*.md` (append iterations to the existing thread).

**Two-axis ticket tagging.** Every ticket carries exactly one `cluster:` (categorical — *where the work lives in code*, see [`docs/open-work/clusters.md`](docs/open-work/clusters.md)) and zero-or-more `initiative:` tags (thematic — *what outcome it serves*, see [`docs/open-work/initiatives/`](docs/open-work/initiatives/)). A crafting ticket and a monument ticket carry different clusters (`items-crafting` vs `buildings-zones`) but can share `initiative: [world-richness]`. **`--cluster` is required at open-time** (`just open-ticket "<title>" --cluster <name>` errors without it); `--initiative <a,b>` is optional. The index renders both axes: `## Ready by cluster` for categorical filtering, `## Ready by initiative` for thematic rollups. Never reuse `cluster:` to express thematic outcomes — that conflation was the substrate-shape problem that motivated the split (precedent: tickets 305 / 306 / 307).

**Before starting work:** `just open-work-active` to see what's load-bearing right now; `just open-work-ready` / `open-work-wip` to match against existing tickets; check `docs/wiki/systems.md` if a system is named. If no ticket matches, name whether the work advances `project-vision.md` §5 (broaden sideways) or a continuity canary, confirm with the user, then run `just open-ticket "<title>" --cluster <name>` (add `--bugfix`, `--initiative <a,b>`, or `--blocked-by <ids>` as needed) as the first commit — **never hand-write the file.** If it advances an in-flight ticket, flip its `status: in-progress` and regenerate the index with `just open-work-index` (this transition has no dedicated script yet — see "Coverage gaps" below).

**When work lands / defers / surfaces.** Landed → `just land <id>` (flags under Daily). Surfaced mid-session → `just open-ticket "<title>"`. Deferred → set `status: parked` + `parked: <date>` + a `## Log` line naming the blocker, then `just open-work-index`. Trivial work without a ticket → write a fresh `landed/NNN-<slug>.md` with the standard frontmatter, then `just open-work-index`. Balance iteration → append to the existing `docs/balance/*.md` thread. Any change to `SimulationPlugin::build()` regenerates `docs/wiki/systems.md` (`just wiki`) in the same commit.

**Ticket lifecycle is script-driven.** NEVER hand-edit a ticket's frontmatter, hand-move files between `tickets/` and `landed/`, hand-clear `blocked-by` entries on dependent tickets, or hand-regenerate `docs/open-work.md` when `just land` or `just open-ticket` covers the operation. The scripts exist precisely to absorb that repetition; re-implementing them by hand burns tokens for zero added value and risks divergent frontmatter shapes. Same enforcement strength as "Substrate stubs are forbidden" — if the script can do it, the script does it.

**Coverage gaps (manual edits still required, no script yet):** (a) `ready → in-progress` flip on an existing ticket — edit `status:` line, then `just open-work-index`. (b) `ready → parked` (and `parked → ready`) — edit `status:` + `parked:` + `## Log` line, then `just open-work-index`. (c) Trivial work landed without ever opening a ticket — write a fresh `landed/NNN-<slug>.md` directly with the standard frontmatter (`status: done`, `landed-at`, `landed-on`), then `just open-work-index`. All other transitions go through `just land` / `just open-ticket`.

**Antipattern migration follow-ups are non-optional.** When a substrate-over-override or antipattern-migration ticket narrows scope, lists items in §Out of scope, or parks subscope ("park as a separate ticket," "follow-on if desired"), each parked item MUST be opened with `just open-ticket "<title>" [--blocked-by <parent>]` in the same commit that lands the parent ticket — `--blocked-by` auto-sets `status: blocked`; omit it for `status: ready`. The opening commit's `## Why` references the parent's narrowing decision. The repo is large; "open as follow-on if desired" rots into lost context. The parent ticket's `## Log` lands-day line names the IDs opened with it. This is the substrate-over-override discipline applied to the work-tracking layer itself: don't author parallel intent ("we should do X someday") in conversation memory when the index can hold it durably.

**Major in-flight: AI substrate refactor.** Spec [`docs/systems/ai-substrate-refactor.md`](docs/systems/ai-substrate-refactor.md) (§4 markers + §6 target-taking DSEs are load-bearing; **§4.7 substrate-vs-search-state is required reading before opening any substrate-migration ticket** — it names the boundary that 092 misclassified). Status [`docs/open-work/tickets/014-phase-4-follow-ons.md`](docs/open-work/tickets/014-phase-4-follow-ons.md) — read before any DSE port. Balance-tuning on refactor-affected metrics is **deferred** until the substrate stabilizes. DSE registration: `populate_dse_registry` in `src/plugins/simulation.rs`. Exemplar port: `src/ai/dses/socialize_target.rs`.

**Parallel-session orchestration (tickets 354 + 355 / [`docs/workflow/parallel-sessions.md`](docs/workflow/parallel-sessions.md)).** When running multiple Claude Code sessions in parallel, the operator surface is `/work` (daily driver) + `/retag` (one-shot corpus tagging) + `/foreman` (master-orchestrator that spawns swarm-safe polecats — Stage 2 live as of 355). Sessions live at `~/clowder-sessions/<slug>/` as jj workspaces with their own working copy + target dir; `session/<slug>` is the bookmark namespace, owned by exactly one session and never moved by anyone else. `main` is read-only inside sessions — the only path to `main` is `just refinery --land <slug>` (manual) or `just refinery --auto` (swarm-safe only, whitelisted in code). Every active ticket carries an `orchestration:` frontmatter axis with three values (enforced by `scripts/check_orchestration_frontmatter.py` via `just check`):

- **`substrate-sensitive`** (default): bugfix work; layer-walk required; per-ticket verdict cadence; sweep-land never auto.
- **`coherent-block`**: epic construction (HTN 128, crafting 016, body zones 095, continuous-position 135) where intermediate states are structurally unverifiable. Carries `block: <initiative-id>`; ≤1 ticket per block carries `verdict-anchor: true` (asserting the orthogonality precondition — that the block's legs are designed orthogonally per the "richer perception, better strategy" pillar). Verdict fires at the anchor's landing; intermediates land verdict-skipped.
- **`swarm-safe`**: docs / frontmatter migrations / mechanical refactors / atomic bugfixes with verified layer-walks. Polecat-eligible; `refinery --auto` is whitelisted in code to this track only.

Recipes (Claude's primitive surface; humans use `/work` / `/retag` / `/foreman`): `[session]` lifecycle (`just session-new <slug>`, `session-list`, `session-done`); `[refinery]` (`just refinery [--land <slug> | --auto [--dry-run]]`); `[retag]` (`just retag <id> --track <name>`, `retag-suggest`, `retag-audit`); `[foreman]` (`just foreman`, `foreman-spawn N`, `foreman-watch`, `foreman-log`, `foreman-shutdown`); `[block]` (`just block-list`, `block-info <id>`); `[ticket-query]` (`just ticket-info <id>`, `open-work-ready-filtered --track <name>`). Discoverability for me: `just --list | grep '\[<tag>\]'`.

**Polecats (Stage 2).** Headless child `claude` CLI processes spawned by `/foreman` against pre-created swarm-safe workspaces. Default 3 polecats, 30m wall-clock cap per child (subscription-billed — no dollar caps; macOS lacks `timeout(1)`, so the cap is implemented via a wallclock-sentinel subprocess). The foreman enters an auto-poll-and-land loop after spawning: every 30s checks PIDs; when all polecats exit, runs `just refinery --auto` to drain bookmarks that pass the gate (`just check && just test` per workspace + working-copy clean + fast-forward only). Failed polecats (dead PID + bookmark not pushed) release their ticket-claim back to `ready` via `session_done.sh --force`. Polecat-eligibility is **swarm-safe only**, enforced in three places: `/foreman` skill refuses other tracks, `scripts/foreman.sh` only picks from the swarm-safe ready queue, `scripts/refinery.sh --auto` rejects non-swarm-safe rows even with explicit `--track <other>`.

## Bugfix discipline

Every bugfix plan MUST include at least one **structural-revision candidate** alongside parameter-level options. "Structural" means one of: **split / extend / rebind / retire** an existing `DispositionKind`, DSE, Marker, or plan template. The structural candidate doesn't have to ship — it has to be drafted, named, and explicitly considered. If you can't draft one, you haven't audited `src/components/disposition.rs::from_action`, the plan templates under `src/ai/planner/` (and `goap_plan.rs`), or the completion proxies in `src/components/commitment.rs` carefully enough.

**Structural-option menu** (mirror in every fix-shape decision tree):
- **split** — give the action its own `DispositionKind` / DSE / Marker variant. (Precedent: ticket 150 R5a, `Eat` out of `Resting`.)
- **extend** — keep the umbrella, branch the plan template / completion proxy / scoring shape on entry conditions so the umbrella varies by trigger. (Precedent: ticket 148 distress → adrenaline-facet refactor.)
- **rebind** — change the Action → Disposition (or sibling) mapping without inventing a new variant.
- **retire** — delete the variant entirely if the layer-walk shows it has no load-bearing job.

**All multi-tick aspirations are HTN methods.** Any ticket proposing new per-cat multi-step goal-shaped commitment substrate (a new Component carrying "I am pursuing X across N ticks") must either (a) author an HTN method in `populate_method_registry`, OR (b) be a mirroring projection of an existing method (like `JointIntention` is of the Courtship method). Naked aspiration Components — multi-tick goal state with no method-registry entry — are forbidden by the same enforcement strength as substrate stubs. Enforcement: `scripts/check_method_registry.sh` (lands with [319](docs/open-work/tickets/319-method-registry-populate-no-stub-enforcement.md)). Design home: [`docs/systems/htn-methods.md`](docs/systems/htn-methods.md); epic dashboard: [128](docs/open-work/tickets/128-htn-method-composition.md). Precedent: the 128 epic + 25 children opened 2026-05-14.

**Every dormant method has a glue ticket.** A method registered as `ApplicableWhen::PendingSubstrate { blocker }` must have its `blocker` field point to an **open** ticket in `docs/open-work/tickets/` whose frontmatter carries `wires-method: [<method-id>...]` referencing back. If the wiring ticket doesn't exist when authoring the dormant method, open it in the same commit. The registry script enforces both directions: dormant method without glue ticket fails CI; glue ticket without matching method-id in frontmatter fails CI. Without this discipline, design intent for arcs the sim could express rots — methods describe natural narrative trees that never sprout because nobody trips over the design intent in their work surface (`just open-work-ready` / `just next` / `just similar` don't surface registry-internal state). Precedent: 128 epic's Tier-2 dormant methods (332/333/334) all carry `wires-method` frontmatter from open-time.

**Layer-walk audit before listing fix candidates.** Walk **L1 markers → L2 DSE scores → L3 softmax → Action→Disposition mapping → plan template → completion proxy → resolver.** For each layer, mark the relevant facts `[verified-correct]` or `[suspect]` in the ticket's "Current architecture" section. A plan that lists only resolver-level fixes against `[suspect]` mappings or templates has not been audited.

**Reframe discipline.** When a hypothesis upgrades (v1→v2), `[verified-...]` rows promoted under the prior framing are not transitively verified — re-promote each via a fresh query that *distinguishes* v2 from v1 before any candidate depends on them. The same evidence pool can support compatible-but-incomplete framings; falsifying v1 doesn't license v2. (Precedent: ticket 189 v1→v2 reframe carried the schedule-edge row's verification across; the actual defect in 193 required different evidence to surface.)

**Scenario microexperiment before a soak.** Once the layer-walk identifies the suspect mapping/template/scoring, isolate the question with `just scenario <name>` (or define a new scenario under `src/scenarios/`) instead of running `just soak`. The harness preloads 1–5 cats with specific needs/personality/markers/positions and prints the focal cat's per-tick winning DSE + ranked L2 score table in ~3 seconds — the right tool for "given this state, which DSE wins?" triage. Reach for `just soak` only when the bug genuinely requires whole-colony dynamics (continuity canaries, drift, multi-system interaction) — and state that explicitly in the ticket's investigation section so future readers see why the cheaper tool was skipped. Ticket 162 ships the harness + 7 archetype scenarios.

Precedent: ticket 150's first plan listed R1 (resolver) / R2 (predicate) / R3 (scoring), all parameter-level; the user surfaced R5 (split Eat from Resting), which was load-bearing. The same lesson lives in the auto-memory entry "Audit L3 Action→Disposition mapping when investigating Clowder AI defects" at the user-global layer. Bugfix tickets should use [`docs/open-work/tickets/_template_bugfix.md`](docs/open-work/tickets/_template_bugfix.md), which embeds the layer-walk table and structural-option slot.

**Sub-agent dispatch discipline.** Before delegating any non-trivial investigation to an Explore / Plan / general-purpose sub-agent, walk [`docs/open-work/_template_subagent_prompt.md`](docs/open-work/_template_subagent_prompt.md) — five required slots (mark load-bearing facts as hypotheses · field-name validation · alternative-mechanism enumeration · skill-surface escape clause · ratio normalization for cross-run comparison). The prompt IS the agent's perception layer; bad framing produces bad sense data, and the failure propagates one layer up. Precedent: ticket 194 §F9 (the 189-cluster diagnostic delay traces back to two Explore-agent prompts that inherited the wrong premise as established context).

**Open-time prefill for collapse tickets.** When a session ends in a failing soak, open the bugfix ticket via `/ticket-from-session "<title>"` ([`.claude/commands/ticket-from-session.md`](.claude/commands/ticket-from-session.md)). The skill detects the failing run, composes existing recipes (`just verdict`, `just fingerprint`, `just q run-summary`, `just q deaths`, `just q anomalies`) into a hot-context payload, dispatches a Plan agent against the five-slot scaffolding, and post-edits the new bugfix ticket to splice in `## Hot context` + a promoted layer-walk + a draft structural-option menu. Promoting `[needs-promote]` → `[verified-*]` in the next session is non-optional and uses fresh queries (per Reframe discipline). The skill refuses to open a ticket if no failing run is present — it's a session-failure tool, not a generic shortcut. Wired here so it actually fires (per `feedback_diagnostic_tools_need_discipline_wiring`).

## ECS rules (Bevy 0.18)

- **Messages, not Events:** `#[derive(Message)]`, `MessageWriter<T>` / `MessageReader<T>`, `app.add_message::<T>()`. Register in `SimulationPlugin::build()` — windowed and headless paths share that plugin (ticket 030). Names are verbs (`SpawnCat`, `CatDied`), not `*Event`.
- Prefer `run_if` guards over early returns. Never `.clone()` resource data in per-tick systems — borrow via `Res<T>` / `ResMut<T>`.
- **Bevy 16-param limit:** bundle related queries / writers in `#[derive(SystemParam)]` structs. Preferred over `Option<Res<T>>` hacks.
- **Query disjointness:** splitting `Query<&mut C>` by marker → pair `With<M>` and `Without<M>` against sibling queries.

## GOAP Step Resolver Contract

Every `pub fn resolve_*` under `src/steps/**` returns `StepOutcome<W>` (`src/steps/outcome.rs`) — module rustdoc carries the witness-shape rationale. The contract makes "silent-advance with no real-world effect" a type error: callers MUST route Feature emission through `record_if_witnessed`, never directly on `StepResult::Advance`.

**Five required rustdoc headings on every `pub fn resolve_*`** (grepped by `scripts/check_step_contracts.sh` via `just check`):

```text
/// **Real-world effect** — what this mutates when it succeeds.
/// **Plan-level preconditions** — `StatePredicate`s the planner guarantees before this step runs.
/// **Runtime preconditions** — what this checks internally; what happens if the check fails (MUST NOT return witnessed Advance when the effect didn't happen).
/// **Witness** — the `StepOutcome<W>` shape and what `W` records.
/// **Feature emission** — which `Feature::*` the caller passes to `record_if_witnessed` (Positive / Neutral / Negative).
```

Exemplars: `src/steps/disposition/cook.rs`, `src/steps/disposition/feed_kitten.rs`, `src/steps/building/tend.rs`. **Never-fired canary:** new positive `Feature::*` must be classified in `Feature::expected_to_fire_per_soak()` (`src/resources/system_activation.rs`). Returning `true` enrolls the feature in the seed-42 canary; rare-legend events (`ShadowFoxBanished`, `FateAwakened`, …) return `false` and are exempt.

## Verification

`just headless` is the canonical diagnostic; `just soak [seed]` is the canonical 15-min release deep-soak; **`just verdict <run-dir>` is the one-call gate.** Always release for verification — debug is ~4× slower. **Never overwrite** `logs/tuned-*/` or `logs/baseline-*/` — `just soak` and `just soak-trace` refuse, and `.claude/hooks/no-log-overwrite.py` enforces. Line 1 of `events.jsonl` is a header with seed + commit + full `SimConstants` + `start_tick`; runs are only comparable iff their headers match on `constants` and carry the same non-dirty `commit_hash`. **Ticks on disk are absolute, never zero-based** — every run begins at `start_tick = 60 × ticks_per_season ≈ 1,200,000` so founder cats can have varied ages (rationale: `src/plugins/setup.rs:297-301`, `docs/balance/activation-1-status.md`). jq recipes for ad-hoc queries: `docs/diagnostics/log-queries.md`.

**Hard survival gates** (must pass on the canonical seed-42 deep-soak): `deaths_by_cause.Starvation == 0` · `deaths_by_cause.ShadowFoxAmbush <= 10` · footer line written · `never_fired_expected_positives == 0`.

**Continuity canaries** (each ≥1 per soak; collapse means survival lock): `grooming` · `play` · `mentoring` · `courtship` · `mythic-texture` (≥1 named event per sim year). Generational continuity tracked via `KittenMatured` in the activation block. Ticket 250 demoted `burial` from the canary set because post-247 / 248 substrate stability makes deaths (and therefore burials) genuinely rare in healthy colonies; the footer tally still records burials when they happen.

**Drift > ±10% on a characteristic metric requires a hypothesis** `{ecological/perceptual fact} ⇒ {predicted direction + magnitude}` and four artifacts (hypothesis · prediction · observation · concordance — direction match + magnitude within ~2×). `just hypothesize <spec.yaml>` runs this end-to-end. Drift > ±30% needs additional scrutiny. Survival canaries are hard gates regardless. **A refactor that changes sim behavior is a balance change.** Doctrine: `docs/balance/*.md`.

## Tuning constants

All knobs in `src/resources/sim_constants.rs` (`#[derive(Resource)]`; no inline magic numbers). The full struct serializes into the `events.jsonl` header — that's the comparability invariant. `just explain <constants.path>` shows doc-comment + current value + every read-site + (if `rebuild-sensitivity-map` was run) Spearman rho per metric.

## Rendering

Tilemap uses plain Bevy `Sprite` entities — **NOT `TilemapBundle`**. bevy_ecs_tilemap's GPU pipeline silently renders all tiles as texture index 0 on macOS Metal. Base terrain at z=0, autotile overlays at z=1/2/3. F6/F7/F8 toggle overlay visibility.
