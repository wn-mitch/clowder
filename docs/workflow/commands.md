# Commands you should reach for

Cheat sheet for the `just`-recipes most worth knowing about. Categorized by intent.

## Daily

- `just run` / `just seed <n>` / `just load` — launch the interactive sim. On macOS these assemble an ephemeral `Clowder.app` around the fresh debug binary and launch via `open`, so the window takes focus (Dock icon + Cmd-Tab). Quit with Ctrl-C, Esc, or Cmd-Q. (`just headless` is unaffected — no window.)
- `just check` / `just test` / `just ci` — `check` includes step-resolver and time-unit linters.
- `just open-work` / `just open-work-ready` / `just open-work-wip` — ticket queues.
- `just land <id>` / `just open-ticket "<title>"` — **the only sanctioned way to land or open a ticket.** See [`ticket-lifecycle.md`](ticket-lifecycle.md).
- `just q <subtool> <run-dir>` — logq drill-down (`run-summary` · `events` · `deaths` · `narrative` · `trace` · `cat-timeline` · `anomalies`). Reach for it whenever you ask "why did X happen in this run?"
- `just logdb-build` / `logdb-query SQL` / `logdb-shell` / `logdb-chart <recipe>` — cross-run SQL over every archive in `logs/` (DuckDB at `logs/runs.duckdb`). Use whenever the question spans seeds, commits, or archives. Complementary to `logq`: logq drills *into one run*, logdb compares *across many*. Heavy tables (`cat_snapshot_scores`, traces) are opt-in via `--with-scores` / `--with-traces`. Schema + chart recipes: [`docs/diagnostics/logdb.md`](../diagnostics/logdb.md).

## Verifying a change

- `just scenario <name>` — fast (~3s) deterministic microexperiment harness (preset cats, preloaded state). **Preferred over `just soak` for hypothesis triage** during bugfix loops. `just soak` remains for whole-colony verification once a fix is drafted. See ticket 162 and [`../discipline/bugfix.md`](../discipline/bugfix.md).
- `just soak [seed]` — canonical 15-min release deep-soak (writes `logs/tuned-<seed>/`; refuses overwrite).
- `just verdict <run-dir>` — **one-call gate; always run after a soak.** Composes canaries + continuity + constants drift + footer-vs-baseline. Exit 0/1/2 = pass/concern/fail. See [`../discipline/verification.md`](../discipline/verification.md).
- `just fingerprint <run-dir>` — per-metric in-band readout vs `docs/balance/healthy-colony.md`.

## Balance work

- `just hypothesize <spec.yaml>` — runs the four-artifact methodology end-to-end (baseline + treatment sweeps + concordance check + draft balance doc).
- `just sweep <label>` — multi-seed × multi-rep headless sweep.
- `just sweep-stats <dir> [--vs <other>]` — Welch's t / Cohen's d / effect-size bands.
- `just promote <dir> <label>` — lock in a named baseline (`verdict` auto-reads `logs/baselines/current.json` next).
- `just bisect-canary <metric>` — find the commit that introduced a canary regression.
- `just baseline-dataset <label>` — 5-phase versioned-baseline orchestrator (probe → sweep → focal traces → conditional weather → REPORT.md).
- `just rebuild-sensitivity-map` — quarterly perturbation sweep; powers `just explain`'s rho column.

## Inspecting one cat / one knob

- `just inspect <name>` — cat personality + decision history from the event log.
- `just explain <constants.path>` — doc-comment + current value (from a recent run header) + every read-site + (if rebuilt) Spearman rho per metric.
- `just soak-trace <seed> <cat>` — focal-cat L1/L2/L3 trace sidecar (per §11 of the substrate-refactor spec). Multi-focal sweeps probe the full DSE catalog: marker-gated DSEs stay silent on cats without the marker.

  **Multi-focal convention (ticket 227):** when the tuning ticket targets a marker-gated or eligibility-filtered DSE, run a second `soak-trace` on a cat that satisfies the gate alongside the default generalist (Simba). Find the eligible cat name before writing the balance doc: use `just q events <run-dir> CoordinatorElected` for Coordinate; filter for `hungry_kitten_urgency > 0` presence for Caretake. Without an eligible focal cat, the gated DSE's per-cat `frame-diff` row is structurally absent (not scored zero), making per-cat verification impossible.

- `just frame-diff <baseline> <new> [hypothesis.md]` — per-DSE drift ranked by |Δ mean|; hypothesis classifies each DSE as ok / drift / wrong-direction.

## Parallel sessions

- `[session]` / `[refinery]` / `[retag]` / `[foreman]` / `[block]` / `[ticket-query]` — see [`parallel-sessions.md`](parallel-sessions.md). Discoverability: `just --list | grep '\[<tag>\]'`.

## Misc

- `just logs` · `just trace` · `just narrative-editor` (Writer's Toolkit — drop JSONL onto the page) · `just template-audit` · `just wiki`.

## Skills surface

Skills wrap these recipes with a higher-level interface. Reach for skills first when the question matches their trigger; they compose recipes and produce structured envelopes the raw recipe doesn't. Use the skill surface for **all** log queries — never raw `grep` / `jq` / `just q` directly. (Memory: `feedback_use_skill_surface`.)

Key skills (see system reminder for the full list):
- `/logq` / `/inspect` / `/verdict` / `/diagnose-run` / `/diagnose-collapse` — run-level investigation.
- `/explain` / `/frame-diff` / `/sweep-stats` / `/hypothesize` — balance + tuning.
- `/similar` / `/next` — corpus retrieval.
- `/work` / `/foreman` / `/retag` — parallel-session orchestration.
- `/ticket-from-session` — collapse → bugfix ticket prefill.
