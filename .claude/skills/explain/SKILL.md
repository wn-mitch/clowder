---
name: explain
description: Explain a tuning constant (`just explain <constants.path>`) — doc comment + current value (from a recent `events.jsonl` header) + every read-site in `src/` + (if `logs/sensitivity-map.json` exists) per-metric Spearman rho. Use whenever the user asks what a knob does, what its current value is, where it's read, or what it would shift if tuned. Trigger phrases — "what does FOOD_DECAY_RATE do", "where is X read", "what knob controls Y", "what's the current value of …", "show me the doc comment for …", "list all the constants". Do NOT fire for — multi-knob hypothesis testing (use `just hypothesize`), wanting to *change* a constant (this tool is read-only — edit `src/resources/sim_constants.rs` directly), or runtime-derived values that aren't `SimConstants` fields.
---

# Constants Explainer (`just explain`)

`just explain <constants.path>` resolves a dotted path against the live `SimConstants` struct (read from a recent `events.jsonl` header) and surfaces the doc comment, every read-site in `src/`, and per-metric sensitivity rho if `logs/sensitivity-map.json` is built.

## When to fire

**Fire when:**

- User asks "what does X do" or "what controls Y" where X / Y is a tuning knob.
- User is balance-tuning and wants to know which metrics a knob most influences (sensitivity rho).
- User wants the canonical list of dotted paths to use in a `constants_patch` for `just hypothesize`.
- Reviewing a balance change and wanting to see every read-site for the field being changed.

**Do NOT fire when:**

- User wants to test a balance change end-to-end (use `just hypothesize`).
- User wants to *edit* a constant — this tool is read-only. Edit `src/resources/sim_constants.rs` directly.
- The "constant" in question is a derived value (e.g., a function output, a per-tick computation) — `explain` only resolves struct fields. Use `rg` for arbitrary symbol search.

## The envelope

```jsonc
{
  "constant": "magic.thornward_decay_rate",
  "value":    0.001,
  "doc":      "Per-tick passive decay applied to every active thornward.\nClamped to [0, 1]; 0 = wards never decay, 1 = wards vanish next tick.",
  "read_sites": [
    "src/systems/magic/wards.rs:142",
    "src/ai/dses/place_ward.rs:87",
    "src/resources/sim_constants.rs:412"
  ],
  "sensitivity": [
    { "metric": "wards_placed_total",     "rho": +0.78 },
    { "metric": "ward_count_final",       "rho": -0.62 },
    { "metric": "deaths_by_cause.ShadowFoxAmbush", "rho": +0.34 }
  ],
  "constants_source": "/Users/will.mitchell/clowder/logs/tuned-42/events.jsonl",
  "note":     null,
  "children": null,    // populated when path is a sub-tree, see below
  "nearest":  null     // populated when path doesn't resolve, see below
}
```

**Exit codes:** `0` if the path resolved (or has children, or has nearest matches with read-sites/doc) · `1` if path is unrecognized AND has no read-sites/children/nearest · `2` on hard error (no events.jsonl with constants block found and `--list` was requested).

## Three failure modes — each surfaces useful adjacency

1. **Sub-tree, not leaf** — caller passed `magic` when meaning `magic.thornward_decay_rate`. Envelope sets `note: "path is a sub-tree, not a leaf — N child paths available"` and populates `children: [...]`. **Read the children list and pick the leaf you meant.**
2. **No such path, but the leaf name has read sites** — caller passed `social.foo_rate` when `foo_rate` is actually a method, not a field. Envelope sets `note: "path resolves to no constants leaf; the field name has read sites in src/, suggesting it's a method or fn rather than a struct field"` and `read_sites` is populated.
3. **No such path, fuzzy match available** — caller passed `social.bond_proximity_social_rate` when it lives at `needs.bond_proximity_social_rate`. Envelope sets `note: "no such constant path"` and `nearest: ["needs.bond_proximity_social_rate", ...]`. **Re-fire with the corrected path.**

## `--list` mode

```bash
just explain --list                    # one dotted path per line, sorted
just explain --list | grep magic       # find every magic-related knob
just explain --list | wc -l            # how many tunable knobs total
```

Useful when drafting a `constants_patch` for `just hypothesize` — these are the only paths the override system recognizes.

## Sensitivity rho — when present

`logs/sensitivity-map.json` is built by `just rebuild-sensitivity-map` (a quarterly perturbation sweep). When present, `explain` includes the top-N metrics ranked by |Spearman rho| against this knob. Interpretation:

- **|rho| ≥ 0.5** — strong correlation; tuning this knob will move this metric.
- **0.2 ≤ |rho| < 0.5** — weak correlation; moves the metric but with noise.
- **|rho| < 0.2** — effectively independent.
- **Sign** — positive rho means raising this knob raises the metric; negative the inverse.

If the field is empty (`"sensitivity": []`), the sensitivity map either doesn't exist yet (run `just rebuild-sensitivity-map`) or the knob has no measurable effect on any tracked metric.

## Examples

```bash
# Single leaf — the common case.
just explain magic.thornward_decay_rate

# Sub-tree — get the children list.
just explain magic

# Fuzzy match — recovers from typos in one turn.
just explain social.bond_proximity_social_rate    # → "did you mean: needs.bond_proximity_social_rate"

# Override the constants source (defaults to most-recent logs/tuned-*/events.jsonl).
just explain magic.ward_decay_per_tick --run logs/baseline-2026-04-25/events.jsonl

# List everything.
just explain --list

# Human-readable.
just explain magic.thornward_decay_rate --text
```

## Caveats

- **Float precision noise.** Values are rounded to 6 significant figures (f32→f64 re-encoding noise: `0.001` would otherwise display as `0.0010000000474974513`). Don't compare to the raw `events.jsonl` constants block character-for-character.
- **Read-site grep is in `src/` only.** Tests, examples, and docs are not searched. If you need broader coverage, use `rg --type rust '\.<field_name>\b'` directly.
- **Stale constants block.** If your most-recent `tuned-*/events.jsonl` predates a constants change, `value` will be wrong. Pass `--run <path>` to point at a known-current run, or run a fresh `just soak` first.
- **Doc comment must be `///`.** `//` comments and `#[doc = "..."]` attributes are not parsed. Single-line `///` comments are concatenated.

## Relationship to neighbouring tools

- **`just hypothesize`** — uses paths from this tool's `--list` output as keys in `constants_patch`. Verify a path with `just explain <path>` before invoking `hypothesize`.
- **`just rebuild-sensitivity-map`** — populates the `sensitivity` field. Quarterly cadence (~6h sweep); don't run unless the map is genuinely stale.
- **`src/resources/sim_constants.rs`** — source of truth for fields and `///` doc comments. To *change* a constant, edit this file directly.
- **`docs/balance/*.md`** — appended to as iterations land. `explain` is the orientation tool *before* the balance work begins.

## Non-goals

- Does not write to disk. Pure read-only.
- Does not validate a `constants_patch` for `hypothesize`. The patch is validated when `hypothesize` runs (path → metric not found error after both sweeps complete) — preflight every patch key with `just explain <key>` to avoid the wasted sweep.
- Does not explain runtime-derived values (e.g., per-cat fulfillment scores, per-tick decay applications). Those are computed from constants but aren't constants themselves.
