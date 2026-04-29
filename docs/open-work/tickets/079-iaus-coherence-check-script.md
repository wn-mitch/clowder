---
id: 079
title: check_iaus_coherence.sh — process gate against MacGyvered pins
status: ready
cluster: planning-substrate
added: 2026-04-29
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The planning-substrate hardening sub-epic (071) establishes that every defense lands inside the IAUS engine — `Consideration` × `Curve`, `Modifier` in §3.5.1 pipeline, or `EligibilityFilter` — not as a post-hoc pin in a resolver body. The discipline only holds if there's a process gate.

027b Commit B added one such MacGyvered pin (`socialize_target.rs:193`); ticket 078 backports it. Without a gate, the same pattern will reappear next time someone wants to "just override the score for this case." A grep-based check in `just check` catches future MacGyvered pins on PR and forces the contributor to express the override as an engine primitive.

Parent: ticket 071. Independent of 072.

## Scope

- New `scripts/check_iaus_coherence.sh` — greps `src/ai/dses/*.rs` for hard-coded score overrides matching:
  - `if .* { return 1.0 }` / `if .* { return 0.0 }` — direct early-return overrides
  - `return 1\.0;` / `return 0\.0;` outside a documented Consideration / EligibilityFilter constructor body
- Exits 1 with a clear error message pointing at the file:line and the IAUS-coherence rationale ("This pattern bypasses the IAUS score economy. Express the override as a Consideration with a Curve, a Modifier in src/ai/modifier.rs, or an EligibilityFilter — not a post-hoc return.").
- Hooked into `just check` alongside `scripts/check_step_contracts.sh` (which already lives there per CLAUDE.md §"GOAP Step Resolver Contract").
- **Allowlist mechanism**: a comment marker `// IAUS-COHERENCE-EXEMPT: <reason>` on the line preceding the override skips the check. Rare; for cases where the override is genuinely outside the IAUS economy (e.g., narrative-injected events, EligibilityFilter constructor bodies that legitimately need early-return for the gate semantics).

## Out of scope

- Catching MacGyvered patterns outside `src/ai/dses/*.rs` (e.g., in `src/systems/goap.rs` or step resolvers) — those land in `plan_substrate` per ticket 072 and don't have the same anti-pattern shape.
- Static analysis beyond grep (e.g., AST parsing). Grep is sufficient for the patterns we're catching; if false positives accumulate, we can upgrade.

## Approach

Files:

- `scripts/check_iaus_coherence.sh` — new. Bash script using `grep -rEn` over `src/ai/dses/`. Skips lines with the allowlist marker. Returns exit 1 with a clear error message including the offending file:line and a one-paragraph IAUS rationale.
- `justfile` — add the check to `just check` alongside `scripts/check_step_contracts.sh`.

Pattern to grep for (initial; refine on landing):
```
^\s*if\s+.+\{\s*return\s+(1\.0|0\.0);?\s*\}
```

The exemption marker comment must appear on the line immediately before the offending pattern:
```
// IAUS-COHERENCE-EXEMPT: <one-line reason>
if foo { return 1.0; }
```

## Verification

- `just check` passes today (after ticket 078 lands and removes the existing `bond_score` pin — there should be no offending matches in the codebase).
- Regression-test: adding a synthetic `if foo { return 1.0; }` to a target DSE in a test branch makes `just check` exit 1 with the expected error message pointing at the right file:line.
- Allowlist marker is honored: `// IAUS-COHERENCE-EXEMPT: <reason>` immediately preceding the pattern skips the check.
- No false positives on existing `EligibilityFilter` constructor bodies or other legitimate uses (the allowlist marker accommodates these).

## Log

- 2026-04-29: Opened under sub-epic 071.
