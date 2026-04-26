# Baseline registry

First-class registry of named "accepted" soaks for `just verdict`'s
footer-vs-baseline drift comparison and any future tool that needs
a stable "compare against this" reference.

Layout:

```
logs/baselines/
├── README.md              # this file
├── current.json           # active baseline (copy of one <label>.json)
└── <label>.json           # one per promoted soak
```

Each `<label>.json` carries:

- `label` — slug name.
- `run_dir`, `events_path` — absolute paths to the source soak.
- `commit_hash_short`, `commit_dirty` — the commit the binary was built from.
- `seed`, `duration_secs` — run parameters.
- `promoted_at` — ISO-8601 UTC when the promotion ran.
- `footer_snapshot` — full footer JSON from the source events.jsonl, so
  comparisons don't have to re-read the source file.

## Workflow

```bash
# Run a canonical 15-min soak
just soak 42

# Inspect it (see tooling-composition ticket 031)
just verdict logs/tuned-42 --baseline logs/baseline-pre-substrate-refactor/events.jsonl

# Once you're happy with it, promote it as the active baseline
just promote logs/tuned-42 post-state-trio
```

After `just promote`, every `just verdict <run>` invocation auto-reads
`logs/baselines/current.json` for footer drift unless overridden by
`--baseline <path>`.

## Conventions

- One label per substrate change worth treating as a checkpoint
  (`baseline-pre-substrate-refactor`, `post-state-trio`,
  `post-pairing-activity`, etc.).
- Don't commit `current.json` — it's a per-machine pointer that should
  reflect the active workstation's choice. (It is gitignored.)
- Every named baseline's `<label>.json` SHOULD be committed so other
  workstations can `cp <label>.json current.json` to adopt the same.
- Append-only: don't delete an old label even after a new one supersedes
  it. The historical reference is the load-bearing artifact.
