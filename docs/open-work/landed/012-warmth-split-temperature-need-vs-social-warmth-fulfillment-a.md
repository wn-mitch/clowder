---
id: 012
title: Warmth split — temperature need vs social-warmth fulfillment axis
status: done
cluster: null
landed-at: fc7f5e9
landed-on: 2026-04-24
---

# Warmth split — temperature need vs social-warmth fulfillment axis

**Landed-at:** `fc7f5e9` (HEAD-reachable). The frontmatter recorded `47047261`; that was a hidden jj revision rewritten into the current commit during rebase. Bundled with ticket 024 (the §7.W Fulfillment register MVP that 012 phase 3 was blocked on).

**Status at archival:** phase 1 (design) committed; phases 2–4 landed via the bundle commit.

**Why it matters:** `needs.warmth` previously conflated physiological body-heat (hearth/den/sleep/self-groom) with affective closeness (grooming another cat, `src/steps/disposition/groom_other.rs:47`). A cat near a hearth was immune to loneliness at the needs level. The warring-self dynamic of `docs/systems/ai-substrate-refactor.md` §7.W.2 requires a cat to be able to be physically warm and socially starving at the same time — otherwise the losing-axis narrative signal is drowned out by shelter.

**Design captured at:** `docs/systems/warmth-split.md` (phase 1). Cross-linked from `ai-substrate-refactor.md` §7.W.4(b).

**Phase 2 — mechanical rename.** Renamed `needs.warmth` → `needs.temperature` and all `*_warmth_*` constants across ~30 call sites enumerated in the design doc. No behavior change. Verified with `just check`, `just test`, and byte-identical `sim_config`/`constants` header on seed 42 vs pre-rename baseline.

**Phase 3 — `social_warmth` implementation.** Gated on §7.W Fulfillment component/resource landing (ticket 024). Added `social_warmth` as a fulfillment axis; modified `groom_other.rs:47` to feed both parties' `social_warmth` instead of the groomer's temperature; added isolation-driven decay; added UI inspect second bar.

**Phase 4 — balance-thread retune.** New `docs/balance/warmth-split.md` iteration log. Hypothesis was that removing social-grooming from temperature-inflow would reduce well-bonded cats' temperature refill by ~10–20%; without compensating drain-rate reduction, cold-stress would rise 1.5–3× on seed 42. Full four-artifact acceptance per CLAUDE.md balance methodology. Starvation and cold-death canaries held at 0.

**Dependencies (resolved at land):** phase 2 was independent; phase 3 was gated on §7.W (Fulfillment component, ticket 024); phase 4 was gated on phase 3.
