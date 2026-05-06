# Clowder

A colony sim about a clowder of cats living in a world with its own weight — honest ecology with a mythic undercurrent. *Watership Down meets Timberborn, starring cats.* **Stack:** Rust + Bevy ECS 0.18, 2D pixel-art sprites. Vision: [`docs/systems/project-vision.md`](docs/systems/project-vision.md).

## Commands you should reach for

### Daily
- `just check` / `just test` / `just ci` (`check` includes step-resolver and time-unit linters)
- `just open-work` / `just open-work-ready` / `just open-work-wip`
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
- `just soak-trace <seed> <cat>` — focal-cat L1/L2/L3 trace sidecar (per §11 of the substrate-refactor spec). Multi-focal sweeps probe the full DSE catalog: marker-gated DSEs stay silent on cats without the marker.
- `just frame-diff <baseline> <new> [hypothesis.md]` — per-DSE drift ranked by |Δ mean|; hypothesis classifies each DSE as ok / drift / wrong-direction

Also: `just logs` · `just trace` · `just narrative-editor` (Writer's Toolkit — drop JSONL onto the page) · `just template-audit` · `just wiki`.

## Conventions

- Conventional commits (`feat:` / `fix:` / `chore:` / `refactor:` / `test:` / `docs:`) — no scopes.
- **Solo-to-main: commits push to main directly; feature branches optional. Global `wnmitch/<name>` convention does not apply here.**
- VCS: `jj` (not raw git).
- Design docs: `docs/systems/` — one stub per tunable system. Auto-generated status: `docs/wiki/systems.md`.
- **Substrate stubs are forbidden.** Every marker declared in `src/components/markers.rs` MUST land with at least one reader (`Has<>` / `With<>` / `X::KEY`) AND at least one writer (`.insert(X)` / `.remove::<X>()` / `MarkerSnapshot::set_entity` / `set_colony` / `toggle()`-style helper) in the same commit, or with an allowlist entry in `scripts/substrate_stubs.allowlist` naming the ticket that wires it. Catalogue: [`docs/open-work/pre-existing/substrate-stub-catalogue.md`](docs/open-work/pre-existing/substrate-stub-catalogue.md). `just check` enforces via `scripts/check_substrate_stubs.sh` (also audits `MarkerConsideration::new(_, "<NAME>", _)` string-name references against the marker set). Spec'd-but-unwired substrate cost a full soak round-trip on ticket 158 — the lint exists so the next one fails fast.

## Architecture

- **Utility AI + GOAP.** Cats score per-tick (`src/ai/scoring.rs`); winning disposition drives the GOAP planner (`src/systems/goap.rs`) that sequences `resolve_*` steps under `src/steps/`. No behavior trees, no LLMs.
- **Maslow needs.** 5 levels (physiological → self-actualization); lower levels suppress higher when critical.
- **Ecological-magical-realist world.** Magic, fate, the Calling, wards, corruption are *ecological phenomena with metaphysical weight* — tune as part of the ecosystem, not as an unlockable layer.
- **No director.** No difficulty scaling, no out-of-fiction storyteller. (In-fiction coordinator cats *can* issue directives; those are perceivable substrate that recipients score and may refuse — not a thumb on the scale.) Seasons / weather / corruption *are* the event generator.

## Long-horizon coordination

**Indexes** (read before any new system / balance change / non-trivial refactor): `docs/open-work/tickets/<NNN>-<slug>.md` (frontmatter — `status`, `cluster`, `parked`, `blocked-by` — is source of truth; index at `docs/open-work.md`) · `docs/open-work/pre-existing/*.md` (long-lived issues) · `docs/open-work/landed/<NNN>-<slug>.md` (per-file landed archive — same layout as active tickets, with `landed-at` + `landed-on` frontmatter) · `docs/wiki/systems.md` (Built / Partial / Aspirational per system) · `docs/balance/*.md` (append iterations to the existing thread).

**Before starting work:** `just open-work-ready` / `open-work-wip` to match against existing tickets; check `docs/wiki/systems.md` if a system is named. If no ticket matches, name whether the work advances `project-vision.md` §5 (broaden sideways) or a continuity canary, confirm with the user, then open `tickets/NNN-<slug>.md` as the first commit. If it advances an in-flight ticket, flip its `status: in-progress` and proceed.

**When work lands / defers / surfaces:** Landed → set `status: done` + `landed-at: <sha>` + `landed-on: <date>` and move the ticket file from `tickets/NNN-slug.md` to `landed/NNN-slug.md` (per-file layout matches active tickets; tooling resolves landed blockers via the same `load_tickets` call). Trivial work without a ticket → write a fresh `landed/NNN-slug.md` with the same frontmatter shape. Deferred → `status: parked` + `parked: <date>` + `## Log` line naming the blocker; ticket-blocked → `status: blocked` + `blocked-by: [id]`. Surfaced mid-session → open new ticket (min: `status`, `title`, `added`, `## Why`). Balance iteration → append to existing `docs/balance/*.md`. Any change to `SimulationPlugin::build()` regenerates `docs/wiki/systems.md` (`just wiki`) in the same commit; every ticket status change regenerates `docs/open-work.md` (`just open-work-index`) in the same commit.

**Antipattern migration follow-ups are non-optional.** When a substrate-over-override or antipattern-migration ticket narrows scope, lists items in §Out of scope, or parks subscope ("park as a separate ticket," "follow-on if desired"), each parked item MUST be opened as a concrete `tickets/NNN-<slug>.md` in the same commit that lands the parent ticket — `status: ready` (or `blocked` with `blocked-by: [<parent>]`), `## Why` referencing the parent's narrowing decision. The repo is large; "open as follow-on if desired" rots into lost context. The parent ticket's `## Log` lands-day line names the IDs opened with it. This is the substrate-over-override discipline applied to the work-tracking layer itself: don't author parallel intent ("we should do X someday") in conversation memory when the index can hold it durably.

**Major in-flight: AI substrate refactor.** Spec [`docs/systems/ai-substrate-refactor.md`](docs/systems/ai-substrate-refactor.md) (§4 markers + §6 target-taking DSEs are load-bearing; **§4.7 substrate-vs-search-state is required reading before opening any substrate-migration ticket** — it names the boundary that 092 misclassified). Status [`docs/open-work/tickets/014-phase-4-follow-ons.md`](docs/open-work/tickets/014-phase-4-follow-ons.md) — read before any DSE port. Balance-tuning on refactor-affected metrics is **deferred** until the substrate stabilizes. DSE registration: `populate_dse_registry` in `src/plugins/simulation.rs`. Exemplar port: `src/ai/dses/socialize_target.rs`.

## Bugfix discipline

Every bugfix plan MUST include at least one **structural-revision candidate** alongside parameter-level options. "Structural" means one of: **split / extend / rebind / retire** an existing `DispositionKind`, DSE, Marker, or plan template. The structural candidate doesn't have to ship — it has to be drafted, named, and explicitly considered. If you can't draft one, you haven't audited `src/components/disposition.rs::from_action`, the plan templates under `src/ai/planner/` (and `goap_plan.rs`), or the completion proxies in `src/components/commitment.rs` carefully enough.

**Structural-option menu** (mirror in every fix-shape decision tree):
- **split** — give the action its own `DispositionKind` / DSE / Marker variant. (Precedent: ticket 150 R5a, `Eat` out of `Resting`.)
- **extend** — keep the umbrella, branch the plan template / completion proxy / scoring shape on entry conditions so the umbrella varies by trigger. (Precedent: ticket 148 distress → adrenaline-facet refactor.)
- **rebind** — change the Action → Disposition (or sibling) mapping without inventing a new variant.
- **retire** — delete the variant entirely if the layer-walk shows it has no load-bearing job.

**Layer-walk audit before listing fix candidates.** Walk **L1 markers → L2 DSE scores → L3 softmax → Action→Disposition mapping → plan template → completion proxy → resolver.** For each layer, mark the relevant facts `[verified-correct]` or `[suspect]` in the ticket's "Current architecture" section. A plan that lists only resolver-level fixes against `[suspect]` mappings or templates has not been audited.

**Scenario microexperiment before a soak.** Once the layer-walk identifies the suspect mapping/template/scoring, isolate the question with `just scenario <name>` (or define a new scenario under `src/scenarios/`) instead of running `just soak`. The harness preloads 1–5 cats with specific needs/personality/markers/positions and prints the focal cat's per-tick winning DSE + ranked L2 score table in ~3 seconds — the right tool for "given this state, which DSE wins?" triage. Reach for `just soak` only when the bug genuinely requires whole-colony dynamics (continuity canaries, drift, multi-system interaction) — and state that explicitly in the ticket's investigation section so future readers see why the cheaper tool was skipped. Ticket 162 ships the harness + 7 archetype scenarios.

Precedent: ticket 150's first plan listed R1 (resolver) / R2 (predicate) / R3 (scoring), all parameter-level; the user surfaced R5 (split Eat from Resting), which was load-bearing. The same lesson lives in the auto-memory entry "Audit L3 Action→Disposition mapping when investigating Clowder AI defects" at the user-global layer. Bugfix tickets should use [`docs/open-work/tickets/_template_bugfix.md`](docs/open-work/tickets/_template_bugfix.md), which embeds the layer-walk table and structural-option slot.

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

**Continuity canaries** (each ≥1 per soak; collapse means survival lock): `grooming` · `play` · `mentoring` · `burial` · `courtship` · `mythic-texture` (≥1 named event per sim year). Generational continuity tracked via `KittenMatured` in the activation block.

**Drift > ±10% on a characteristic metric requires a hypothesis** `{ecological/perceptual fact} ⇒ {predicted direction + magnitude}` and four artifacts (hypothesis · prediction · observation · concordance — direction match + magnitude within ~2×). `just hypothesize <spec.yaml>` runs this end-to-end. Drift > ±30% needs additional scrutiny. Survival canaries are hard gates regardless. **A refactor that changes sim behavior is a balance change.** Doctrine: `docs/balance/*.md`.

## Tuning constants

All knobs in `src/resources/sim_constants.rs` (`#[derive(Resource)]`; no inline magic numbers). The full struct serializes into the `events.jsonl` header — that's the comparability invariant. `just explain <constants.path>` shows doc-comment + current value + every read-site + (if `rebuild-sensitivity-map` was run) Spearman rho per metric.

## Rendering

Tilemap uses plain Bevy `Sprite` entities — **NOT `TilemapBundle`**. bevy_ecs_tilemap's GPU pipeline silently renders all tiles as texture index 0 on macOS Metal. Base terrain at z=0, autotile overlays at z=1/2/3. F6/F7/F8 toggle overlay visibility.
