---
id: 474
title: Warder succession + shaman dispatch demand signals (festering-peer aftermath ripple)
status: ready
cluster: social-coordination
orchestration: substrate-sensitive
initiative: [welfare-fidelity, world-richness]
added: 2026-05-26
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, htn-methods.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

When a warder is taken out of action by a festering wound at a besieged ward (the (26,61) death class from `logs/tuned-42-01eb555d`), **the colony fails to detect the role vacancy** and **fails to dispatch a shaman**. There's no positional intent map for "warder slot vacant at this location" and no positional intent map for "festering peer at this position needs a high-spirituality cleanser." Per user reframe 2026-05-26: *"this seems like something that should narratively be an emergent event but impacts things bigly, like our warder getting injured means replacements + shaman has to cleanse them + collecting herbs + bedside grooming."*

The four user-named ripple effects partition naturally:
- **Warder replacement** — this ticket. New `WardIntentMap` lift source: when `count(eligible_warders) < expected_warder_count` (eligible = `CanWardFromSupply ∩ ¬Incapacitated ∩ ¬is_festering`), the existing `WardIntentMap` lifts at the vacant warder's last-warded position.
- **Shaman cleanses them** — this ticket. The [[473]] influence map already encodes "festering cat at position X"; cats with `IsSpiritualist` marker (landed [[173]]) + high magic skill score `MagicCleanse` highly when corruption is nearby. The composition `MagicCleanse score + [[473]] sample at peer-position` is the dispatch signal — no new intent map.
- **Collecting herbs** — covered by [[309]] (Herbcraft DSE reserve-deficit consideration) + landed [[308]] (Colony reserves belief). When festering cats need source-appropriate cures, the existing reserves-belief substrate from 308/309 lifts forager DSEs. This ticket adds the trigger: festering presence raises the cure-herb demand.
- **Bedside grooming** — covered by [[473]] (`GroomOther` lifts toward festering bonded peer via the new influence map).

Scope narrows to warder-succession + shaman-dispatch trigger + festering-cure herb-demand trigger. All three are *new sources of lift on existing intent surfaces*, not new intent maps.

## Hot context

In `logs/tuned-42-01eb555d`, after Heron died at tick 1229681, his durable ward at (35,60) persisted but no other cat took over the role. The colony had no signal "warder slot vacant" because no substrate tracks "Heron was warding here." Then 13k ticks later, Simba placed durable ward at (35,60) and walked into the same misfire trap at (26,61) — possibly because the placement scorer saw "no recent ward here" without seeing "the last warder died doing exactly this."

Shaman dispatch: Heron's bonded peers Mocha + Calcifer + Bramble include cats with magic skill (Bramble's skill `magic: 1.058`, snapshot tick 1229400). Bramble was Cooking at (20,20) while Heron bled out at (26,61). The substrate had no signal pulling her toward the festering peer; her own MagicCleanse DSE didn't compose a "go to bonded peer at distance N who is festering" target.

This is the **colony-economy-ripple** layer of the kin-care cluster: [[472]] (foundation), [[473]] (per-pair perception + TendFestering DSE), this ticket (colony-level intent), [[475]] (role-recognition helper).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| WardIntentMap producer | `src/systems/coordination.rs::compute_ward_placement` (~line 1913) | Existing coordinator-stamped intent map. Scores tiles by threat / corruption / coverage / cat_value. Adding a "vacant-warder-here" lift source extends the existing producer. | `[verified-correct]` |
| Warder eligibility marker | `src/components/markers.rs::CanWardFromSupply` (landed [[173]] + 084) | Already exists; gates HerbcraftWard DSE eligibility. Queryable to count eligible warders. | `[verified-correct]` |
| Spiritualist marker | landed [[173]] `IsHerbalist / IsSpiritualist / HasCorruptionNearby` | Already exists. `IsSpiritualist` is the role marker for shaman dispatch. | `[verified-correct]` |
| Colony reserves belief | landed [[308]] | Mental-model facet tracking thornbriar / remedy-herb stockpile for anticipatory crafting. Already substrate-side wired. | `[verified-correct]` |
| Herbcraft reserve-deficit consideration | [[309]] (ready) | Anticipatory ward / remedy crafting from ColonyReservesBelief. Reads 308's belief facet; lifts HerbcraftGather when reserves low. **This ticket adds a new lift source to 309's consideration: festering-cat presence → cure-herb demand.** | `[verified-correct]` |
| WardPlaced events | `src/resources/event_log.rs` (existing) | Queryable to determine which cat last warded each position. The "recent warder" attribution comes from the event stream. | `[verified-correct]` |
| HTN method registry | `populate_method_registry` per CLAUDE.md "All multi-tick aspirations are HTN methods" | New methods slot here: `AssumeWarderRole` (gated on vacant-warder-intent at sample-position + `CanWardFromSupply`), `TendCorruptedKin` (gated on [[473]] map sample + `IsSpiritualist` + magic skill). | `[verified-correct]` |
| Missing: festering-aware lift sources | (new) | No existing system reads `is_festering` count to lift WardIntentMap, HerbcraftGather demand, or MagicCleanse target-selection. This ticket adds them. | `[verified-defect-shape]` |

## Fix candidates

**Parameter-level options**:
- R1 — Raise the baseline weight of HerbcraftWard DSE so any eligible warder takes over: blunt, fires whether or not a warder is actually vacant. Loses succession-event narrative texture.

**Structural options**:

- R2 (**split**) — **Recommended.** Three composition-only additions, each extending an existing intent surface:
  1. **Warder-succession lift on WardIntentMap**: in `compute_ward_placement` per-tick, count `eligible_warders = CanWardFromSupply ∩ ¬Incapacitated ∩ ¬is_festering`. When `count < expected_warder_count`, look up the most-recent `WardPlaced` event per vacant slot and lift `WardIntentMap` at those positions. The existing `HerbcraftWardDse` already reads `WardIntentMap` (post-[[301]]'s conditional 4th axis); this lift just lights up unused intent surface.
  2. **Shaman-dispatch lift on MagicCleanse target selection**: extend `MagicCleanse` scoring to compose with [[473]]'s `CorruptedKinSignalMap` sample at peer-positions (read via existing target-taking pattern). Cats with `IsSpiritualist` + magic skill > threshold get a stronger lift; the cure step targets the festering cat directly (cat-not-tile, inverting the current MagicCleanse-targets-tile substrate).
  3. **Cure-herb demand lift on HerbcraftGather (via [[309]] reserves belief)**: when [[473]] sees festering peers, lift the corresponding cure-herb's demand scalar through [[308]]'s reserves-belief facet. [[309]] already gates the consideration on reserves; this ticket extends the "expected reserve" calculation to include "cure herbs for current festering count."
  4. **HTN methods** (per CLAUDE.md "All multi-tick aspirations are HTN methods"): author `AssumeWarderRole` and `TendCorruptedKin` in `populate_method_registry`, both with `ApplicableWhen` reading the lifted intent surfaces.
- R3 (**extend**) — Single mega-intent map `KinCareIntentMap` carrying combined warder-succession + shaman-dispatch + herb-demand signals. Loses the existing-substrate composition; creates a new resource for what existing intent surfaces already do.
- R4 (**rebind**) — Route through `WitnessableEvent::WarderDied` / `WitnessableEvent::FesteringObserved`, lift via belief facets. Belief layer is per-target — doesn't compose with positional intent for placement decisions. Mismatches the user's framing.
- R5 (**retire**) — Not viable.

## Recommended direction

**R2 (split)** — three orthogonal lift sources on three existing intent surfaces, plus the two HTN methods. Composition-only; no new substrate. Each lift source is independently verifiable.

Landing approach:
1. Add the warder-succession lift on `compute_ward_placement` per-tick. Verify a soak shows `WardIntentMap` lighting up at the position of a recently-dead warder.
2. Add the shaman-dispatch composition on `MagicCleanse` scoring. Note: this is the cat-not-tile retargeting of MagicCleanse — needs careful handling (existing MagicCleanse targets tiles; adding a cat-target branch may need a new `Action::CleanseCat` variant or a target-kind discriminator on the existing action). Defer this part to a follow-on if it bloats scope.
3. Extend [[309]]'s reserve-belief threshold to include cure-herbs-for-festering. Coordinate with [[309]]'s author if it lands first.
4. Author `AssumeWarderRole` and `TendCorruptedKin` HTN methods. Land with `ApplicableWhen` reading the lifted surfaces.

Per CLAUDE.md "Every dormant method has a glue ticket": the HTN methods are wired to this ticket as the glue (`wires-method` in frontmatter if landed dormant); active otherwise.

## Out of scope

- The MagicCleanse cat-target action variant (R2 step 2) if it bloats this ticket — defer to a follow-on `MagicCleanse targets cat-not-tile when peer is festering`.
- New intent-map types — not authoring any new `InfluenceMap` impls; only adding lift sources to existing maps.
- The festering wound substrate itself ([[472]]) — blocker.
- The corrupted-kin perception map ([[473]]) — sibling; this ticket reads from it.
- Role-recognition helper ([[475]]) — sibling; provides the "currently fulfilling X role" query this ticket consumes.
- Activation tuning for the new lift weights — separate balance ticket per `feedback_dormant_substrate_activation_soak_first`.

## Verification

- `just check && just test` clean.
- Scenario test: preload a colony with one warder (Heron) and one spiritualist (Bramble), kill Heron with `WoundKind::Festering`, advance ticks, assert (a) `WardIntentMap` lifts at Heron's last-ward position within N ticks, (b) Bramble's L3 trace shows `MagicCleanse` scoring high targeting the dying Heron, (c) the substrate emits the `WardSuccessionStarted` or analogous narrative event.
- `just soak-trace 42 Simba` post-landing — bonded peers visit a festering cat (compare to baseline where they don't); herb stockpile of cure herbs rises when festering count > 0; warder placement responds when a warder dies.
- Behavior-neutral at land if [[472]]'s `misfire_festering_chance = 0.0` (no festering = no signal = no new lift). Active behavior emerges only when [[472]] and [[473]] both tune up.

## Related work

<!-- linkages:start -->
- · **472** (ready, combat-threat) — Festering wound substrate (BLOCKER).
- · **473** (blocked, belief-perception) — Corrupted-kin signal influence map (sibling — this ticket reads its sample).
- · **475** (blocked, social-coordination) — Role-recognition helper (sibling — provides `current_role` query).
- · **309** (ready, items-crafting) — Herbcraft DSE reserve-deficit consideration (sibling — composes with this ticket's cure-herb demand lift).
- ✓ landed **308** (done) — Colony reserves belief (the substrate this ticket lifts demand into).
- ✓ landed **173** (done, ai-substrate) — IsHerbalist / IsSpiritualist capability markers (the role markers consumed here).
- ✓ landed **301** (done) — Coordinator-stamped ward intent map (the WardIntentMap producer extended here).
- ✓ landed **357** (done, ai-substrate) — HTN-driven action dispatch (the substrate the new HTN methods slot into).
- · **470** (ready, belief-perception) — Ward-siege fear influence map (cluster sibling — perception-before).
- · **471** (ready, combat-threat) — Damage events emit to log (cluster sibling — telemetry-during).
- · **474** is this ticket.
<!-- linkages:end -->

## Log

- 2026-05-26: opened from seed-42 soak `logs/tuned-42-01eb555d` kin-care cluster. User framing: warder injury triggers colony-wide response (replacements + shaman dispatch + herb gathering + bedside grooming). Scope-narrowed after `just similar` surfaced existing substrate: herb-demand routes through [[309]] / landed [[308]]; bedside-grooming routes through [[473]]; this ticket covers warder-succession + shaman-dispatch + the festering-aware demand lift on existing intent surfaces. Composition-only; no new substrate. Blocked-by [[472]]. Cluster siblings [[470]] [[471]] [[473]] [[475]].
